//! Matched resident R2.4 and task/mesh runs of the original S5 fixture.

use super::{SIZE, fixtures::Fixture, write};
use module_engine_core::{RenderOverrides, SceneSnapshot, SceneUpdate};
use module_engine_vulkan::{
    FrameWork, MeshletDeviceReport, RenderPath, ResidentScene, SceneCulling, SceneInput,
    VulkanRenderer, default_graph,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

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

pub(super) fn compare_image(root: &Path, output: &Path) -> Result<(), String> {
    let fixture = Fixture::load("S5", root)?;
    let camera = fixture.camera;
    let graph = default_graph(RenderPath::Forward);
    let render = |renderer: &mut VulkanRenderer, scene: SceneInput<'_>| {
        renderer
            .render_scene(
                scene,
                camera,
                SIZE[0],
                SIZE[1],
                &graph,
                &BTreeMap::new(),
                &RenderOverrides::default(),
            )
            .map_err(|error| error.to_string())
    };

    let (reference, reference_device, reference_work) = {
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
        (frame, device, work)
    };

    let (candidate, candidate_device, candidate_work) = {
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
        (frame, device, work)
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
    for (a, b) in reference
        .rgba
        .chunks_exact(4)
        .zip(candidate.rgba.chunks_exact(4))
    {
        changed_pixels += usize::from(a != b);
        for (&a, &b) in a.iter().zip(b) {
            max_delta = max_delta.max(a.abs_diff(b));
        }
    }
    let reference_counts = json!(reference.stats);
    let candidate_counts = json!(candidate.stats);
    let expected_triangles = fixture.inventory["source_triangles"].as_u64().unwrap();
    let passed = max_delta <= 2
        && reference_counts == candidate_counts
        && reference.stats.submitted_triangles as u64 == expected_triangles
        && reference.stats.shaded_pixels > 0;
    crate::write_png(&output.join("s5-r24.png"), &reference)?;
    crate::write_png(&output.join("s5-r81.png"), &candidate)?;
    write(
        &output.join("comparison.json"),
        &json!({
            "status": if passed { "complete" } else { "failed" },
            "camera": camera,
            "render_pixels": SIZE,
            "fixture": fixture.inventory,
            "max_channel_error_allowed": 2,
            "max_channel_error": max_delta,
            "changed_pixels": changed_pixels,
            "r24": {"device": reference_device, "stats": reference_counts, "work": reference_work},
            "r81": {"device": candidate_device, "stats": candidate_counts, "work": candidate_work},
        }),
    )?;
    if !passed {
        return Err(format!(
            "S5 image or geometry counts differ: max channel delta {max_delta}, {changed_pixels} changed pixels"
        ));
    }
    Ok(())
}
