//! Matched resident R2.4 and task/mesh runs of the original S5 fixture.

use super::{SIZE, fixtures::Fixture, write};
use module_engine_core::{RenderOverrides, SceneSnapshot, SceneUpdate};
use module_engine_vulkan::{
    FrameWork, MeshletDeviceReport, RenderPath, ResidentScene, SceneCulling, SceneInput,
    VulkanRenderer, default_graph,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

const MAX_CHANNEL_DELTA: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PathUnderTest {
    Indirect,
    Mesh,
}

impl PathUnderTest {
    pub(super) fn from_scene(scene: &str) -> Option<Self> {
        match scene {
            "S5-r24" => Some(Self::Indirect),
            "S5-r81" => Some(Self::Mesh),
            _ => None,
        }
    }

    pub(super) fn check_work(
        self,
        work: &FrameWork,
        triangles: usize,
        first: bool,
    ) -> Result<(), String> {
        let matches_path = match self {
            Self::Indirect => {
                work.resident_scene
                    && work.culling_mode == Some(SceneCulling::HiZ)
                    && work.occluder_draw_calls == 1
                    && work.culling_stats.is_some_and(|stats| stats.visible > 0)
                    && work.meshlets.is_none()
                    && (work.vertex_write_bytes != 0) == first
            }
            Self::Mesh => {
                !work.resident_scene
                    && work.culling_mode.is_none()
                    && work.occluder_draw_calls == 0
                    && work.vertex_write_bytes == 0
                    && work.meshlets.is_some_and(|work| {
                        work.triangles_emitted == triangles
                            && work.meshlets_emitted != 0
                            && work.task_groups != 0
                            && work.meshlets_culled == 0
                            && (work.upload_bytes > 0) == first
                    })
            }
        };
        if matches_path && work.scene_draw_calls == 1 {
            Ok(())
        } else {
            Err(format!(
                "Expected R8.1 comparison path {self:?}, observed {work:?}"
            ))
        }
    }
}

pub(super) fn retained(snapshot: SceneSnapshot) -> Result<ResidentScene, String> {
    let mut scene = ResidentScene::default();
    scene.apply(SceneUpdate::Reset {
        revision: 0,
        snapshot,
    })?;
    Ok(scene)
}

pub(super) fn check_indirect_support(
    renderer: &VulkanRenderer,
    device: &MeshletDeviceReport,
    fixture: &Value,
) -> Result<(), String> {
    // Skip the expanded-vertex fallback before allocating the 20M-triangle input.
    let required = (fixture["indexed_vertices"]
        .as_u64()
        .ok_or("Missing S5 vertex count")?
        * 20)
        .max(
            fixture["source_triangles"]
                .as_u64()
                .ok_or("Missing S5 triangle count")?
                * 12,
        );
    if !renderer.capabilities().hiz_occlusion_culling
        || device.extension_enabled
        || u64::from(device.max_storage_buffer_range) < required
    {
        return Err("The S5 control requires the R2.4 resident Hi-Z route on this adapter".into());
    }
    Ok(())
}

fn forward_readbacks(renderer: &VulkanRenderer) -> Result<[Vec<u8>; 3], String> {
    let capture = |name| {
        renderer
            .synchronous_graph_readback(name)
            .map_err(|error| error.to_string())
    };
    Ok([
        capture("geometry.depth")?,
        capture("geometry.normals")?,
        capture("geometry.color")?,
    ])
}

fn read_float(bytes: &[u8], offset: usize) -> f32 {
    f32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn shared_edge(scene: &SceneSnapshot, object: f32, before: f32, after: f32) -> bool {
    let Some(object) = (object as usize).checked_sub(1) else {
        return false;
    };
    let Some(item) = scene.items.get(object) else {
        return false;
    };
    let first = 1 + scene.items[..object]
        .iter()
        .map(|item| item.mesh.triangles.len())
        .sum::<usize>();
    let triangle = |id: f32| {
        // Above this point f32 Forward IDs cannot identify every adjacent integer.
        (id > 0. && id < (1 << 24) as f32 && id.fract() == 0.)
            .then_some(id as usize)
            .and_then(|id| id.checked_sub(first))
            .and_then(|index| item.mesh.triangles.get(index))
    };
    let (Some(before), Some(after)) = (triangle(before), triangle(after)) else {
        return false;
    };
    let common: Vec<_> = before
        .iter()
        .filter(|vertex| after.contains(vertex))
        .collect();
    matches!(common.as_slice(), [a, b] if a != b)
}

fn same_edge_depth(before: f32, after: f32) -> bool {
    before.is_finite()
        && after.is_finite()
        && before > 0.
        && after > 0.
        && before.to_bits().abs_diff(after.to_bits()) <= 1
}

fn forward_pixel(planes: &[Vec<u8>; 3], index: usize) -> Value {
    let floats = |plane: &[u8], components| {
        let first = index * components * 4;
        plane[first..first + components * 4]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&bytes| f32::from_ne_bytes(bytes))
            .collect::<Vec<_>>()
    };
    json!({
        "depth": floats(&planes[0], 1)[0],
        "normal_and_object_id": floats(&planes[1], 4),
        "color_and_triangle_id": floats(&planes[2], 4),
    })
}

pub(super) fn compare_image(root: &Path, output: &Path, fxaa: bool) -> Result<(), String> {
    let fixture = Fixture::load("S5", root)?;
    let camera = fixture.camera;
    let graph = default_graph(RenderPath::Forward);
    let mut overrides = RenderOverrides::default();
    if !fxaa {
        overrides.enabled.insert("aa".into(), false);
    }
    let render = |renderer: &mut VulkanRenderer, scene: SceneInput<'_>| {
        renderer
            .render_scene(
                scene,
                camera,
                SIZE[0],
                SIZE[1],
                &graph,
                &BTreeMap::new(),
                &overrides,
            )
            .map_err(|error| error.to_string())
    };

    let (reference, reference_device, reference_work, reference_planes) = {
        let mut renderer =
            VulkanRenderer::new_at(root.to_owned()).map_err(|error| error.to_string())?;
        let device = renderer
            .meshlet_device_report()
            .map_err(|error| error.to_string())?;
        check_indirect_support(&renderer, &device, &fixture.inventory)?;
        renderer.set_scene_culling(SceneCulling::HiZ);
        renderer.set_culling_stats(true);
        let mut scene = retained(fixture.scene.clone())?;
        let frame = render(&mut renderer, SceneInput::Retained(&mut scene))?;
        let work = renderer
            .last_frame_work()
            .ok_or("Missing R2.4 work counters")?;
        PathUnderTest::Indirect.check_work(&work, frame.stats.submitted_triangles, true)?;
        (frame, device, work, forward_readbacks(&renderer)?)
    };

    let (candidate, candidate_device, candidate_work, candidate_planes, source) = {
        let mut renderer = VulkanRenderer::new_meshlet_benchmark(root.to_owned(), None, true)
            .map_err(|error| error.to_string())?;
        let device = renderer
            .meshlet_device_report()
            .map_err(|error| error.to_string())?;
        let mut scene = renderer
            .build_meshlet_probe(fixture.scene)
            .map_err(|error| error.to_string())?;
        let frame = render(&mut renderer, SceneInput::Meshlets(&mut scene))?;
        let work = renderer
            .last_frame_work()
            .ok_or("Missing R8.1 work counters")?;
        PathUnderTest::Mesh.check_work(&work, reference.stats.submitted_triangles, true)?;
        (
            frame,
            device,
            work,
            forward_readbacks(&renderer)?,
            scene.into_snapshot(),
        )
    };

    let same_device = reference_device.vendor_id == candidate_device.vendor_id
        && reference_device.device_id == candidate_device.device_id
        && reference_device.device_uuid == candidate_device.device_uuid
        && reference_device.driver_version_raw == candidate_device.driver_version_raw;
    let valid_frames = reference.is_valid()
        && candidate.is_valid()
        && reference.gpu.is_none()
        && candidate.gpu.is_none()
        && [reference.width, reference.height] == SIZE
        && [candidate.width, candidate.height] == SIZE;
    if !same_device || !valid_frames {
        return Err(
            "S5 image comparison requires valid 1080p frames on the same Vulkan adapter".into(),
        );
    }
    let mut changed_pixels = 0;
    let mut max_delta = 0;
    let mut max_non_edge_delta = 0;
    let mut changed_objects = 0;
    let mut changed_triangles = 0;
    let mut shared_edge_changes = 0;
    let mut worst = None;
    for (index, (a, b)) in reference
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(candidate.rgba.as_chunks::<4>().0)
        .enumerate()
    {
        changed_pixels += usize::from(a != b);
        let delta = a.iter().zip(b).map(|(&a, &b)| a.abs_diff(b)).max().unwrap();
        let object_offset = index * 16 + 12;
        let object = read_float(&reference_planes[1], object_offset);
        let same_object = object == read_float(&candidate_planes[1], object_offset);
        changed_objects += usize::from(!same_object);
        let before = read_float(&reference_planes[2], object_offset);
        let after = read_float(&candidate_planes[2], object_offset);
        let mut edge_change = false;
        if before != after {
            changed_triangles += 1;
            edge_change = same_object
                && same_edge_depth(
                    read_float(&reference_planes[0], index * 4),
                    read_float(&candidate_planes[0], index * 4),
                )
                && shared_edge(&source, object, before, after);
            shared_edge_changes += usize::from(edge_change);
        }
        if !edge_change {
            max_non_edge_delta = max_non_edge_delta.max(delta);
        }
        if delta > max_delta {
            max_delta = delta;
            worst = Some(index);
        }
    }
    let worst_pixel = worst.map(|index| {
        json!({
            "xy": [index % SIZE[0], index / SIZE[0]],
            "r24": forward_pixel(&reference_planes, index),
            "r81": forward_pixel(&candidate_planes, index),
        })
    });
    let reference_counts = json!(reference.stats);
    let candidate_counts = json!(candidate.stats);
    let expected_triangles = fixture.inventory["source_triangles"].as_u64().unwrap();
    let passed = max_non_edge_delta <= MAX_CHANNEL_DELTA
        && changed_objects == 0
        && changed_triangles == shared_edge_changes
        && reference_counts == candidate_counts
        && reference.stats.submitted_triangles as u64 == expected_triangles
        && reference.stats.shaded_pixels > 0;
    crate::write_png(&output.join("s5-r24.png"), &reference)?;
    crate::write_png(&output.join("s5-r81.png"), &candidate)?;
    write(
        &output.join("comparison.json"),
        &json!({
            "status": if passed { "complete" } else { "failed" },
        "fxaa": fxaa,
            "camera": camera,
            "render_pixels": SIZE,
            "fixture": fixture.inventory,
            "max_channel_error_allowed": MAX_CHANNEL_DELTA,
            "max_channel_error": max_delta,
            "changed_pixels": changed_pixels,
            "max_channel_error_outside_shared_edges": max_non_edge_delta,
            "forward_readback": {
                "changed_object_id_pixels": changed_objects,
                "changed_triangle_id_pixels": changed_triangles,
                "shared_edge_ownership_changes": shared_edge_changes,
                "worst_final_pixel": worst_pixel,
            },
            "r24": {"device": reference_device, "stats": reference_counts, "work": reference_work},
            "r81": {"device": candidate_device, "stats": candidate_counts, "work": candidate_work},
        }),
    )?;
    if !passed {
        return Err(format!(
            "S5 image differs: max RGBA8 delta {max_non_edge_delta} outside verified shared edges, \
             {changed_objects} object-ID and {} non-edge triangle-ID changes",
            changed_triangles - shared_edge_changes
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_engine_core::{Mesh, RenderItem, Transform};

    #[test]
    fn edge_equivalence_checks_topology_and_adjacent_depth_values() {
        let scene = SceneSnapshot {
            items: vec![RenderItem {
                id: 1,
                mesh: Mesh::cube(),
                model: Transform::default().matrix(),
                color: [255; 4],
                material: Default::default(),
            }],
            ..Default::default()
        };
        assert!(shared_edge(&scene, 1., 1., 2.));
        assert!(!shared_edge(&scene, 1., 1., 3.));
        assert!(!shared_edge(&scene, 0., 1., 2.));
        assert!(!shared_edge(&scene, 1., 1., (1 << 24) as f32));
        let depth = 87_f32;
        assert!(same_edge_depth(depth, depth.next_up()));
        assert!(!same_edge_depth(depth, depth.next_up().next_up()));
        assert!(!same_edge_depth(0., 0.));
    }
}
