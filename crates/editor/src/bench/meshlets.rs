//! Matched resident R2.4 and task/mesh runs of the original S5 fixture.

use super::{SIZE, fixtures::Fixture, write};
use module_engine_core::{
    Camera, RenderItem, RenderOverrides, SceneSnapshot, SceneUpdate, transform_point,
};
use module_engine_vulkan::{
    FrameWork, MeshletDeviceReport, RenderPath, ResidentScene, SceneCulling, SceneInput,
    VulkanRenderer, default_graph, project_meshlet_benchmark_vertex,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

const MAX_CHANNEL_DELTA: u8 = 2;
const MAX_EDGE_CHANNEL_DELTA: u8 = 2 * MAX_CHANNEL_DELTA;
const MAX_EDGE_PIXELS: usize = SIZE[0] * SIZE[1] / 1_000_000;
const FORWARD_FLOAT_EPSILON: f32 = 1e-5;

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

struct SharedEdge<'a> {
    item: &'a RenderItem,
    faces: [[usize; 3]; 2],
    edge: [usize; 2],
}

fn shared_edge(
    scene: &SceneSnapshot,
    object: f32,
    before: f32,
    after: f32,
) -> Option<SharedEdge<'_>> {
    let object = (object as usize).checked_sub(1)?;
    let item = scene.items.get(object)?;
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
    let before = *triangle(before)?;
    let after = *triangle(after)?;
    let common: Vec<_> = before
        .into_iter()
        .filter(|vertex| after.contains(vertex))
        .collect();
    let [u, v] = *common.as_slice() else {
        return None;
    };
    (u != v).then_some(SharedEdge {
        item,
        faces: [before, after],
        edge: [u, v],
    })
}

fn same_edge_depth(before: f32, after: f32) -> bool {
    before.is_finite()
        && after.is_finite()
        && before > 0.
        && after > 0.
        && before.to_bits().abs_diff(after.to_bits()) <= 1
}

#[derive(Clone, Copy, serde::Serialize)]
struct ForwardPixel {
    depth: f32,
    normal_and_object_id: [f32; 4],
    color_and_triangle_id: [f32; 4],
}

fn forward_pixel(planes: &[Vec<u8>; 3], index: usize) -> ForwardPixel {
    ForwardPixel {
        depth: read_float(&planes[0], index * 4),
        normal_and_object_id: std::array::from_fn(|i| read_float(&planes[1], index * 16 + 4 * i)),
        color_and_triangle_id: std::array::from_fn(|i| read_float(&planes[2], index * 16 + 4 * i)),
    }
}

fn pixel_center_over_edge(endpoints: [[f32; 2]; 2], pixel: [usize; 2], subpixel: f32) -> bool {
    let [a, b] = endpoints;
    let [x, y] = pixel.map(|coord| coord as f32 + 0.5);
    let [u, v] = [b[0] - a[0], b[1] - a[1]];
    let [dx, dy] = [x - a[0], y - a[1]];
    let length = u.hypot(v);
    let along = (dx * u + dy * v) / length;
    (0. ..=length).contains(&along) && (dx * v - dy * u).abs() <= subpixel * length
}

fn normalize(vector: [f32; 3]) -> [f32; 3] {
    let length = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if length > 0.00001 {
        vector.map(|x| x / length)
    } else {
        [0., 1., 0.]
    }
}

fn matches_forward_face(item: &RenderItem, positions: [[f32; 3]; 3], pixel: ForwardPixel) -> bool {
    if item.material.base_color_texture.is_some() {
        return false;
    }
    let [a, b, c] = positions;
    let [ux, uy, uz] = std::array::from_fn(|i| b[i] - a[i]);
    let [vx, vy, vz] = std::array::from_fn(|i| c[i] - a[i]);
    let normal = normalize([uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx]);
    let light = normalize([-0.5, 1., 0.7]);
    let diffuse = 0.20
        + 1.65
            * normal
                .iter()
                .zip(light)
                .map(|(n, l)| n * l)
                .sum::<f32>()
                .abs();
    (0..3).all(|i| {
        let expected_color =
            f32::from(item.color[i]) / 255. * item.material.base_color[i] * diffuse;
        (pixel.normal_and_object_id[i] - normal[i]).abs() <= FORWARD_FLOAT_EPSILON
            && (pixel.color_and_triangle_id[i] - expected_color).abs() <= FORWARD_FLOAT_EPSILON
    })
}

fn valid_edge_change(
    scene: &SceneSnapshot,
    camera: Camera,
    position: [usize; 2],
    subpixel: f32,
    pixels: [ForwardPixel; 2],
) -> bool {
    let [a, b] = pixels;
    let object = a.normal_and_object_id[3];
    if object != b.normal_and_object_id[3] || !same_edge_depth(a.depth, b.depth) {
        return false;
    }
    let Some(shared) = shared_edge(
        scene,
        object,
        a.color_and_triangle_id[3],
        b.color_and_triangle_id[3],
    ) else {
        return false;
    };
    let world = |i: usize| transform_point(shared.item.model, shared.item.mesh.vertices[i]);
    let [Some(u), Some(v)] = shared
        .edge
        .map(|i| project_meshlet_benchmark_vertex(camera, SIZE, world(i)))
    else {
        return false;
    };
    pixel_center_over_edge([u, v], position, subpixel)
        && shared
            .faces
            .into_iter()
            .zip(pixels)
            .all(|(face, pixel)| matches_forward_face(shared.item, face.map(world), pixel))
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
    let subpixel = 2_f32.powi(-(reference_device.sub_pixel_precision_bits as i32));
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
            edge_change = valid_edge_change(
                &source,
                camera,
                [index % SIZE[0], index / SIZE[0]],
                subpixel,
                [
                    forward_pixel(&reference_planes, index),
                    forward_pixel(&candidate_planes, index),
                ],
            );
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
        && max_delta <= MAX_EDGE_CHANNEL_DELTA
        && shared_edge_changes <= MAX_EDGE_PIXELS
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
            "max_shared_edge_channel_error_allowed": MAX_EDGE_CHANNEL_DELTA,
            "max_shared_edge_pixels_allowed": MAX_EDGE_PIXELS,
            "edge_distance_pixels_allowed": subpixel,
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
            "S5 image differs: max RGBA8 delta {max_delta} ({max_non_edge_delta} outside verified shared edges), \
             {shared_edge_changes} shared-edge, {changed_objects} object-ID and {} other triangle-ID changes",
            changed_triangles - shared_edge_changes
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_engine_core::{Mesh, Transform};

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
        assert!(shared_edge(&scene, 1., 1., 2.).is_some());
        assert!(shared_edge(&scene, 1., 1., 3.).is_none());
        assert!(shared_edge(&scene, 0., 1., 2.).is_none());
        assert!(shared_edge(&scene, 1., 1., (1 << 24) as f32).is_none());
        let depth = 87_f32;
        assert!(same_edge_depth(depth, depth.next_up()));
        assert!(!same_edge_depth(depth, depth.next_up().next_up()));
        assert!(!same_edge_depth(0., 0.));
        let edge = [[0., 2.5], [5., 2.5]];
        let subpixel = 1. / 256.;
        assert!(pixel_center_over_edge(edge, [2, 2], subpixel));
        assert!(!pixel_center_over_edge(edge, [2, 3], subpixel));
        assert!(!pixel_center_over_edge(edge, [5, 2], subpixel));
        let item = &scene.items[0];
        let positions = item.mesh.triangles[0].map(|i| item.mesh.vertices[i]);
        let diffuse = 0.2 + 1.65 * 0.7 / 1.74_f32.sqrt();
        let mut pixel = ForwardPixel {
            depth,
            normal_and_object_id: [0., 0., -1., 1.],
            color_and_triangle_id: [diffuse, diffuse, diffuse, 1.],
        };
        assert!(matches_forward_face(item, positions, pixel));
        pixel.color_and_triangle_id[0] += 0.1;
        assert!(!matches_forward_face(item, positions, pixel));
    }
}
