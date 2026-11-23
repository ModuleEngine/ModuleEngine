//! Repeatable CPU and upload measurements for a single retained S3 edit.
use super::fixtures::Fixture;
use module_engine_core::{
    Frame, RenderItem, RenderOverrides, SceneChange, SceneChangeKind, SceneSnapshot, SceneUpdate,
};
use module_engine_vulkan::{
    FrameWork, RenderPath, ResidentScene, SceneInput, VulkanRenderer, default_graph,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, time::Instant};

const SIZE: [usize; 2] = [192, 108];
const REPEATS: usize = 24;

pub(super) fn run(root: &Path, output: &Path) -> Result<(), String> {
    let fixture = Fixture::load("S3", root)?;
    let mut renderer = VulkanRenderer::new_at(root.to_owned()).map_err(|e| e.to_string())?;
    if !renderer.capabilities().resident_scene {
        return Err("R23-structure requires resident Vulkan geometry".into());
    }
    let graph = default_graph(RenderPath::Forward);
    let sources = BTreeMap::new();
    let controls = RenderOverrides::default();
    let render = |renderer: &mut VulkanRenderer,
                  scene: SceneInput<'_>|
     -> Result<(Frame, FrameWork), String> {
        let frame = renderer
            .render_scene(
                scene,
                fixture.camera,
                SIZE[0],
                SIZE[1],
                &graph,
                &sources,
                &controls,
            )
            .map_err(|error| error.to_string())?;
        Ok((
            frame,
            renderer
                .last_frame_work()
                .ok_or("No Vulkan frame counter")?,
        ))
    };
    let mut cases = Vec::<Value>::new();
    for count in [1000, fixture.scene.items.len()] {
        let mut scene = SceneSnapshot {
            items: fixture.scene.items[..count].to_vec(),
            time: 0.,
        };
        for item in &mut scene.items {
            item.id *= 2;
        }
        for (position, index) in [("start", 0), ("middle", count / 2), ("end", count - 1)] {
            for operation in ["insert", "remove", "replace_mesh"] {
                let mut retained = ResidentScene::default();
                retained.apply(SceneUpdate::Reset {
                    revision: 0,
                    snapshot: scene.clone(),
                })?;
                render(&mut renderer, SceneInput::Retained(&mut retained))?;
                let original = &scene.items[index];
                let mut edit = original.clone();
                if operation == "insert" {
                    edit.id = if index == count - 1 {
                        original.id + 1
                    } else {
                        original.id - 1
                    };
                    edit.model[1][3] += 0.42;
                    edit.color = [230, 31, 175, 255];
                } else if operation == "replace_mesh" {
                    edit.mesh.triangles.pop().ok_or("Expected nonempty mesh")?;
                }
                let moved_mesh = SceneChangeKind::Edited {
                    moved: false,
                    material_changed: false,
                    mesh_changed: true,
                };
                fn change(id: u64, kind: SceneChangeKind, items: &[RenderItem]) -> SceneChange<'_> {
                    SceneChange { id, kind, items }
                }
                let (apply, undo) = match operation {
                    "insert" => (
                        change(
                            edit.id,
                            SceneChangeKind::Spawned,
                            std::slice::from_ref(&edit),
                        ),
                        change(edit.id, SceneChangeKind::Removed, &[]),
                    ),
                    "remove" => (
                        change(edit.id, SceneChangeKind::Removed, &[]),
                        change(
                            edit.id,
                            SceneChangeKind::Spawned,
                            std::slice::from_ref(original),
                        ),
                    ),
                    _ => (
                        change(edit.id, moved_mesh, std::slice::from_ref(&edit)),
                        change(edit.id, moved_mesh, std::slice::from_ref(original)),
                    ),
                };
                // Prime both edit states once. The repeating case measures a
                // change to existing mesh/image content, plus a stable control.
                let mut samples = Vec::with_capacity(REPEATS);
                let mut fixed = Vec::with_capacity(REPEATS);
                let mut parity = false;
                let mut first_edit = None;
                for i in 0..=REPEATS {
                    let at = Instant::now();
                    retained.apply(SceneUpdate::Delta {
                        base: i as u64 * 2,
                        revision: i as u64 * 2 + 1,
                        time: 0.,
                        changes: std::slice::from_ref(&apply),
                    })?;
                    let apply_ms = at.elapsed().as_secs_f64() * 1000.;
                    let (frame, work) = render(&mut renderer, SceneInput::Retained(&mut retained))?;
                    let (_, unchanged) =
                        render(&mut renderer, SceneInput::Retained(&mut retained))?;
                    if !work.resident_scene || !unchanged.resident_scene {
                        return Err(format!(
                            "Resident path unavailable in {operation} / {position} / {count}"
                        ));
                    }
                    if i == 0 {
                        first_edit = Some(json!({"apply_ms":apply_ms,"work":work}));
                        let (reference, _) =
                            render(&mut renderer, SceneInput::Snapshot(retained.snapshot()?))?;
                        parity = frame.rgba == reference.rgba && frame.stats == reference.stats;
                    } else {
                        samples.push(json!({"apply_ms":apply_ms,"work":work}));
                        fixed.push(unchanged);
                    }
                    retained.apply(SceneUpdate::Delta {
                        base: i as u64 * 2 + 1,
                        revision: i as u64 * 2 + 2,
                        time: 0.,
                        changes: std::slice::from_ref(&undo),
                    })?;
                    render(&mut renderer, SceneInput::Retained(&mut retained))?;
                }
                cases.push(json!({
                    "drawables":count,"position":position,"operation":operation,"pixel_and_stats_parity":parity,
                    "first_edit":first_edit,"edited":samples,"unchanged":fixed,
                }));
            }
        }
    }
    let parity = cases
        .iter()
        .all(|case| case["pixel_and_stats_parity"] == true);
    super::write(
        &output.join("structure.json"),
        &json!({
            "scope":"offscreen 192x108, warm meshes and textures, full diagnostic wait recorded separately",
            "device":renderer.device_name(), "viewport":SIZE, "samples_per_case":REPEATS, "cases":cases,
        }),
    )?;
    if parity {
        Ok(())
    } else {
        Err(
            "At least one structural edit changed RGBA or render stats versus the full snapshot"
                .into(),
        )
    }
}
