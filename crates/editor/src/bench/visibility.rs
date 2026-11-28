//! Stock raster reference checks, including occlusion and disocclusion.
use module_engine_core::{
    Camera, Graph, Material, Mesh, RenderItem, RenderOverrides, SceneChange, SceneChangeKind,
    SceneSnapshot, SceneUpdate, TextureImage, Transform,
};
use module_engine_vulkan::{
    RenderPath, ResidentScene, SceneCulling, SceneInput, VulkanRenderer, default_graph,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, sync::Arc};

const SIZE: [usize; 2] = [641, 361];
const CAMERA: Camera = Camera {
    yaw: 0.,
    pitch: 0.,
    distance: 8.,
    target: [0.; 3],
};

fn cube(id: u64, translation: [f32; 3], scale: [f32; 3], color: [u8; 4]) -> RenderItem {
    RenderItem {
        id,
        mesh: Mesh::cube(),
        model: Transform {
            translation,
            scale,
            ..Default::default()
        }
        .matrix(),
        color,
        material: Material::default(),
    }
}

fn draw(
    renderer: &mut VulkanRenderer,
    scene: &mut ResidentScene,
    graph: &Graph,
    camera: Camera,
    size: [usize; 2],
) -> Result<module_engine_core::Frame, String> {
    renderer
        .render_scene(
            SceneInput::Retained(scene),
            camera,
            size[0],
            size[1],
            graph,
            &BTreeMap::new(),
            &RenderOverrides::default(),
        )
        .map_err(|error| error.to_string())
}

pub(super) fn run(root: &Path, output: &Path) -> Result<(), String> {
    let mut reference = VulkanRenderer::new_at(root.to_owned()).map_err(|e| e.to_string())?;
    let mut candidate = VulkanRenderer::new_at(root.to_owned()).map_err(|e| e.to_string())?;
    let capabilities = candidate.capabilities();
    if !capabilities.hiz_occlusion_culling {
        return Err(format!(
            "R24-visibility requires sampled D32 and GPU culling on {}",
            candidate.device_name()
        ));
    }
    reference.set_scene_culling(SceneCulling::Off);
    candidate.set_culling_stats(true);
    candidate.set_frame_profiling(true);
    let mut samples: Vec<Value> = Vec::new();
    let result = (|| {
        for (name, path) in [
            ("forward", RenderPath::Forward),
            ("deferred", RenderPath::Deferred),
        ] {
            let graph = default_graph(path);
            for (mode_name, mode) in [
                ("frustum", SceneCulling::Frustum),
                ("hiz", SceneCulling::HiZ),
            ] {
                candidate.set_scene_culling(mode);
                for count in [8_usize, 257, 4099] {
                    let wall = cube(20, [0., 0., 2.], [1.55, 1.45, 0.12], [234, 26, 14, 255]);
                    let target = cube(40, [0.; 3], [0.24; 3], [12, 162, 244, 255]);
                    let side = cube(60, [-3., -0.65, 0.2], [0.22; 3], [235, 191, 22, 255]);
                    let edge = cube(80, [7.35, 0.9, 0.], [0.23; 3], [54, 235, 122, 255]);
                    let outside = cube(100, [510., 0., 0.], [0.24; 3], [200, 58, 187, 255]);
                    let mut items = vec![wall.clone(), target, side.clone(), edge, outside];
                    let fillers = count - 6; // A new earlier object is inserted after the first draw.
                    items.extend((0..fillers).map(|i| {
                        let position = if i % 2 == 0 {
                            [0., 0., -2.]
                        } else {
                            [520., 0., 0.]
                        };
                        cube(i as u64 + 110, position, [0.18; 3], [117, 27, 143, 255])
                    }));
                    let scene = SceneSnapshot {
                        lights: Vec::new(),
                        items,
                        time: 0.,
                    };
                    let mut expected_scene = ResidentScene::default();
                    let mut actual_scene = ResidentScene::default();
                    for retained in [&mut expected_scene, &mut actual_scene] {
                        retained.apply(SceneUpdate::Reset {
                            revision: 0,
                            snapshot: scene.clone(),
                        })?;
                    }
                    draw(&mut reference, &mut expected_scene, &graph, CAMERA, SIZE)?;
                    draw(&mut candidate, &mut actual_scene, &graph, CAMERA, SIZE)?;

                    // Same depth as the side cube, but first in the logical
                    // order and last in the physical slots. Its texture is
                    // evidence for both the tie and physical material lookup.
                    let mut tie = side;
                    tie.id = 10;
                    tie.color = [255; 4];
                    tie.material.base_color_texture = Some(Arc::new(TextureImage {
                        width: 1,
                        height: 1,
                        rgba8: vec![105, 41, 226, 255],
                        blocks: None,
                    }));
                    let insertion = [SceneChange {
                        id: 10,
                        kind: SceneChangeKind::Spawned,
                        items: std::slice::from_ref(&tie),
                    }];
                    for scene in [&mut expected_scene, &mut actual_scene] {
                        scene.apply(SceneUpdate::Delta {
                            lights: &[],
                            base: 0,
                            revision: 1,
                            time: 0.,
                            changes: &insertion,
                        })?;
                    }
                    for (pose, camera) in [
                        ("hidden", CAMERA),
                        (
                            "orbit",
                            Camera {
                                yaw: 1.08,
                                ..CAMERA
                            },
                        ),
                        ("moved_wall", CAMERA),
                    ] {
                        if pose == "moved_wall" {
                            let mut moved = wall.clone();
                            moved.model[0][3] = -3.8;
                            let motion = [SceneChange {
                                id: moved.id,
                                kind: SceneChangeKind::Edited {
                                    moved: true,
                                    mesh_changed: false,
                                    material_changed: false,
                                },
                                items: std::slice::from_ref(&moved),
                            }];
                            for scene in [&mut expected_scene, &mut actual_scene] {
                                scene.apply(SceneUpdate::Delta {
                                    lights: &[],
                                    base: 1,
                                    revision: 2,
                                    time: 0.,
                                    changes: &motion,
                                })?;
                            }
                        }
                        let expected =
                            draw(&mut reference, &mut expected_scene, &graph, camera, SIZE)?;
                        let reference_work = reference
                            .last_frame_work()
                            .ok_or("Missing reference counters")?;
                        let actual = draw(&mut candidate, &mut actual_scene, &graph, camera, SIZE)?;
                        let work = candidate
                            .last_frame_work()
                            .ok_or("Missing candidate counters")?;
                        let stats = work
                            .culling_stats
                            .ok_or("Missing GPU visibility counters")?;
                        let center_offset = ((SIZE[1] / 2) * SIZE[0] + SIZE[0] / 2) * 4;
                        let center = &actual.rgba[center_offset..center_offset + 4];
                        let center_matches = if pose == "hidden" {
                            center[0] > center[2]
                        } else {
                            center[2] > center[0]
                        };
                        let pixel_diffs = expected
                            .rgba
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .zip(actual.rgba.as_chunks::<4>().0)
                            .filter(|(a, b)| a != b)
                            .count();
                        samples.push(json!({
                            "path":name,"mode":mode_name,"drawables":count,"pose":pose,"camera":camera,
                            "pixel_differences":pixel_diffs,"stats_match":expected.stats == actual.stats,
                            "center_rgba":center,"render_stats":actual.stats,
                            "reference_work":reference_work,"work":work,"timings":actual.timings,
                        }));
                        if (count == 257 && mode == SceneCulling::HiZ) || pixel_diffs != 0 {
                            crate::write_png(
                                &output.join(format!("{name}-{mode_name}-{count}-{pose}.png")),
                                &actual,
                            )?;
                            crate::write_png(
                                &output.join(format!(
                                    "{name}-{mode_name}-{count}-{pose}-reference.png"
                                )),
                                &expected,
                            )?;
                        }
                        if work.culling_mode != Some(mode)
                            || work.scene_draw_calls != 1
                            || reference_work.scene_draw_calls < 2
                            || work.occluder_draw_calls != usize::from(mode == SceneCulling::HiZ)
                            || pixel_diffs != 0
                            || expected.stats != actual.stats
                            || !center_matches
                        {
                            return Err(format!(
                                "Reference mismatch: {name}/{mode_name}/{count}/{pose}, {pixel_diffs} differing pixels"
                            ));
                        }
                        if pose == "hidden" {
                            let expected_candidates = (5 + fillers.div_ceil(2)) as u32;
                            let (occluders, visible) = if mode == SceneCulling::HiZ {
                                (1, 4)
                            } else {
                                (0, expected_candidates)
                            };
                            if (stats.frustum_candidates, stats.occluders, stats.visible)
                                != (expected_candidates, occluders, visible)
                            {
                                return Err(format!(
                                    "Unexpected visibility: {name}/{mode_name}/{count}: {stats:?}"
                                ));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    })();
    super::write(
        &output.join("visibility.json"),
        &json!({
            "scope":"offscreen retained stock passes, exact R2.3 reference, camera and occluder motion",
            "device":candidate.device_name(),"capabilities":capabilities,"size":SIZE,
            "samples":samples,"error":result.as_ref().err(),
        }),
    )?;
    result?;
    scaled_capture(root, output, &mut reference, &mut candidate)
}

fn scaled_capture(
    root: &Path,
    output: &Path,
    reference: &mut VulkanRenderer,
    candidate: &mut VulkanRenderer,
) -> Result<(), String> {
    let mut fixture = super::fixtures::Fixture::load("S3-100k", root)?;
    fixture.tick(270);
    let mut scene = ResidentScene::default();
    scene.apply(SceneUpdate::Reset {
        revision: 0,
        snapshot: fixture.scene,
    })?;
    let graph = default_graph(RenderPath::Forward);
    reference.set_detailed_stats(false);
    candidate.set_detailed_stats(false);
    candidate.set_scene_culling(SceneCulling::HiZ);
    let expected = draw(reference, &mut scene, &graph, fixture.camera, super::SIZE)?;
    let actual = draw(candidate, &mut scene, &graph, fixture.camera, super::SIZE)?;
    let work = candidate
        .last_frame_work()
        .ok_or("Missing S3-100k counters")?;
    let matches = actual.rgba == expected.rgba;
    crate::write_png(&output.join("s3-100k-1080p.png"), &actual)?;
    crate::write_png(&output.join("s3-100k-1080p-reference.png"), &expected)?;
    super::write(
        &output.join("s3-100k-capture.json"),
        &json!({
            "scope":"unscored offscreen snapshot of the moving S3-100k fixture",
            "camera":fixture.camera,"size":super::SIZE,"work":work,"pixel_parity":matches,
        }),
    )?;
    if !matches
        || work.culling_mode != Some(SceneCulling::HiZ)
        || work.scene_draw_calls != 1
        || work
            .culling_stats
            .is_none_or(|stats| stats.visible != 100_000)
    {
        return Err("S3-100k Hi-Z capture differs from the reference or fixture counts".into());
    }
    Ok(())
}
