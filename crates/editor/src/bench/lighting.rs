use super::{Fixture, SIZE, write};
use module_engine_core::{GpuTimings, LightKind, RenderOverrides, SceneLight, SceneUpdate};
use module_engine_vulkan::{RenderPath, ResidentScene, SceneInput, VulkanRenderer, default_graph};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

const WARMUP: usize = 12;
const FRAMES: usize = 60;
const BUDGET_MS: f64 = 2.0;
const REFERENCE_ADAPTER: &str = "NVIDIA GeForce RTX 4080 SUPER";

#[derive(Deserialize)]
struct FixturePoint {
    id: u64,
    position: [f32; 3],
    linear_rgb: [f32; 3],
    intensity_cd: f32,
    range_m: f32,
}

fn summarize(mut values: Vec<f64>) -> Value {
    values.sort_by(f64::total_cmp);
    json!({
        "mean": values.iter().sum::<f64>() / values.len() as f64,
        "p95": values[(values.len() * 95).div_ceil(100) - 1],
        "max": values.last(),
    })
}

pub(super) fn run(root: &Path, output: &Path) -> Result<(), String> {
    let mut fixture = Fixture::load("S4", root)?;
    let points: Vec<FixturePoint> =
        serde_json::from_value(fixture.inventory["point_lights"].clone())
            .map_err(|e| e.to_string())?;
    if points.len() != 256 {
        return Err("R5.2 requires the original 256 S4 point lights".into());
    }
    let sun_direction = [-0.5_f32, 1., 0.7];
    let length = sun_direction.into_iter().map(|v| v * v).sum::<f32>().sqrt();
    let sun = SceneLight {
        id: 257,
        kind: LightKind::Directional,
        position: [0.; 3],
        direction: sun_direction.map(|v| -v / length),
        color: [1.; 3],
        intensity: 3.,
        range: 1.,
        inner_angle_cos: 1.,
        outer_angle_cos: 0.,
    };
    let mut lit = vec![sun];
    lit.extend(points.into_iter().map(|point| SceneLight {
        id: point.id,
        kind: LightKind::Point,
        position: point.position,
        color: point.linear_rgb,
        intensity: point.intensity_cd,
        range: point.range_m,
        ..sun
    }));

    let mut report = Vec::new();
    let mut adapter = None;
    let mut within_budget = true;
    for (name, route, timer) in [
        ("forward", RenderPath::Forward, "geometry"),
        ("deferred", RenderPath::Deferred, "lighting"),
    ] {
        let mut renderer = VulkanRenderer::new_at(root.to_owned()).map_err(|e| e.to_string())?;
        let device = renderer.device_name();
        if !device.contains(REFERENCE_ADAPTER) || !renderer.capabilities().clustered_resident_scene
        {
            return Err(format!(
                "R5.2 requires the reference RTX 4080 SUPER and resident lighting, selected {device}"
            ));
        }
        adapter = Some(device.to_owned());
        renderer.set_frame_profiling(true);
        renderer.set_detailed_stats(false);

        let graph = default_graph(route);
        let mut resident = ResidentScene::default();
        let mut pairs = Vec::new();
        let mut sun_image = Vec::new();
        let mut sun_receiver_times = Vec::new();
        let mut lit_cost = Vec::new();
        let mut point_changed_pixels = 0;
        for (revision, (mode, lights)) in [
            ("original", &[][..]),
            ("sun_only", std::slice::from_ref(&sun)),
            ("256_points", lit.as_slice()),
        ]
        .into_iter()
        .enumerate()
        {
            fixture.scene.lights = lights.to_vec();
            resident.apply(SceneUpdate::Reset {
                revision: revision as u64,
                snapshot: fixture.scene.clone(),
            })?;
            let mut samples = Vec::with_capacity(FRAMES);
            let mut costs = Vec::with_capacity(FRAMES);
            let mut receiver_times = Vec::with_capacity(FRAMES);
            for index in 0..WARMUP + FRAMES {
                let frame = renderer
                    .render_scene(
                        SceneInput::Retained(&mut resident),
                        fixture.camera,
                        SIZE[0],
                        SIZE[1],
                        &graph,
                        &BTreeMap::new(),
                        &RenderOverrides::default(),
                    )
                    .map_err(|e| e.to_string())?;
                let work = renderer
                    .last_frame_work()
                    .ok_or("R5.2 missing completed frame work")?;
                let expected_local = if mode == "256_points" { 256 } else { 0 };
                if work.clustered_local_lights != expected_local
                    || work.punctual_lights != lights.len()
                    || !work.resident_scene
                {
                    return Err(format!(
                        "{name} {mode} did not bind the expected scene lights"
                    ));
                }
                let Some(GpuTimings::Available { graph_ms, passes }) = frame
                    .timings
                    .as_ref()
                    .and_then(|timing| timing.gpu.as_ref())
                else {
                    return Err("R5.2 requires hardware GPU timestamps".into());
                };
                let pass_time = |id| {
                    passes
                        .iter()
                        .find(|pass| pass.id == id && !pass.bypassed)
                        .map(|pass| pass.ms)
                };
                let receiver = pass_time(timer).ok_or("R5.2 missing receiver timestamp")?;
                let cluster = pass_time("Light clustering");
                if cluster.is_some() != (expected_local == 256)
                    || !receiver.is_finite()
                    || receiver <= 0.
                {
                    return Err("R5.2 frame has invalid clustered lighting timing scopes".into());
                }
                if index >= WARMUP {
                    receiver_times.push(receiver);
                    costs.push(receiver + cluster.unwrap_or(0.));
                    samples.push(json!({ "cluster_ms":cluster, "receiver_ms":receiver, "graph_ms":graph_ms, "passes":passes }));
                }
                if index == WARMUP + FRAMES - 1 {
                    crate::write_png(&output.join(format!("{name}-{mode}.png")), &frame)?;
                    if mode == "sun_only" {
                        sun_image = frame.rgba;
                    } else if mode == "256_points" {
                        point_changed_pixels = frame
                            .rgba
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .zip(sun_image.as_chunks::<4>().0)
                            .filter(|(a, b)| a != b)
                            .count();
                    }
                }
            }
            if mode == "sun_only" {
                sun_receiver_times = receiver_times;
            } else if mode == "256_points" {
                lit_cost = costs.clone();
            }
            pairs.push(
                json!({"mode":mode,"cluster_and_receiver_ms":summarize(costs),"samples":samples}),
            );
        }
        if point_changed_pixels == 0 {
            return Err(format!(
                "The 256 S4 point lights left the {name} frame unchanged"
            ));
        }
        let absolute = summarize(lit_cost.clone());
        let incremental = summarize(
            lit_cost
                .into_iter()
                .zip(sun_receiver_times)
                .map(|(lit, sun)| lit - sun)
                .collect(),
        );
        let metric = if route == RenderPath::Deferred {
            &absolute
        } else {
            &incremental
        };
        let p95 = metric["p95"].as_f64().ok_or("R5.2 missing p95")?;
        within_budget &= p95 <= BUDGET_MS;
        report.push(json!({
            "path":name,"receiver":timer,"scopes":pairs,
            "point_lights_changed_pixels":point_changed_pixels,
            "absolute_cluster_and_receiver_ms":absolute,"paired_increment_over_sun_ms":incremental,
            "budget_metric":if route == RenderPath::Deferred {"absolute_cluster_and_receiver_ms"} else {"paired_increment_over_sun_ms"},
            "budget_p95_ms":p95,
        }));
    }
    fixture.inventory["point_lights_rendered"] = json!(256);
    write(
        &output.join("lighting-report.json"),
        &json!({
            "schema_version":1,"status":if within_budget {"passed"} else {"over_budget"},
            "adapter":adapter,"resolution":SIZE,"camera":fixture.camera,"fixture":fixture.inventory,
            "warmup_frames_per_mode":WARMUP,"frames_per_mode":FRAMES,"budget_ms":BUDGET_MS,"graphs":report,
        }),
    )?;
    if !within_budget {
        return Err("R5.2 lighting exceeded the 2 ms p95 budget".into());
    }
    Ok(())
}
