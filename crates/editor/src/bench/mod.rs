//! Source-only R0.2 fixtures using the existing native renderer and Player bridge.
mod fixtures;
mod materials;
mod meshlets;
#[path = "../../../../examples/bench/s3_ecs.rs"]
mod s3_ecs;
mod structure;
mod visibility;

use eframe::egui;
use fixtures::Fixture;
use module_engine_core::{
    CompositionContext, FrameTimings, Graph, GraphDocument, RenderOverrides, RenderSubmission,
    SceneChange, SceneChangeKind, SceneUpdate,
};
use module_engine_host::{GameHost, HostOptions, RenderMode};
use module_engine_player::{
    gpu_presentation::{GpuPresentationBridge, GpuTexture},
    ui,
};
use module_engine_vulkan::{
    FrameWork, MeshletScene, RenderPath, ResidentScene, SceneCulling, SceneInput, VulkanRenderer,
    builtin_hlsl_source, builtin_manifest, default_graph,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const SIZE: [usize; 2] = [1920, 1080];
const WARMUP: usize = 30;
const FRAMES: usize = 240;
const DENSE_WARMUP: usize = 5;
const DENSE_FRAMES: usize = 30;

pub fn run(id: &str, folder: &Path) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // This developer command is only available in a bench-enabled source build.
    // Keep the output and game services isolated from installed packages.
    fs::create_dir(folder).map_err(|e| format!("Create new benchmark output folder: {e}"))?;
    let folder = folder.canonicalize().map_err(|e| e.to_string())?;
    match id {
        "S1" | "S2" => game(id, &root, &folder),
        "S3" | "S3-100k" | "S4" | "S5" | "S5-r24" | "S5-r81" => scene(id, &root, &folder),
        "S4-floor" | "S4-floor-atlas" => {
            s4_floor(&root, &folder, RenderPath::Forward, id == "S4-floor-atlas")
        }
        "S4-floor-deferred" => s4_floor(&root, &folder, RenderPath::Deferred, false),
        "R2-textures" => materials::check(&root, &folder),
        "R23-structure" => structure::run(&root, &folder),
        "R24-visibility" => visibility::run(&root, &folder),
        "R81-visibility" => meshlets::compare_image(&root, &folder, true),
        "R81-visibility-no-fxaa" => meshlets::compare_image(&root, &folder, false),
        _ => Err(format!("Unknown benchmark scene {id}")),
    }
}

// A fixed camera path through the stock graphs. Explicit readback makes
// consecutive PNGs reproducible without involving the desktop compositor.
fn s4_floor(
    root: &Path,
    folder: &Path,
    path: RenderPath,
    atlas_control: bool,
) -> Result<(), String> {
    const FRAMES: usize = 32;
    let mut fixture = Fixture::load("S4", root)?;
    let mut graph = default_graph(path);
    let mut sources = BTreeMap::new();
    let mut renderer = VulkanRenderer::new_at(root.to_owned()).map_err(|e| e.to_string())?;
    let capabilities = renderer.capabilities();
    if atlas_control || !capabilities.bindless_materials {
        fixture.use_legacy_s4_textures()?;
    }
    if atlas_control {
        // Replay the original material ABI on the same device, graph and pose.
        let geometry = graph
            .nodes
            .iter_mut()
            .find(|node| node.id == "geometry")
            .ok_or("S4 Forward geometry pass is missing")?;
        let mut manifest =
            builtin_manifest(&geometry.module).ok_or("S4 Forward manifest is missing")?;
        manifest.bindless_materials = false;
        manifest.vertex_entry = Some("mainVS".into());
        sources.insert(
            "legacy_forward".into(),
            builtin_hlsl_source(&geometry.module)
                .ok_or("S4 Forward shader is missing")?
                .into(),
        );
        geometry.module = "gpu.custom".into();
        geometry.params["source_id"] = json!("legacy_forward");
        geometry.params["manifest"] = serde_json::to_value(manifest).map_err(|e| e.to_string())?;
    }
    let mut cameras = Vec::with_capacity(FRAMES);
    for index in 0..FRAMES {
        let mut camera = fixture.camera;
        camera.pitch = 0.10;
        camera.target = [-5.0 - index as f32 * 0.025, 0.45, index as f32 * 0.0125];
        cameras.push(camera);
    }
    write(
        &folder.join("capture.json"),
        &json!({
            "fixture": fixture.inventory,
            "render_pixels": SIZE,
            "graph": format!("{path:?}"),
            "materials": if atlas_control || !capabilities.bindless_materials { "atlas" } else { "bindless" },
            "cameras": cameras,
            "adapter": renderer.device_name(),
            "device_capabilities": capabilities,
            "presentation": "Vulkan renderer with explicit diagnostic readback",
        }),
    )?;
    for (index, camera) in cameras.into_iter().enumerate() {
        let frame = renderer
            .render(
                &fixture.scene,
                camera,
                SIZE[0],
                SIZE[1],
                &graph,
                &sources,
                &RenderOverrides::default(),
            )
            .map_err(|e| e.to_string())?;
        crate::write_png(&folder.join(format!("frame-{index:03}.png")), &frame)?;
    }
    let allocations = renderer
        .synchronous_graph_image_allocations()
        .ok_or("S4 benchmark has no completed synchronous graph allocations")?;
    write(
        &folder.join("graph-memory.json"),
        &json!({"graph": format!("{path:?}"), "slot": 0, "render_pixels": SIZE, "images": allocations, "asynchronous_compute": renderer.last_frame_work().unwrap().asynchronous_compute}),
    )
}

fn game(id: &str, root: &Path, folder: &Path) -> Result<(), String> {
    let g1 = id == "S1";
    use module_engine_editor_composition::ProjectTemplate;
    let template = if g1 {
        ProjectTemplate::G1
    } else {
        ProjectTemplate::TwoArenas
    };
    if !template.installed() {
        return Err(format!(
            "Benchmark {id} requires the {} template",
            template.0
        ));
    }
    let storage = folder.join("profile");
    let mut reference = None;
    let mut host = ui::prepare_host(|| {
        let (document, asset_root, compile_root) = if g1 {
            (
                crate::game_templates::g1_document(),
                root.join("examples/crate-garden"),
                None,
            )
        } else {
            (
                crate::game_templates::demo_document(),
                root.into(),
                Some(root.into()),
            )
        };
        let composition = crate::host::composition_at(root)?;
        let document =
            if let Some(dir) = std::env::var_os("MODULE_ENGINE_BENCHMARK_R06_GAME_FIXTURES") {
                let (prepared, proof) =
                    prepare_r06_game_fixture(id, Path::new(&dir), document, &composition)?;
                reference = Some(proof);
                prepared
            } else {
                document
            };
        let host = GameHost::build(
            document,
            composition,
            HostOptions {
                asset_root,
                backend_root: Some(root.into()),
                compile_root,
                render: RenderMode::Graph,
                services: module_engine_player::service_registry_at(storage)?,
                services_enabled: true,
            },
            None,
        )?;
        Ok(host)
    })?;
    let document = serde_json::to_vec(host.accepted_document()).map_err(|e| e.to_string())?;
    let host_document_sha256 = module_engine_project::hash_bytes(&document);
    let document_sha256 = if let Some((reference_sha256, prepared_sha256)) = reference {
        if host_document_sha256 != prepared_sha256 {
            return Err(format!(
                "{id} host did not retain the checked benchmark document"
            ));
        }
        write(
            &folder.join("r06-game-fixture.json"),
            &json!({
                "reference_head": R06_REFERENCE_HEAD,
                "input_document_sha256": reference_sha256,
                "host_document_sha256": host_document_sha256,
            }),
        )?;
        reference_sha256
    } else {
        host_document_sha256
    };
    let mut input = json!({"fixture":"Two Arenas ARPG source template","input":"idle"});
    #[cfg(feature = "g1")]
    if g1 {
        // Use the game's normal Play action against the fresh profile.
        host.step(
            1. / 60.,
            &module_engine_editor_composition::g1::action_input("g1.play"),
        )?;
        let view = module_engine_editor_composition::g1::view(host.session.state())?;
        if view.phase != module_engine_editor_composition::g1::Phase::Playing {
            return Err("G1 Play action did not enter Playing".into());
        }
        input = json!({"fixture":"Crate Garden source template","input":"Play, then idle","fresh_profile":true});
    }
    input["document_sha256"] = json!(document_sha256);
    write(&folder.join("fixture.json"), &input)?;
    module_engine_player::run_benchmark_host(host, folder)
}

const R06_REFERENCE_HEAD: &str = "731661c47e181847450e00480e60c0645abd4753";

fn prepare_r06_game_fixture(
    id: &str,
    directory: &Path,
    stock: GraphDocument,
    composition: &CompositionContext,
) -> Result<(GraphDocument, (String, String)), String> {
    let expected = match id {
        "S1" => "71d44139df5d3c7eb42e5b1974bec80b37d4e3b496d0e9da77135d7c98ad73c8",
        "S2" => "cd71b49d91a25c174016e47b17ce09d0f00aebba26ea527d03ce1e8ee1a77439",
        _ => return Err(format!("No R0.6 game fixture for {id}")),
    };
    let path = directory.join(format!("{id}.json"));
    let bytes = fs::read(&path).map_err(|error| format!("Read {}: {error}", path.display()))?;
    let input_sha256 = module_engine_project::hash_bytes(&bytes);
    if input_sha256 != expected {
        return Err(format!(
            "{id} input does not match the pinned R0.6 document: {input_sha256}"
        ));
    }
    let mut document = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let profile = module_engine_project::EngineProfile::from_document(&stock)?;
    // The stock R0.6 binary validates this exact input with its own SDK.
    // Reattach the installed profile in memory using the regular lock checks.
    crate::engine_profiles::attach(&mut document, composition, &profile)?;
    let prepared = serde_json::to_vec(&document).map_err(|error| error.to_string())?;
    if prepared != serde_json::to_vec(&stock).map_err(|error| error.to_string())? {
        return Err(format!(
            "{id} replay differs from the current stock fixture after repinning"
        ));
    }
    let prepared_sha256 = module_engine_project::hash_bytes(&prepared);
    Ok((document, (input_sha256, prepared_sha256)))
}

fn write(path: &Path, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, bytes).map_err(|error| format!("Write {}: {error}", path.display()))
}

fn scene(id: &str, root: &Path, folder: &Path) -> Result<(), String> {
    let r81 = meshlets::PathUnderTest::from_scene(id);
    let fixture_started = Instant::now();
    let mut fixture = Fixture::load(if r81.is_some() { "S5" } else { id }, root)?;
    let fixture_load_ms = fixture_started.elapsed().as_secs_f64() * 1000.;
    let objects = fixture.scene.items.len();
    let triangles = fixture
        .scene
        .items
        .iter()
        .map(|i| i.mesh.triangles.len())
        .sum::<usize>();
    let (warmup, frames) = if id == "S5" || r81.is_some() {
        (DENSE_WARMUP, DENSE_FRAMES)
    } else {
        (WARMUP, FRAMES)
    };
    let gui_vsync = ui::gui_vsync_requested()?;
    let frame_profiling = ui::benchmark_profiling_enabled()?;
    let s3 = matches!(id, "S3" | "S3-100k");
    let static_s3 = s3 && std::env::var_os("MODULE_ENGINE_BENCHMARK_S3_STATIC").is_some();
    let culling = if s3 {
        match std::env::var("MODULE_ENGINE_BENCHMARK_S3_CULLING").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("hiz") => SceneCulling::HiZ,
            Ok("frustum") => SceneCulling::Frustum,
            Ok("off") => SceneCulling::Off,
            _ => {
                return Err(
                    "MODULE_ENGINE_BENCHMARK_S3_CULLING must be off, frustum or hiz".into(),
                );
            }
        }
    } else if r81 == Some(meshlets::PathUnderTest::Indirect) {
        SceneCulling::HiZ
    } else {
        SceneCulling::Off
    };
    let retained_started = Instant::now();
    let retained = if r81 == Some(meshlets::PathUnderTest::Indirect) {
        Some(meshlets::retained(std::mem::take(&mut fixture.scene))?)
    } else if s3 {
        let mut scene = ResidentScene::default();
        scene.apply(SceneUpdate::Reset {
            revision: 0,
            snapshot: fixture.scene.clone(),
        })?;
        Some(scene)
    } else {
        None
    };
    let mut report = json!({
        "static_s3":static_s3,
        "requested_culling":culling,
        "version":1,"scene":id,"status":"incomplete","fixture":fixture.inventory,"gui_vsync":gui_vsync,
        "frame_profiling":frame_profiling,
        "loaded_draw_items":objects,"loaded_triangles":triangles,"camera":fixture.camera,
        "render_pixels":SIZE,"warmup_requested":warmup,"measured_requested":frames,
        "scope":"Current Vulkan Forward graph into a native window using the Player GPU bridge",
        "metrics":{
            "meshlets_culled":null,"meshlets_drawn":null,
            "image_reference_difference":null,
        },
        "unavailable_reasons":{
            "meshlets_culled":"R8 meshlet pipeline",
            "meshlets_drawn":"R8 meshlet pipeline",
            "image_reference_difference":"No accepted S4 reference image; current Forward cannot bind the fixture's 256 point lights",
        },
    });
    if let Some(path) = r81 {
        report["r81_path"] = json!(path);
        report["fixture_load_ms"] = json!(fixture_load_ms);
        report["retained_scene_prepare_ms"] = if retained.is_some() {
            json!(retained_started.elapsed().as_secs_f64() * 1000.)
        } else {
            Value::Null
        };
        report.as_object_mut().unwrap().remove("metrics");
        report
            .as_object_mut()
            .unwrap()
            .remove("unavailable_reasons");
    }
    let path = folder.join("scene.json");
    write(&path, &report)?;
    let preflight_path = path.clone();
    let report = Arc::new(Mutex::new(report));
    let returned = Arc::clone(&report);
    let id = id.to_owned();
    let root = root.to_owned();
    let viewport = egui::ViewportBuilder::default()
        .with_title(format!("ModuleEngine / {id} benchmark"))
        .with_position([0., 0.])
        .with_inner_size([1920., 1080.])
        .with_decorations(false);
    let outcome = module_engine_native_ui::run_with_meshlets(
        root.clone(),
        viewport,
        r81 == Some(meshlets::PathUnderTest::Mesh),
        move |ctx, window, native_bridge| {
            let graphics = ui::window_graphics(window, &native_bridge);
            ui::record_window_mode(native_bridge.present_mode);
            let mut renderer = native_bridge.graph_renderer(root)?;
            let bridge = Arc::new(GpuPresentationBridge::window(native_bridge));
            let mut meshlets = None;
            if let Some(path) = r81 {
                let device = renderer
                    .meshlet_device_report()
                    .map_err(|e| e.to_string())?;
                let mut report = report.lock().unwrap();
                report["r81_device"] = json!(device);
                report["vulkan_capabilities"] = json!(renderer.capabilities());
                write(&preflight_path, &report)?;
                if path == meshlets::PathUnderTest::Mesh {
                    let source = renderer
                        .build_meshlet_probe(std::mem::take(&mut fixture.scene))
                        .map_err(|e| e.to_string())?;
                    report["meshlet_build"] = json!(source.build_report());
                    meshlets = Some(source);
                } else {
                    meshlets::check_indirect_support(&renderer, &device, &report["fixture"])?;
                }
            }
            if id == "S4" && !renderer.capabilities().bindless_materials {
                fixture.use_legacy_s4_textures()?;
                report.lock().unwrap()["fixture"] = fixture.inventory.clone();
            }
            renderer.set_detailed_stats(false);
            renderer.set_frame_profiling(frame_profiling);
            renderer.set_scene_culling(culling);
            renderer.set_culling_stats(s3 || r81 == Some(meshlets::PathUnderTest::Indirect));
            // S3 measures overlap. S4 and S5 keep the original one-request
            // protocol and S5's single expanded-stream memory preflight.
            let frame_window = if s3 { renderer.frame_capacity() } else { 1 };
            let probe = r81
                .is_none()
                .then(|| renderer.probe_vertex_allocation(triangles));
            let mut blocked = probe.as_ref().and_then(|probe| probe.blocked.clone());
            if id == "S5" {
                let probe = probe.as_ref().unwrap();
                let available = std::env::var("MODULE_ENGINE_BENCH_AVAILABLE_RAM_BYTES")
                    .ok()
                    .and_then(|value| value.parse::<u64>().ok());
                // Indexed source and frame images coexist with the host-visible
                // Vulkan allocation and expanded stream.
                let required = probe.requested_bytes + probe.requested_bytes / 2 + 1024_u64.pow(3);
                report.lock().unwrap()["system_memory_preflight"] = json!({"available_bytes_before_load":available,"estimated_required_bytes":required});
                if blocked.is_none() && available.is_none_or(|bytes| bytes < required) {
                    blocked = Some(
                        "S5 requires sufficient reported RAM/commit headroom; check system memory and launch with scripts/bench.py".into(),
                    );
                }
            }
            let window = window.clone();
            let scale = window.scale_factor() as f32;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                SIZE[0] as f32 / scale,
                SIZE[1] as f32 / scale,
            )));
            {
                let mut report = report.lock().unwrap();
                report["gui_graphics"] = json!(graphics);
                if let Some(probe) = probe {
                    report["legacy_vertex_allocation"] = json!(probe);
                }
                report["vulkan_capabilities"] = json!(renderer.capabilities());
                report["monitor"] = json!(window.current_monitor().map(|m| json!({
                    "pixels":<[u32;2]>::from(m.size()),"refresh_millihertz":m.refresh_rate_millihertz(),
                    "scale":m.scale_factor(),
                })));
                write(&preflight_path, &report)?;
            }
            Ok(Box::new(SceneApp {
                fixture,
                retained,
                meshlets,
                r81,
                static_s3,
                renderer,
                bridge,
                blocked,
                warmup,
                frames,
                objects,
                triangles,
                report,
                window_state: Box::new(move || (window.inner_size().into(), window.is_minimized())),
                graph: default_graph(RenderPath::Forward),
                texture: GpuTexture::default(),
                started: Instant::now(),
                completed: 0,
                submitted: 0,
                in_flight: std::collections::VecDeque::new(),
                retry_tick: None,
                max_pending: 0,
                frame_window,
                last_ui: None,
                last_painted: None,
                samples: Vec::new(),
                initial_work: None,
                finished: false,
            }))
        },
    );
    let mut report = returned.lock().unwrap();
    let present_mode = ui::benchmark_present_mode();
    report["effective_present_mode"] = json!(present_mode.effective_present_mode);
    report["effective_present_mode_unavailable_reason"] =
        json!(present_mode.effective_present_mode_unavailable_reason);
    report["observed_present_modes"] = json!(present_mode.observed_present_modes);
    if let Err(error) = outcome {
        report["status"] = json!("failed");
        report["error"] = json!(error);
    }
    write(&path, &report)?;
    if report["status"] == "complete" || report["status"] == "blocked" {
        Ok(())
    } else {
        Err(report["error"]
            .as_str()
            .unwrap_or("Scene window closed before completing")
            .into())
    }
}

#[derive(serde::Serialize)]
struct Sample {
    frame_interval_ms: f64,
    cpu_submit_ms: f64,
    ecs_tick_ms: Option<f64>,
    ecs_query_ms: Option<f64>,
    ecs_motion_update_ms: Option<f64>,
    transform_extract_ms: Option<f64>,
    scene_update_ms: f64,
    cpu_scene_frame_ms: f64,
    cpu_fixture_frame_ms: f64,
    observed_producer_latency_ms: f64,
    work: FrameWork,
    timings: Option<Box<FrameTimings>>,
}

struct Flight {
    ticket: u64,
    started: Instant,
    cpu_submit_ms: f64,
    tick: TickWork,
}

#[derive(Default)]
struct TickWork {
    ecs: Option<fixtures::EcsWork>,
    scene_update_ms: f64,
}

struct SceneApp {
    fixture: Fixture,
    retained: Option<ResidentScene>,
    meshlets: Option<MeshletScene>,
    r81: Option<meshlets::PathUnderTest>,
    static_s3: bool,
    graph: Graph,
    renderer: VulkanRenderer,
    bridge: Arc<GpuPresentationBridge>,
    texture: GpuTexture,
    window_state: Box<dyn Fn() -> ([u32; 2], Option<bool>)>,
    blocked: Option<String>,
    started: Instant,
    warmup: usize,
    frames: usize,
    objects: usize,
    triangles: usize,
    completed: usize,
    submitted: usize,
    in_flight: std::collections::VecDeque<Flight>,
    retry_tick: Option<TickWork>,
    max_pending: usize,
    frame_window: usize,
    last_painted: Option<Instant>,
    last_ui: Option<u64>,
    samples: Vec<Sample>,
    initial_work: Option<FrameWork>,
    report: Arc<Mutex<Value>>,
    finished: bool,
}

impl SceneApp {
    fn finish(&mut self, status: &str, error: Option<&str>) {
        self.finished = true;
        let mut report = self.report.lock().unwrap();
        let counters = self.renderer.performance();
        report["status"] = json!(status);
        report["error"] = json!(error);
        report["warmup_completed"] = json!(self.completed.min(self.warmup));
        report["measured_completed"] = json!(self.samples.len());
        report["gpu_submissions"] = json!(counters.gpu_submissions);
        report["gpu_completed_frames"] = json!(counters.gpu_presentation_frames);
        report["cpu_pixel_readback_bytes"] = json!(counters.cpu_readback_bytes);
        report["window_pixels_at_close"] = json!((self.window_state)().0);
        report["presentation"] = json!(self.texture.counters());
        report["first_completed_frame_work"] = json!(self.initial_work);
        report["samples"] = json!(self.samples);
        report["producer_frame_slots"] = json!(self.renderer.frame_capacity());
        report["producer_max_pending"] = json!(self.max_pending);
        report["producer_allocator"] = json!(self.renderer.allocator_stats().map(
            |[blocks, used, reserved]| json!({"blocks": blocks, "used_bytes": used, "reserved_bytes": reserved})
        ));
    }

    fn observe(&mut self, now: Instant) -> Result<(), String> {
        if let Some(flight) = self.in_flight.pop_front() {
            let Some(mut frame) = self
                .renderer
                .poll_presented(flight.ticket)
                .map_err(|error| error.to_string())?
            else {
                self.in_flight.push_front(flight);
                return Ok(());
            };
            if frame.stats.objects != self.objects
                || frame.stats.submitted_triangles != self.triangles
                || !frame.is_valid()
                || frame.gpu.is_none()
            {
                return Err("Rendered frame does not match the loaded native workload".into());
            }
            if self.texture.update(&self.bridge, &frame)?.is_none() {
                return Err("Native bridge did not accept the completed frame".into());
            }
            let previous = self.last_painted.replace(now);
            let work = self
                .renderer
                .last_frame_work()
                .ok_or("No workload counters for completed submission")?;
            if let Some(path) = self.r81 {
                path.check_work(&work, self.triangles, self.completed == 0)?;
                if self.completed == 0 {
                    let mut report = self.report.lock().unwrap();
                    report["first_submission_ms"] = json!(flight.cpu_submit_ms);
                    report["first_producer_latency_ms"] =
                        json!(now.duration_since(flight.started).as_secs_f64() * 1000.);
                }
            }
            if self.completed == 0 {
                self.initial_work = Some(work);
            }
            if self.completed >= self.warmup {
                let (pixels, minimized) = (self.window_state)();
                if pixels != SIZE.map(|size| size as u32) || minimized == Some(true) {
                    return Err(format!(
                        "Measured native window must remain visible at {SIZE:?}; got {pixels:?}"
                    ));
                }
                if let Some(timings) = &mut frame.timings
                    && let Some(ecs) = flight.tick.ecs
                {
                    timings.cpu_tick_ms = Some(ecs.tick_ms);
                    timings.cpu_extract_ms = Some(ecs.extract_ms);
                }
                self.samples.push(Sample {
                    frame_interval_ms: now
                        .duration_since(previous.ok_or("Missing warmup observation")?)
                        .as_secs_f64()
                        * 1000.,
                    cpu_submit_ms: flight.cpu_submit_ms,
                    scene_update_ms: flight.tick.scene_update_ms,
                    cpu_scene_frame_ms: flight.tick.scene_update_ms + flight.cpu_submit_ms,
                    cpu_fixture_frame_ms: flight.tick.scene_update_ms
                        + flight.cpu_submit_ms
                        + flight
                            .tick
                            .ecs
                            .map_or(0., |ecs| ecs.tick_ms + ecs.extract_ms),
                    ecs_tick_ms: flight.tick.ecs.map(|ecs| ecs.tick_ms),
                    ecs_query_ms: flight.tick.ecs.map(|ecs| ecs.query_ms),
                    ecs_motion_update_ms: flight.tick.ecs.map(|ecs| ecs.tick_ms - ecs.query_ms),
                    transform_extract_ms: flight.tick.ecs.map(|ecs| ecs.extract_ms),
                    observed_producer_latency_ms: now.duration_since(flight.started).as_secs_f64()
                        * 1000.,
                    work,
                    timings: frame.timings,
                });
            }
            self.completed += 1;
            // Submit the next frame in this same callback. Waiting for another
            // UI callback would add one display interval to every sample.
        }
        Ok(())
    }

    fn advance(&mut self) -> Result<(), String> {
        if self.started.elapsed() > Duration::from_secs(300) {
            return Err("Native scene watchdog expired".into());
        }
        let now = Instant::now();
        self.observe(now)?;
        if self.completed == self.warmup + self.frames {
            if self.texture.counters().pending_copies == 0 {
                self.finish("complete", None);
            }
            return Ok(());
        }
        while self.submitted < self.warmup + self.frames && self.in_flight.len() < self.frame_window
        {
            let tick = if let Some(retry) = self.retry_tick.take() {
                retry
            } else if self.static_s3 || self.r81.is_some() {
                TickWork::default()
            } else {
                let ecs = self.fixture.tick(self.submitted);
                let scene_update_ms = if let Some(retained) = &mut self.retained {
                    let started = Instant::now();
                    // S3 owns a standalone draw list, separate from GameHost.
                    let edits: Vec<_> = self.fixture.scene.items[..s3_ecs::MOVING]
                        .iter()
                        .map(|item| SceneChange {
                            id: item.id,
                            kind: SceneChangeKind::Edited {
                                moved: true,
                                material_changed: false,
                                mesh_changed: false,
                            },
                            items: std::slice::from_ref(item),
                        })
                        .collect();
                    retained.apply(SceneUpdate::Delta {
                        base: self.submitted as u64,
                        revision: self.submitted as u64 + 1,
                        time: self.fixture.scene.time,
                        changes: &edits,
                    })?;
                    started.elapsed().as_secs_f64() * 1000.
                } else {
                    0.
                };
                TickWork {
                    ecs,
                    scene_update_ms,
                }
            };
            let started = Instant::now();
            let submission = self
                .renderer
                .render_scene_presented(
                    match (&mut self.retained, &mut self.meshlets) {
                        (Some(scene), _) => SceneInput::Retained(scene),
                        (_, Some(scene)) => SceneInput::Meshlets(scene),
                        _ => SceneInput::Snapshot(&self.fixture.scene),
                    },
                    self.fixture.camera,
                    SIZE[0],
                    SIZE[1],
                    &self.graph,
                    &BTreeMap::new(),
                    &RenderOverrides::default(),
                )
                .map_err(|error| error.to_string())?;
            let cpu_submit_ms = started.elapsed().as_secs_f64() * 1000.;
            match submission {
                RenderSubmission::Pending(ticket) => {
                    self.in_flight.push_back(Flight {
                        ticket,
                        started,
                        cpu_submit_ms,
                        tick,
                    });
                    self.submitted += 1;
                    self.max_pending = self.max_pending.max(self.in_flight.len());
                }
                RenderSubmission::Busy => {
                    self.retry_tick = Some(tick);
                    break;
                }
                RenderSubmission::Ready(_) => {
                    return Err("Expected asynchronous Vulkan frame submission".into());
                }
            }
        }
        Ok(())
    }
}

impl module_engine_native_ui::NativeApp for SceneApp {
    fn ui(&mut self, ui: &mut egui::Ui, _: &module_engine_native_ui::Window) {
        let ctx = ui.ctx().clone();
        let ui_frame = ctx.cumulative_frame_nr();
        if self.last_ui.replace(ui_frame) != Some(ui_frame) && !self.finished {
            if let Some(reason) = self.blocked.take() {
                self.finish("blocked", Some(&reason));
            } else if let Err(error) = self.advance() {
                self.finish("failed", Some(&error));
            }
        }
        if let Some(texture) = self.texture.texture_id() {
            let rect = ui.max_rect();
            ui.painter().image(
                texture,
                rect,
                egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(1., 1.)),
                egui::Color32::WHITE,
            );
        }
        if self.finished {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            ctx.request_repaint();
        }
    }
}

impl Drop for SceneApp {
    fn drop(&mut self) {
        if !self.finished {
            self.finish("incomplete", Some("Scene window closed before completing"));
        }
    }
}
