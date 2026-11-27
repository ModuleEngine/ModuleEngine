//! Explicit offscreen native verification, never an interactive fallback.
//! Usage: gpu_presentation_check <report.json> [--png | --window]
use eframe::egui;
use module_engine_core::{
    Camera, Frame, FrameRenderer, Graph, GraphDocument, ModuleRegistry, ObjectsWorld,
    RenderOverrides, RenderSubmission, SceneChange, SceneChangeKind, SceneSnapshot, SceneUpdate,
    TextureImage, WorldKind, WorldProvider, demo_document,
};
use module_engine_native_ui::Offscreen;
use module_engine_player::gpu_presentation::{
    GpuPresentationBridge, GpuTexture, finish_gpu_retirements,
};
use module_engine_vulkan::WindowTarget;
use module_engine_vulkan::{
    RenderPath, ResidentScene, SceneInput, VulkanRenderer, builtin_hlsl_source, builtin_manifest,
    default_graph, remove_pass_with_bypass,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const WATCHDOG_SECONDS: u64 = 180;

fn ensure(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

// Submit exactly once. Polling must never admit a replacement request, even
// when a very fast device has already completed the accepted submission.
fn finish_submission(
    renderer: &mut dyn FrameRenderer,
    submission: RenderSubmission,
) -> Result<Frame, String> {
    let ticket = match submission {
        RenderSubmission::Ready(_) => {
            return Err("Native fixture unexpectedly completed synchronously".into());
        }
        RenderSubmission::Busy => {
            return Err("Native fixture unexpectedly rejected admission".into());
        }
        RenderSubmission::Pending(ticket) => ticket,
    };
    finish_ticket(renderer, ticket)
}

fn finish_ticket(renderer: &mut dyn FrameRenderer, ticket: u64) -> Result<Frame, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(frame) = renderer.poll_presented(ticket)? {
            ensure(
                renderer.poll_presented(ticket)?.is_none(),
                "Consumed ticket published a duplicate frame",
            )?;
            return Ok(frame);
        }
        ensure(
            Instant::now() < deadline,
            "Producer completion poll exceeded 10 seconds",
        )?;
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn presented(
    renderer: &mut dyn FrameRenderer,
    scene: &SceneSnapshot,
    camera: Camera,
    size: [usize; 2],
    controls: &RenderOverrides,
) -> Result<Frame, String> {
    let before = renderer.performance();
    let submission = renderer.render_presented(scene, camera, size[0], size[1], controls)?;
    ensure(
        renderer.performance().gpu_submissions == before.gpu_submissions + 1
            && renderer.performance().gpu_presentation_frames == before.gpu_presentation_frames,
        "Admission counters did not distinguish submitted and completed work",
    )?;
    let frame = finish_submission(renderer, submission)?;
    ensure(
        renderer.performance().gpu_submissions == before.gpu_submissions + 1
            && renderer.performance().gpu_completion_polls > before.gpu_completion_polls
            && renderer.performance().gpu_presentation_frames == before.gpu_presentation_frames + 1,
        "Polling resubmitted work or failed to count real completion",
    )?;
    Ok(frame)
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let Some(report_path) = args.first().map(PathBuf::from) else {
        eprintln!("Usage: gpu_presentation_check <report.json> [--png | --window]");
        std::process::exit(2);
    };
    if args.iter().any(|arg| arg == "--window") {
        if let Err(error) = window_check::run(&report_path) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let png = args.iter().skip(1).any(|arg| arg == "--png");
    if let Some(parent) = report_path.parent().filter(|p| !p.as_os_str().is_empty())
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        eprintln!("Cannot create report directory: {error}");
        std::process::exit(2);
    }
    let report = Arc::new(Mutex::new(json!({
        "passed": false, "fixture": "production_gpu_presentation_offscreen",
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "watchdog_seconds": WATCHDOG_SECONDS,
        "scope": "Production Vulkan FrameRenderer output sampled through the native egui painter into an offscreen RGBA8 attachment. Explicit diagnostic readback. Window presentation is checked separately with --window.",
        "cases": [],
    })));
    let (finished, watchdog) = std::sync::mpsc::sync_channel::<()>(1);
    let timeout_report = report.clone();
    let timeout_path = report_path.clone();
    let watchdog_thread = std::thread::spawn(move || {
        if watchdog
            .recv_timeout(Duration::from_secs(WATCHDOG_SECONDS))
            .is_err()
        {
            if let Ok(mut report) = timeout_report.lock() {
                report["error"] = json!("Native verification watchdog expired");
                report["timeout"] = json!(true);
                let _ = std::fs::write(
                    timeout_path,
                    serde_json::to_vec_pretty(&*report).unwrap_or_default(),
                );
            }
            std::process::exit(124);
        }
    });
    let started = Instant::now();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(&report, &report_path, png)
    }))
    .unwrap_or_else(|_| Err("Native verification panicked; see process stderr".into()));
    // run's fixture/panes are gone, including on failure. A zero pending-copy
    // counter alone does not establish that the retirement thread has exited.
    let retirement = finish_gpu_retirements();
    let retirement_report = match &retirement {
        Ok(report) => report,
        Err(error) => &error.report,
    };
    report.lock().expect("report mutex")["gpu_retirement"] = json!(retirement_report);
    let producer = &retirement_report.producer;
    let producer_retired = producer.registered_jobs > 0
        && producer.joined_jobs == producer.registered_jobs
        && producer.completed_submissions == producer.registered_jobs
        && producer.unconfirmed_submissions == 0
        && producer.failed_jobs == 0;
    let result = result.and_then(|orphan_frame| {
        // All run-local GUI owners and all registered completion jobs are gone.
        // This frame now owns the final device pin through its native payload.
        if let Err(error) = retirement {
            drop(orphan_frame);
            return Err(error.to_string());
        }
        ensure(producer_retired, "Pending producer destruction was not fully retired at shutdown")?;
        ensure(orphan_frame.gpu.is_some(), "Orphan lifetime fixture requires a GPU frame")?;
        drop(orphan_frame);
        record(
            &report,
            json!({"case":"final_published_frame_outlives_gui_owners","passed":true,"gui_owners_dropped":true,"retirement_jobs_joined_before_frame_drop":true,"orphan_frame_dropped":true}),
        );
        Ok(())
    });
    let mut output = report.lock().expect("report mutex");
    output["wall_seconds"] = json!(started.elapsed().as_secs_f64());
    output["passed"] = json!(result.is_ok());
    if let Err(error) = &result {
        output["error"] = json!(error);
    }
    let write = std::fs::write(
        &report_path,
        serde_json::to_vec_pretty(&*output).expect("serializable report"),
    );
    drop(output);
    let _ = finished.send(());
    let _ = watchdog_thread.join();
    if let Err(error) = write {
        eprintln!("Write verification report: {error}");
        std::process::exit(2);
    }
    match result {
        Ok(()) => println!(
            "GPU presentation verification passed: {}",
            report_path.display()
        ),
        Err(error) => {
            eprintln!("GPU presentation verification failed: {error}");
            std::process::exit(1);
        }
    }
}

struct Fixture<'a> {
    consumer: Offscreen,
    registry: ModuleRegistry,
    scene: SceneSnapshot,
    bridge: GpuPresentationBridge,
    display: GpuTexture,
    report: &'a Arc<Mutex<Value>>,
    png_folder: Option<PathBuf>,
}
fn record(report: &Arc<Mutex<Value>>, case: Value) {
    report.lock().expect("report mutex")["cases"]
        .as_array_mut()
        .expect("case list")
        .push(case);
}

fn run(report: &Arc<Mutex<Value>>, report_path: &Path, png: bool) -> Result<Frame, String> {
    let mut registry = ModuleRegistry::builtin();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let consumer = Offscreen::new(&root)?;
    let endpoint = consumer.bridge();
    report.lock().expect("report mutex")["gui"] =
        json!({"backend":"Vulkan","adapter": endpoint.adapter,"surface": false});
    let bridge = GpuPresentationBridge::window(endpoint);
    ensure(
        Arc::ptr_eq(&bridge.target(), &bridge.target()),
        "Unstable presentation device",
    )?;
    module_engine_vulkan::install(&mut registry, &root).map_err(|error| error.to_string())?;
    let png_folder = png.then(|| {
        report_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("diagnostic-png")
    });
    if let Some(folder) = &png_folder {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    let mut fixture = Fixture {
        consumer,
        registry,
        scene: ObjectsWorld::demo().snapshot(),
        bridge,
        display: GpuTexture::default(),
        report,
        png_folder,
    };
    for (label, path) in [
        ("forward", RenderPath::Forward),
        ("deferred", RenderPath::Deferred),
    ] {
        let initial_document = document(default_graph(path), BTreeMap::new());
        let mut renderer = fixture.renderer(&initial_document)?;
        fixture.compare(
            &format!("{label}_default"),
            &mut *renderer,
            [96, 64],
            &RenderOverrides::default(),
            false,
        )?;
        // Native-window acceptance exposed a size-dependent cross-API import
        // rejection that the small fixtures could not detect. Compare the exact
        // required framebuffer and a neighboring nonaligned extent on both paths.
        for (extent_label, size) in [
            ("full_hd", [1920, 1080]),
            ("full_hd_unaligned", [1919, 1079]),
        ] {
            fixture.compare(
                &format!("{label}_{extent_label}"),
                &mut *renderer,
                size,
                &RenderOverrides::default(),
                false,
            )?;
        }
        let mut disabled = RenderOverrides::default();
        disabled.enabled.insert("ao".into(), false);
        disabled.enabled.insert("aa".into(), false);
        fixture.compare(
            &format!("{label}_ao_aa_disabled"),
            &mut *renderer,
            [96, 64],
            &disabled,
            false,
        )?;
        let mut graph = default_graph(path);
        ensure(
            remove_pass_with_bypass(&mut graph, "ao") == Some(1),
            "AO bypass did not preserve one color consumer",
        )?;
        ensure(
            remove_pass_with_bypass(&mut graph, "aa") == Some(1),
            "AA bypass did not preserve one color consumer",
        )?;
        let replacement = document(graph, BTreeMap::new());
        renderer = renderer
            .prepare_candidate(renderer_node(&replacement)?, &replacement)?
            .ok_or("Vulkan candidate did not reuse production renderer")?;
        fixture.compare(
            &format!("{label}_ao_aa_removed_candidate"),
            &mut *renderer,
            [97, 63],
            &RenderOverrides::default(),
            false,
        )?;
    }
    let mut alpha_document = custom_alpha_document(RenderPath::Forward)?;
    let mut renderer = fixture.renderer(&alpha_document)?;
    fixture.compare(
        "custom_final_alpha",
        &mut *renderer,
        [97, 63],
        &RenderOverrides::default(),
        true,
    )?;
    fixture.compare(
        "resize_up",
        &mut *renderer,
        [163, 101],
        &RenderOverrides::default(),
        true,
    )?;
    fixture.compare(
        "resize_down",
        &mut *renderer,
        [97, 63],
        &RenderOverrides::default(),
        true,
    )?;
    let previous_id = fixture.display.texture_id();
    let previous_pixels = capture(&mut fixture.consumer, &fixture.display)?;
    let mut invalid = alpha_document.clone();
    invalid.shader_sources.insert(
        "fixture-final-alpha".into(),
        "invalid HLSL fixture candidate".into(),
    );
    let rejection = match renderer.prepare_candidate(renderer_node(&invalid)?, &invalid) {
        Err(error) => error,
        Ok(_) => return Err("Invalid shader candidate was accepted".into()),
    };
    ensure(
        fixture.display.texture_id() == previous_id
            && capture(&mut fixture.consumer, &fixture.display)? == previous_pixels,
        "Rejected graph changed last good GUI display",
    )?;
    fixture.compare(
        "last_good_after_rejected_shader",
        &mut *renderer,
        [97, 63],
        &RenderOverrides::default(),
        true,
    )?;
    record(
        report,
        json!({"case":"rejected_shader_preserves_display","passed":true,"diagnostic":rejection.to_string()}),
    );

    let previous_id = fixture.display.texture_id();
    let leased = presented(
        &mut *renderer,
        &fixture.scene,
        Camera::default(),
        [97, 63],
        &RenderOverrides::default(),
    )?;
    let other_device = Offscreen::new(&root)?;
    let other_bridge = GpuPresentationBridge::window(other_device.bridge());
    let target_error = renderer
        .set_presentation_target(Some(other_bridge.target()))
        .err()
        .ok_or("Different target accepted while source lease was live")?;
    renderer.set_presentation_target(Some(fixture.bridge.target()))?;
    ensure(
        fixture.display.texture_id() == previous_id
            && capture(&mut fixture.consumer, &fixture.display)? == previous_pixels,
        "Rejected target changed last good GUI display",
    )?;
    drop(leased);
    record(
        report,
        json!({"case":"rejected_target_preserves_endpoint_and_display","passed":true,"diagnostic":target_error.to_string()}),
    );

    let foreign_frame = presented(
        &mut *renderer,
        &fixture.scene,
        Camera::default(),
        [96, 64],
        &RenderOverrides::default(),
    )?;
    let old_id = fixture.display.texture_id();
    ensure(
        fixture
            .display
            .update(&other_bridge, &foreign_frame)
            .is_err()
            && fixture.display.texture_id() == old_id,
        "Wrong logical device accepted a frame or changed the retained display",
    )?;
    drop(foreign_frame);
    record(
        report,
        json!({"case":"foreign_logical_device_rejected","passed":true}),
    );

    fixture.pool_check(&alpha_document)?;
    fixture.async_check(&alpha_document, &other_bridge)?;
    fixture.material_replacement_check(&root, false)?;
    fixture.material_replacement_check(&root, true)?;
    // Publishing a replacement shares the actual backend; replacing authored
    // configuration never substitutes synthetic pixels for graph output.
    alpha_document = custom_alpha_document(RenderPath::Deferred)?;
    renderer = renderer
        .prepare_candidate(renderer_node(&alpha_document)?, &alpha_document)?
        .ok_or("Deferred replacement candidate missing")?;
    fixture.compare(
        "deferred_custom_graph_replacement",
        &mut *renderer,
        [97, 63],
        &RenderOverrides::default(),
        true,
    )?;
    drop(renderer);
    fixture.drop_check(&alpha_document)?;
    let mut recreated = fixture.renderer(&alpha_document)?;
    fixture.compare(
        "renderer_recreated_after_stop",
        &mut *recreated,
        [97, 63],
        &RenderOverrides::default(),
        true,
    )?;
    report.lock().expect("report mutex")["bridge_counters"] =
        serde_json::to_value(fixture.bridge.counters()).map_err(|e| e.to_string())?;
    let counters = fixture.bridge.counters();
    ensure(
        counters.cpu_readback_bytes == 0 && counters.cpu_upload_bytes == 0,
        "Normal bridge path transferred CPU pixels",
    )?;
    ensure(
        counters.pending_copies == 0,
        "Verification ended with pending consumer copies",
    )?;
    // Return only this final published frame. The renderer, fixture/display,
    // bridge, RenderState and GUI instance all leave scope before its Drop.
    presented(
        &mut *recreated,
        &fixture.scene,
        Camera::default(),
        [97, 63],
        &RenderOverrides::default(),
    )
}

fn document(graph: Graph, sources: BTreeMap<String, String>) -> GraphDocument {
    let mut document = demo_document(WorldKind::Objects);
    let node = document
        .root
        .nodes
        .iter_mut()
        .find(|n| n.id == "renderer")
        .expect("demo renderer");
    node.module = "render.vulkan".into();
    node.params = json!({});
    node.subgraph = Some(Box::new(graph));
    document.shader_sources = sources;
    document
}
fn renderer_node(document: &GraphDocument) -> Result<&module_engine_core::Node, String> {
    document
        .root
        .nodes
        .iter()
        .find(|node| node.id == "renderer")
        .ok_or_else(|| "Fixture renderer node missing".into())
}
fn custom_alpha_document(path: RenderPath) -> Result<GraphDocument, String> {
    let mut graph = default_graph(path);
    let encode = graph
        .nodes
        .iter_mut()
        .find(|node| node.id == "encode")
        .ok_or("Encode node missing")?;
    encode.module = "gpu.custom".into();
    encode.params = json!({"source_id":"fixture-final-alpha","manifest":builtin_manifest("gpu.color_encode").ok_or("Color encode manifest missing")?});
    let original = builtin_hlsl_source("gpu.color_encode").ok_or("Color encode HLSL missing")?;
    let store = "frame[tid.xy]=float4(encoded,1.0);";
    ensure(
        original.contains(store),
        "Fixture must adapt the actual builtin final store",
    )?;
    let source = original.replace(store, "float alpha = tid.x % 3 == 0 ? 0.0 : (tid.x % 3 == 1 ? 128.0 / 255.0 : 1.0);\n    frame[tid.xy] = float4(max(encoded, float3(0.25,0.5,0.75)), alpha);");
    Ok(document(
        graph,
        BTreeMap::from([("fixture-final-alpha".into(), source)]),
    ))
}

impl Fixture<'_> {
    fn material_replacement_check(&mut self, root: &Path, resident: bool) -> Result<(), String> {
        // The FrameRenderer adapter admits one pending request per caller.
        // Exercise the same direct two-slot producer used by the native S3 window.
        let mut renderer = VulkanRenderer::new_at(root.to_owned()).map_err(|e| e.to_string())?;
        let target = self.bridge.target();
        let target = target
            .as_any()
            .downcast_ref::<WindowTarget>()
            .ok_or("Expected shared Vulkan device")?;
        renderer
            .set_window_target(target)
            .map_err(|e| e.to_string())?;
        let case = if resident {
            "resident_scene_replacement"
        } else {
            "bindless_material_replacement"
        };
        if !renderer.capabilities().bindless_materials
            || resident && !renderer.capabilities().resident_scene
        {
            record(
                self.report,
                json!({"case":case, "skipped":"Adapter lacks the required material/scene capabilities"}),
            );
            return Ok(());
        }
        ensure(
            renderer.frame_capacity() == 2 && self.scene.items.len() >= 2,
            "Replacement requires two slots and two material users",
        )?;
        let solid = |color: [u8; 4]| {
            Arc::new(TextureImage {
                width: 9,
                height: 5,
                rgba8: color.repeat(45),
            })
        };
        let mut a = self.scene.clone();
        let original = solid([239, 18, 62, 255]);
        for item in &mut a.items {
            item.material.base_color_texture = Some(original.clone());
        }
        a.items.sort_by_key(|item| item.id);
        let mut b = a.clone();
        b.items[0].material.base_color_texture = Some(solid([35, 238, 69, 255]));
        if resident {
            for vertex in &mut b.items[0].mesh.vertices {
                vertex[1] *= 0.65;
            }
            b.items[0].model[0][0] *= -0.85;
            b.items[0].model[0][3] += 0.3;
        }
        let camera = Camera::default();
        let size = [160, 96];
        let graph = default_graph(RenderPath::Forward);
        let sources = BTreeMap::new();
        let controls = RenderOverrides::default();
        let mut reference = VulkanRenderer::new_at(root.to_owned()).map_err(|e| e.to_string())?;
        let expected = [&a, &b].map(|scene| {
            reference
                .render(scene, camera, size[0], size[1], &graph, &sources, &controls)
                .map(|frame| {
                    egui::ColorImage::from_rgba_unmultiplied(size, &frame.rgba)
                        .pixels
                        .iter()
                        .flat_map(|p| p.to_array())
                        .collect::<Vec<_>>()
                })
                .map_err(|e| e.to_string())
        });
        let [expected_a, expected_b] = expected;
        let (expected_a, expected_b) = (expected_a?, expected_b?);
        ensure(
            expected_a != expected_b,
            "Material replacement positive control did not change pixels",
        )?;
        let mut submit = |scene: SceneInput<'_>| match renderer
            .render_scene_presented(scene, camera, size[0], size[1], &graph, &sources, &controls)
            .map_err(|e| e.to_string())?
        {
            RenderSubmission::Pending(ticket) => Ok(ticket),
            _ => Err("Both material generations must be admitted before polling".to_owned()),
        };
        let mut retained = ResidentScene::default();
        let (a_ticket, b_ticket) = if resident {
            retained.apply(SceneUpdate::Reset {
                revision: 0,
                snapshot: a.clone(),
            })?;
            let a_ticket = submit(SceneInput::Retained(&mut retained))?;
            retained.apply(SceneUpdate::Delta {
                base: 0,
                revision: 1,
                time: 0.,
                changes: &[SceneChange {
                    id: b.items[0].id,
                    kind: SceneChangeKind::Edited {
                        moved: true,
                        material_changed: true,
                        mesh_changed: true,
                    },
                    items: &b.items[..1],
                }],
            })?;
            (a_ticket, submit(SceneInput::Retained(&mut retained))?)
        } else {
            (
                submit(SceneInput::Snapshot(&a))?,
                submit(SceneInput::Snapshot(&b))?,
            )
        };
        let mut display = GpuTexture::default();
        for (ticket, expected, images, uploads) in
            [(b_ticket, &expected_b, 2, 1), (a_ticket, &expected_a, 1, 2)]
        {
            let deadline = Instant::now() + Duration::from_secs(10);
            let frame = loop {
                if let Some(frame) = renderer.poll_presented(ticket).map_err(|e| e.to_string())? {
                    break frame;
                }
                ensure(
                    Instant::now() < deadline,
                    "Material generation completion timed out",
                )?;
                std::thread::sleep(Duration::from_millis(1));
            };
            let work = renderer
                .last_frame_work()
                .ok_or("Missing material submission work")?;
            ensure(
                work.material_images == images
                    && work.material_uploaded_images == uploads
                    && work.atlas_upload_bytes == 0,
                format!("Material generations did not reuse their shared resident image: {work:?}"),
            )?;
            if resident && ticket == b_ticket {
                ensure(
                    work.vertex_write_bytes == b.items[0].mesh.vertices.len() * 20
                        && work.mesh_index_write_bytes == b.items[0].mesh.triangles.len() * 12
                        && work.instance_write_bytes == 64
                        && work.material_row_write_bytes == 4,
                    format!("Replacement rewrote unchanged scene data: {work:?}"),
                )?;
            }
            ensure(
                display.update(&self.bridge, &frame)?.is_some()
                    && capture(&mut self.consumer, &display)? == *expected,
                "Published material generation does not match its individual reference",
            )?;
            self.consumer.collect()?;
        }
        if resident {
            let repeat = |renderer: &mut VulkanRenderer, scene: &mut ResidentScene| match renderer
                .render_scene_presented(
                    SceneInput::Retained(scene),
                    camera,
                    size[0],
                    size[1],
                    &graph,
                    &sources,
                    &controls,
                )
                .map_err(|e| e.to_string())?
            {
                RenderSubmission::Pending(ticket) => Ok::<_, String>(ticket),
                _ => Err("Repeat should be accepted".into()),
            };
            let mut c = b.clone();
            c.items[0].model[0][3] += 0.9;
            let expected_c = reference
                .render(&c, camera, size[0], size[1], &graph, &sources, &controls)
                .map_err(|e| e.to_string())?;
            let expected_c: Vec<_> =
                egui::ColorImage::from_rgba_unmultiplied(size, &expected_c.rgba)
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_array())
                    .collect();
            ensure(
                expected_b != expected_c,
                "Cancelled transform positive control did not change pixels",
            )?;
            retained.apply(SceneUpdate::Delta {
                base: 1,
                revision: 2,
                time: 0.,
                changes: &[SceneChange {
                    id: c.items[0].id,
                    kind: SceneChangeKind::Edited {
                        moved: true,
                        material_changed: false,
                        mesh_changed: false,
                    },
                    items: &c.items[..1],
                }],
            })?;
            let cancelled = repeat(&mut renderer, &mut retained)?;
            ensure(
                renderer.last_frame_work().unwrap().instance_write_bytes == 64,
                "Cancelled request did not submit a resident transform update",
            )?;
            renderer
                .discard_presented(cancelled)
                .map_err(|e| e.to_string())?;
            let repeated = repeat(&mut renderer, &mut retained)?;
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                ensure(
                    renderer
                        .poll_presented(cancelled)
                        .map_err(|e| e.to_string())?
                        .is_none(),
                    "Cancelled retained frame published a lease",
                )?;
                if let Some(frame) = renderer
                    .poll_presented(repeated)
                    .map_err(|e| e.to_string())?
                {
                    let work = renderer.last_frame_work().unwrap();
                    ensure(
                        work.resident_scene
                            && work.vertex_write_bytes == 0
                            && work.mesh_index_write_bytes == 0
                            && work.instance_write_bytes == 0
                            && work.material_row_write_bytes == 0
                            && work.draw_write_bytes == 0
                            && work.material_upload_bytes == 0
                            && work.atlas_upload_bytes == 0,
                        format!("Repeat after cancellation rewrote scene data: {work:?}"),
                    )?;
                    ensure(
                        display.update(&self.bridge, &frame)?.is_some()
                            && capture(&mut self.consumer, &display)? == expected_c,
                        "Retained scene lost accepted updates after cancellation",
                    )?;
                    self.consumer.collect()?;
                    break;
                }
                ensure(Instant::now() < deadline, "Retained repeat timed out")?;
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        ensure(
            renderer.performance().cpu_readback_bytes == 0,
            "Async materials performed a producer readback",
        )?;
        record(
            self.report,
            json!({
                "case":case,
                "passed":true,
                "mesh_and_transform_replaced":resident,
                "zero_scene_bytes_after_cancel":resident,
                "admitted_before_polling":2,
                "first_slot_uploaded_images_including_white":2,
                "second_slot_uploaded_images":1,
                "second_slot_reused_material_images":1,
                "differing_bytes":0,
                "producer_cpu_readback_bytes":0,
            }),
        );
        Ok(())
    }

    fn async_check(
        &mut self,
        document: &GraphDocument,
        other: &GpuPresentationBridge,
    ) -> Result<(), String> {
        let controls = RenderOverrides::default();
        let mut scene_a = self.scene.clone();
        let texture_a = Arc::new(TextureImage {
            width: 2,
            height: 2,
            rgba8: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
            ],
        });
        for item in &mut scene_a.items {
            item.material.base_color_texture = Some(texture_a.clone());
        }
        let mut scene_b = scene_a.clone();
        let texture_b = Arc::new(TextureImage {
            width: 3,
            height: 1,
            rgba8: vec![0, 255, 255, 255, 255, 0, 255, 255, 32, 32, 32, 255],
        });
        for item in &mut scene_b.items {
            item.material.base_color_texture = Some(texture_b.clone());
            item.color = [255, 32, 64, 255];
            item.model[0][3] += 1.0;
        }
        scene_b.time += 3.0;
        // Keep diagnostic reference uploads on another producer. The native
        // producer below must upload its own atlas in the pending submission.
        let mut reference_renderer = self.renderer(document)?;
        let reference_a =
            reference_renderer.render(&scene_a, Camera::default(), 97, 63, &controls)?;
        let expected_a: Vec<u8> =
            egui::ColorImage::from_rgba_unmultiplied([97, 63], &reference_a.rgba)
                .pixels
                .iter()
                .flat_map(|p| p.to_array())
                .collect();
        drop(reference_renderer);
        let mut renderer = self.renderer(document)?;
        let before = renderer.performance();
        let ticket =
            match renderer.render_presented(&scene_a, Camera::default(), 97, 63, &controls)? {
                RenderSubmission::Pending(ticket) => ticket,
                _ => {
                    return Err(
                        "Async isolation fixture did not accept a pending native request".into(),
                    );
                }
            };
        let admitted = renderer.performance();
        ensure(
            admitted.gpu_submissions == before.gpu_submissions + 1
                && admitted.gpu_presentation_frames == before.gpu_presentation_frames,
            "Pending submission counters did not distinguish admission and publication",
        )?;
        ensure(
            matches!(
                renderer.render_presented(&scene_b, Camera::default(), 131, 79, &controls)?,
                RenderSubmission::Busy
            ),
            "Second request overwrote pending scene/atlas/extent",
        )?;
        ensure(
            renderer.performance().gpu_submissions == admitted.gpu_submissions
                && renderer.performance().gpu_busy_skips == admitted.gpu_busy_skips + 1,
            "Busy request submitted GPU work or failed to record backpressure",
        )?;
        ensure(
            renderer
                .render(&scene_b, Camera::default(), 131, 79, &controls)
                .is_err(),
            "CPU render overwrote pending GPU upload or resized its target",
        )?;
        let same_target_before = renderer.performance();
        renderer.set_presentation_target(Some(self.bridge.target()))?;
        ensure(
            renderer.performance() == same_target_before,
            "Same target reconfigured an in-flight renderer",
        )?;
        let rejection = renderer
            .set_presentation_target(Some(other.target()))
            .err()
            .ok_or("Target replaced while producer submission was pending")?;
        // No diagnostic readback or consumer submission has run since native
        // admission. These checks exercise logical flight even on a fast GPU.
        record(
            self.report,
            json!({"case":"async_pending_busy_and_target_preservation","passed":true,"native_pending":true,"second_submission_busy":true,"cpu_render_rejected":true,"same_target_idempotent":true,"different_target_rejected":true,"diagnostic":rejection.to_string(),"readback_before_completion":false}),
        );

        let mut invalid = document.clone();
        invalid.shader_sources.insert(
            "fixture-final-alpha".into(),
            "invalid HLSL pending candidate".into(),
        );
        ensure(
            renderer
                .prepare_candidate(renderer_node(&invalid)?, &invalid)
                .is_err(),
            "Invalid pending shader candidate was accepted",
        )?;
        let candidate_document = custom_alpha_document(RenderPath::Deferred)?;
        let candidate = renderer
            .prepare_candidate(renderer_node(&candidate_document)?, &candidate_document)?
            .ok_or("Valid graph candidate could not be prepared during producer flight")?;
        let frame_a = finish_ticket(&mut *renderer, ticket)?;
        ensure(
            frame_a.width == 97
                && frame_a.height == 63
                && frame_a.gpu.is_some()
                && frame_a.rgba.is_empty(),
            "Pending request lost original dimensions or GPU representation",
        )?;
        ensure(
            renderer.performance().gpu_presentation_frames == before.gpu_presentation_frames + 1
                && renderer.performance().cpu_readback_bytes == before.cpu_readback_bytes,
            "Pending request publication count or normal readback changed",
        )?;
        let mut display = GpuTexture::default();
        ensure(
            display.update(&self.bridge, &frame_a)?.is_some(),
            "Pending A display copy missing",
        )?;
        ensure(
            capture(&mut self.consumer, &display)? == expected_a,
            "Scene/atlas/resize request or rejected candidate corrupted in-flight A pixels",
        )?;
        self.consumer.collect()?;
        drop(frame_a);
        record(
            self.report,
            json!({"case":"async_candidates_preserve_inflight_pixels","passed":true,"invalid_candidate_rejected":true,"valid_candidate_prepared":true,"differing_bytes":0,"exact_ticket_completed_once":true,"normal_cpu_readback_bytes":renderer.performance().cpu_readback_bytes-before.cpu_readback_bytes}),
        );
        let changed_scene = renderer.render(&scene_b, Camera::default(), 97, 63, &controls)?;
        let changed_scene_pixels: Vec<u8> =
            egui::ColorImage::from_rgba_unmultiplied([97, 63], &changed_scene.rgba)
                .pixels
                .iter()
                .flat_map(|p| p.to_array())
                .collect();
        ensure(
            expected_a != changed_scene_pixels,
            "Scene/material positive control did not change same-size pixels",
        )?;
        renderer = candidate;

        let reference_b = renderer.render(&scene_b, Camera::default(), 131, 79, &controls)?;
        let expected_b: Vec<u8> =
            egui::ColorImage::from_rgba_unmultiplied([131, 79], &reference_b.rgba)
                .pixels
                .iter()
                .flat_map(|p| p.to_array())
                .collect();
        ensure(
            expected_a != expected_b,
            "Scene/atlas/extent isolation control is unchanged",
        )?;
        let before_b = renderer.performance();
        let frame_b = presented(
            &mut *renderer,
            &scene_b,
            Camera::default(),
            [131, 79],
            &controls,
        )?;
        ensure(
            frame_b.width == 131 && frame_b.height == 79,
            "Next async request retained previous extent",
        )?;
        ensure(
            display.update(&self.bridge, &frame_b)?.is_some()
                && capture(&mut self.consumer, &display)? == expected_b,
            "Next scene/atlas/resize did not match its own reference",
        )?;
        self.consumer.collect()?;
        drop(frame_b);
        ensure(
            renderer.performance().cpu_readback_bytes == before_b.cpu_readback_bytes
                && renderer.performance().gpu_presentation_frames
                    == before_b.gpu_presentation_frames + 1,
            "Scene B publication used CPU pixels or duplicated output",
        )?;
        record(
            self.report,
            json!({"case":"async_scene_atlas_resize_upload_isolation","passed":true,"previous_dimensions":[97,63],"next_dimensions":[131,79],"scene_material_and_geometry_changed":true,"differing_bytes":0,"published_frames":2,"normal_cpu_readback_bytes":0}),
        );

        let ticket =
            match renderer.render_presented(&scene_a, Camera::default(), 97, 63, &controls)? {
                RenderSubmission::Pending(ticket) => ticket,
                _ => return Err("Discard fixture did not start pending GPU work".into()),
            };
        let before_discard = renderer.performance();
        renderer.discard_presented(ticket)?;
        ensure(
            renderer.poll_presented(ticket)?.is_none(),
            "Discarded request published a frame",
        )?;
        ensure(
            renderer.performance().gpu_presentation_frames
                == before_discard.gpu_presentation_frames,
            "Discarded request counted as publication",
        )?;
        // A discarded request still owns the backend until real completion.
        // Polling its known ticket may retire resources but cannot publish.
        let deadline = Instant::now() + Duration::from_secs(10);
        let next = loop {
            ensure(
                renderer.poll_presented(ticket)?.is_none(),
                "Discarded request later published a frame",
            )?;
            match renderer.render_presented(&scene_b, Camera::default(), 131, 79, &controls)? {
                RenderSubmission::Busy => {
                    ensure(
                        Instant::now() < deadline,
                        "Discarded producer work never retired",
                    )?;
                    std::thread::sleep(Duration::from_millis(1));
                }
                submission => break submission,
            }
        };
        let frame = finish_submission(&mut *renderer, next)?;
        ensure(
            display.update(&self.bridge, &frame)?.is_some()
                && capture(&mut self.consumer, &display)? == expected_b,
            "Discard corrupted the next publication",
        )?;
        self.consumer.collect()?;
        drop(frame);
        record(
            self.report,
            json!({"case":"async_discard_retires_without_publication","passed":true,"discarded_ticket_never_published":true,"next_ticket_exact_pixels":true,"differing_bytes":0}),
        );

        let submission =
            renderer.render_presented(&scene_a, Camera::default(), 97, 63, &controls)?;
        ensure(
            matches!(submission, RenderSubmission::Pending(_)),
            "Drop fixture did not own pending producer work",
        )?;
        let drop_started = Instant::now();
        drop(renderer);
        record(
            self.report,
            json!({"case":"async_drop_pending_producer_retirement","passed":true,"producer_pending_at_drop":true,"owner_drop_ms":drop_started.elapsed().as_secs_f64()*1000.0,"retirement_join_required_at_fixture_shutdown":true,"scope":"Actual pending FrameRenderer destruction; aggregate retirement join is validated at fixture shutdown, not Editor Stop UI. Drop timing is diagnostic, without a timing acceptance threshold."}),
        );
        Ok(())
    }

    fn renderer(&self, document: &GraphDocument) -> Result<Box<dyn FrameRenderer>, String> {
        let mut renderer = self
            .registry
            .create_renderer(renderer_node(document)?, document)?;
        renderer.set_detailed_stats(false);
        renderer.set_presentation_target(Some(self.bridge.target()))?;
        Ok(renderer)
    }

    fn compare(
        &mut self,
        name: &str,
        renderer: &mut dyn FrameRenderer,
        size: [usize; 2],
        controls: &RenderOverrides,
        alpha: bool,
    ) -> Result<Frame, String> {
        let before_reference = renderer.performance();
        let reference =
            renderer.render(&self.scene, Camera::default(), size[0], size[1], controls)?;
        ensure(
            reference.gpu.is_none() && reference.rgba.len() == size[0] * size[1] * 4,
            format!("{name}: explicit CPU reference is malformed"),
        )?;
        let after_reference = renderer.performance();
        let converted = egui::ColorImage::from_rgba_unmultiplied(size, &reference.rgba);
        let expected: Vec<u8> = converted
            .pixels
            .iter()
            .flat_map(|pixel| pixel.to_array())
            .collect();
        if alpha {
            for alpha in [0, 128, 255] {
                ensure(
                    reference
                        .rgba
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|p| p[3] == alpha),
                    format!("{name}: source alpha {alpha} missing"),
                )?;
            }
            ensure(
                reference
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[3] == 0 && p[..3].iter().any(|v| *v != 0)),
                "Transparent source RGB fixture was optimized away",
            )?;
        }
        let before_native = renderer.performance();
        let frame = presented(renderer, &self.scene, Camera::default(), size, controls)?;
        let after_native = renderer.performance();
        ensure(
            frame.gpu.is_some() && frame.rgba.is_empty() && frame.is_valid(),
            format!("{name}: native renderer returned CPU or malformed pixels"),
        )?;
        ensure(
            after_native.cpu_readback_bytes == before_native.cpu_readback_bytes,
            format!("{name}: native renderer read GPU pixels to CPU"),
        )?;
        ensure(
            after_native.gpu_presentation_frames == before_native.gpu_presentation_frames + 1,
            format!("{name}: producer GPU frame counter did not advance exactly once"),
        )?;
        ensure(
            after_native.gpu_presentation_bytes
                == before_native.gpu_presentation_bytes + (size[0] * size[1] * 4) as u64,
            format!("{name}: producer GPU byte counter mismatch"),
        )?;
        let id = self
            .display
            .update(&self.bridge, &frame)?
            .ok_or_else(|| format!("{name}: display conversion not submitted"))?;
        ensure(
            self.display.dimensions() == Some(size),
            "GUI display dimensions mismatch",
        )?;
        let actual = capture(&mut self.consumer, &self.display)?;
        ensure(
            actual.len() == expected.len(),
            format!("{name}: diagnostic display size mismatch"),
        )?;
        let max_error = actual
            .iter()
            .zip(&expected)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        let differing_bytes = actual.iter().zip(&expected).filter(|(a, b)| a != b).count();
        // Repeat has no submission and must leave frame identity/display stable.
        ensure(
            self.display.update(&self.bridge, &frame)?.is_none()
                && self.display.texture_id() == Some(id),
            "Repeated GPU publication was incorrectly marked fresh",
        )?;

        if let Some(folder) = &self.png_folder {
            write_png(&folder.join(format!("{name}.png")), size, &actual)?;
        }
        record(
            self.report,
            json!({
                "case":name, "passed":max_error == 0, "dimensions":size,
                "pixels":size[0]*size[1], "max_channel_error":max_error,"differing_bytes":differing_bytes,
                "reference_cpu_readback_bytes":after_reference.cpu_readback_bytes-before_reference.cpu_readback_bytes,
                "presented_cpu_readback_bytes":after_native.cpu_readback_bytes-before_native.cpu_readback_bytes,
                "presented_gpu_frames":after_native.gpu_presentation_frames-before_native.gpu_presentation_frames,
                "presented_gpu_bytes":after_native.gpu_presentation_bytes-before_native.gpu_presentation_bytes,
                "display_explicit_diagnostic_readback_bytes":actual.len(),"display_counters":self.display.counters(),
                "alpha_fixture":alpha,
            }),
        );
        ensure(
            max_error == 0,
            format!(
                "{name}: GPU display differs from actual ColorImage conversion: {differing_bytes} bytes, max {max_error}"
            ),
        )?;
        Ok(reference)
    }

    fn pool_check(&mut self, document: &GraphDocument) -> Result<(), String> {
        let mut renderer = self.renderer(document)?;
        let before = renderer.performance();
        let mut frames = Vec::new();
        for _ in 0..4 {
            frames.push(presented(
                &mut *renderer,
                &self.scene,
                Camera::default(),
                [97, 63],
                &RenderOverrides::default(),
            )?);
        }
        ensure(
            matches!(
                renderer.render_presented(
                    &self.scene,
                    Camera::default(),
                    97,
                    63,
                    &RenderOverrides::default()
                )?,
                RenderSubmission::Busy
            ),
            "Fifth held lease bypassed the bounded producer pool",
        )?;
        let mut display = GpuTexture::default();
        let first = frames.remove(0);
        ensure(
            display.update(&self.bridge, &first)?.is_some(),
            "Pool output was not registered with the painter",
        )?;
        drop(first);
        // No completion poll has released the consumer's source lease yet.
        ensure(
            matches!(
                renderer.render_presented(
                    &self.scene,
                    Camera::default(),
                    97,
                    63,
                    &RenderOverrides::default()
                )?,
                RenderSubmission::Busy
            ),
            "Source lease was released before consumer completion observation",
        )?;
        // The displayed descriptor pins its source even after a completed draw.
        let diagnostic = capture(&mut self.consumer, &display)?;
        let counters = display.counters();
        drop(display);
        self.consumer.collect()?;
        let recycled = presented(
            &mut *renderer,
            &self.scene,
            Camera::default(),
            [97, 63],
            &RenderOverrides::default(),
        )?;
        ensure(
            recycled.gpu.is_some() && recycled.rgba.is_empty(),
            "Recycled native slot returned CPU pixels",
        )?;
        let after = renderer.performance();
        ensure(
            after.cpu_readback_bytes == before.cpu_readback_bytes
                && after.gpu_presentation_frames == before.gpu_presentation_frames + 5,
            "Pool saturation performed a hidden readback/submission",
        )?;
        drop(recycled);
        drop(frames);
        record(
            self.report,
            json!({"case":"four_leases_backpressure_consumer_retirement","passed":true,"held_leases":4,"fifth_skipped":true,"leased_while_displayed":true,"recycled_after_descriptor_retirement":true,"gpu_frames":after.gpu_presentation_frames-before.gpu_presentation_frames,"normal_cpu_readback_bytes":after.cpu_readback_bytes-before.cpu_readback_bytes,"explicit_diagnostic_bytes":diagnostic.len(),"display_counters":counters}),
        );
        Ok(())
    }

    fn drop_check(&mut self, document: &GraphDocument) -> Result<(), String> {
        let mut renderer = self.renderer(document)?;
        let reference = renderer.render(
            &self.scene,
            Camera::default(),
            97,
            63,
            &RenderOverrides::default(),
        )?;
        let expected: Vec<u8> = egui::ColorImage::from_rgba_unmultiplied([97, 63], &reference.rgba)
            .pixels
            .iter()
            .flat_map(|p| p.to_array())
            .collect();
        let frame = presented(
            &mut *renderer,
            &self.scene,
            Camera::default(),
            [97, 63],
            &RenderOverrides::default(),
        )?;
        drop(renderer);
        let mut display = GpuTexture::default();
        ensure(
            display.update(&self.bridge, &frame)?.is_some(),
            "Frame could not be consumed after producer renderer drop",
        )?;
        let actual = capture(&mut self.consumer, &display)?;
        ensure(
            actual == expected,
            "Renderer drop invalidated published GPU pixels",
        )?;
        self.consumer.collect()?;
        drop(frame);
        // Exercise pane closure with an actual outstanding copy and retain no
        // producer frame outside the production bridge's completion machinery.
        let mut renderer = self.renderer(document)?;
        let frame = presented(
            &mut *renderer,
            &self.scene,
            Camera::default(),
            [97, 63],
            &RenderOverrides::default(),
        )?;
        ensure(
            display.update(&self.bridge, &frame)?.is_some(),
            "Pane-drop frame was not registered with the painter",
        )?;
        drop(frame);
        drop(renderer);
        drop(display);
        self.consumer.collect()?;
        record(
            self.report,
            json!({"case":"producer_drop_and_pane_drop_completion_lifetime","passed":true,"published_frame_consumed_after_renderer_drop":true,"closed_pane_pending_copies":0,"explicit_diagnostic_bytes":actual.len(),"scope":"FrameRenderer and GpuTexture destruction/recreation; not the Editor Stop command UI"}),
        );
        Ok(())
    }
}

fn capture(consumer: &mut Offscreen, display: &GpuTexture) -> Result<Vec<u8>, String> {
    consumer.capture(
        display.texture_id().ok_or("Missing display texture")?,
        display.dimensions().ok_or("Missing display dimensions")?,
    )
}

fn write_png(path: &Path, size: [usize; 2], pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        size[0] as u32,
        size[1] as u32,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels)
        .map_err(|e| e.to_string())
}

#[path = "gpu_presentation_check/window_check.rs"]
mod window_check;
