//! Matched native editor UI benchmark. Both painters use the same Win32 host.
mod gpu;
mod painter;
mod thin;

use crate::{app::Editor, distribution, engine_profiles, host, project};
use ash::vk;
use eframe::egui::{self, epaint::Primitive};
use gpu::{DeviceInfo, Gpu};
use module_engine_core::{WorldKind, demo_document, make_edge};
use serde::Serialize;
use std::{
    fs,
    io::BufWriter,
    path::{Path, PathBuf},
    time::Instant,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

const SIZE: [u32; 2] = [1580, 940];
const WARMUP: usize = 30;
const SAMPLES: usize = 240;
const BUDGET_MS: f64 = 1000. / 60.;

#[derive(Clone, Copy, Serialize)]
enum PainterKind {
    #[serde(rename = "ash")]
    Library,
    #[serde(rename = "thin")]
    Thin,
}

trait VkResultExt<T> {
    fn at(self, operation: &str) -> Result<T, String>;
}

impl<T> VkResultExt<T> for Result<T, vk::Result> {
    fn at(self, operation: &str) -> Result<T, String> {
        self.map_err(|e| format!("R3.1 Vulkan {operation}: {e:?}"))
    }
}

pub(crate) fn run(
    args: &[String],
    profile: Option<distribution::DistributionProfile>,
) -> Result<(), String> {
    let [painter, dpi, destination] = args else {
        return Err("Usage: --r31-editor-vulkan ash|thin 1.0|1.5 <new output folder>".into());
    };
    let kind = match painter.as_str() {
        "ash" => PainterKind::Library,
        "thin" => PainterKind::Thin,
        _ => return Err("Specify the ash or thin painter".into()),
    };
    let scale = match dpi.as_str() {
        "1.0" => 1.0,
        "1.5" => 1.5,
        _ => return Err("Specify the expected monitor scale as 1.0 or 1.5".into()),
    };
    for variable in [
        "EFRAME_SCREENSHOT_TO",
        "MODULE_ENGINE_UI_SCREENSHOT_TO",
        "MODULE_ENGINE_CAPTURE_VIEW",
    ] {
        if std::env::var_os(variable).is_some() {
            return Err(format!("Unset {variable} for the R3.1 benchmark"));
        }
    }
    let destination = PathBuf::from(destination);
    fs::create_dir(&destination).map_err(|e| format!("New R3.1 destination: {e}"))?;
    let destination = destination.canonicalize().map_err(|e| e.to_string())?;
    let (editor, resource_root) = fixture(&destination, profile)?;
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let validation_baseline = module_engine_vulkan::validation_error_count();
    let mut app = Benchmark {
        destination: &destination,
        resource_root,
        kind,
        scale,
        editor: Some(editor),
        session: None,
        result: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    let mut report = app
        .result
        .take()
        .ok_or("R3.1 loop exited without an outcome")??;
    drop(app);
    report.validation_errors = module_engine_vulkan::validation_error_count() - validation_baseline;
    let file = fs::File::create(destination.join("report.json")).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(file, &report).map_err(|e| e.to_string())?;
    if report.validation_errors != 0 {
        return Err(format!(
            "R3.1 Vulkan validation errors: {}",
            report.validation_errors
        ));
    }
    if report.intervals_over_budget != 0 {
        return Err(format!(
            "R3.1 exceeded {BUDGET_MS:.6} ms in {} intervals, worst {:.6} ms",
            report.intervals_over_budget, report.max_interval_ms,
        ));
    }
    Ok(())
}

fn fixture(
    root: &Path,
    profile: Option<distribution::DistributionProfile>,
) -> Result<(Editor, PathBuf), String> {
    let mut document = demo_document(WorldKind::Objects);
    document.root.nodes.retain(|node| node.id != "speed");
    document
        .root
        .edges
        .retain(|edge| edge.from_node != "speed" && edge.to_node != "speed");
    document
        .root
        .edges
        .push(make_edge("base", "value", "world", "speed"));
    document.custom_sources.clear();
    host::ensure_authoring_configuration(&mut document);
    let resource_root = std::env::current_dir().map_err(|e| e.to_string())?;
    engine_profiles::finish_template(
        &mut document,
        engine_profiles::ProfileKind::Neutral,
        &resource_root,
    )?;
    let path = root.join("project.json");
    let distribution = distribution::detect(&path, &document, profile)?;
    if !distribution.can_author_engine() {
        return Err(format!(
            "R3.1 requires Full with valid source inventory: {:?}",
            distribution.source_status.diagnostics
        ));
    }
    let resource_root = distribution.package_root.clone();
    project::save(&path, &document)?;
    let personal = root.join("personal");
    fs::create_dir(&personal).map_err(|e| e.to_string())?;
    let mut editor = Editor::new_native_with_diagnostic_personal_root(
        document,
        path,
        distribution,
        Some(personal),
        None,
    );
    editor.r31_spike_layout()?;
    Ok((editor, resource_root))
}

struct Benchmark<'a> {
    destination: &'a Path,
    resource_root: PathBuf,
    kind: PainterKind,
    scale: f64,
    editor: Option<Editor>,
    session: Option<Session>,
    result: Option<Result<Report, String>>,
}

impl ApplicationHandler for Benchmark<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(editor) = self.editor.take() else {
            return;
        };
        match Session::new(event_loop, &self.resource_root, self.kind, editor) {
            Ok(session) => {
                session.window.set_visible(true);
                session.window.request_redraw();
                self.session = Some(session);
            }
            Err(error) => {
                self.result = Some(Err(error));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(session) = self
            .session
            .as_mut()
            .filter(|session| session.window.id() == id)
        else {
            return;
        };
        let _ = session.input.on_window_event(&session.window, &event);
        let result = match event {
            WindowEvent::CloseRequested => Err("R3.1 window closed before capture".into()),
            WindowEvent::RedrawRequested => {
                session.redraw(event_loop, self.destination, self.kind, self.scale)
            }
            _ => return,
        };
        match result {
            Ok(None) => session.window.request_redraw(),
            Ok(Some(report)) => {
                self.result = Some(Ok(report));
                event_loop.exit();
            }
            Err(error) => {
                self.result = Some(Err(error));
                event_loop.exit();
            }
        }
    }
}

struct FrameTextures(egui::TexturesDelta);

impl Drop for FrameTextures {
    fn drop(&mut self) {
        // An unsuccessful frame exits the benchmark. Discard unprocessed
        // updates so egui's debug Drop check preserves the original error.
        if !self.0.is_empty() {
            self.0.clear();
        }
    }
}

struct Session {
    gpu: Gpu,
    window: Window,
    context: egui::Context,
    input: egui_winit::State,
    viewport: egui::ViewportInfo,
    editor: Editor,
    present_count: usize,
    previous: Option<Instant>,
    samples: Vec<Sample>,
}

impl Session {
    fn new(
        event_loop: &ActiveEventLoop,
        root: &Path,
        kind: PainterKind,
        editor: Editor,
    ) -> Result<Self, String> {
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("ModuleEngine / R3.1 Vulkan")
                    .with_inner_size(PhysicalSize::new(SIZE[0], SIZE[1]))
                    .with_resizable(false)
                    .with_visible(false),
            )
            .map_err(|e| e.to_string())?;
        let gpu = Gpu::new(&window, root, kind)?;
        let context = egui::Context::default();
        crate::app::configure_style(&context);
        let mut viewport = egui::ViewportInfo::default();
        egui_winit::update_viewport_info(&mut viewport, &context, &window, true);
        let input = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(8192),
        );
        Ok(Self {
            gpu,
            window,
            context,
            input,
            viewport,
            editor,
            present_count: 0,
            previous: None,
            samples: Vec::with_capacity(SAMPLES),
        })
    }

    fn redraw(
        &mut self,
        event_loop: &ActiveEventLoop,
        destination: &Path,
        painter: PainterKind,
        scale: f64,
    ) -> Result<Option<Report>, String> {
        let actual = self.window.scale_factor();
        let extent: [u32; 2] = self.window.inner_size().into();
        if actual != scale || extent != SIZE {
            return Err(format!(
                "Expected native DPI scale {scale} and client {SIZE:?}, observed {actual} and {extent:?}"
            ));
        }
        let capture = self.samples.len() == SAMPLES;
        let start = Instant::now();
        egui_winit::update_viewport_info(&mut self.viewport, &self.context, &self.window, false);
        let mut input = self.input.take_egui_input(&self.window);
        input
            .viewports
            .insert(egui::ViewportId::ROOT, self.viewport.clone());
        // Keep the timed workload deterministic. Authoring input is outside this route.
        input.events.clear();
        input.hovered_files.clear();
        input.dropped_files.clear();
        let output = self.context.run_ui(input, |ui| self.editor.draw(ui));
        let mut textures = FrameTextures(output.textures_delta);
        let pixels_per_point = output.pixels_per_point;
        if pixels_per_point != scale as f32 || self.context.zoom_factor() != 1. {
            return Err("Native DPI did not propagate to egui at unit zoom".into());
        }
        self.input.handle_platform_output_with_event_loop(
            &self.window,
            event_loop,
            output.platform_output,
        );
        let mut primitives = self.context.tessellate(output.shapes, pixels_per_point);
        let bounds = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(SIZE[0] as f32, SIZE[1] as f32) / pixels_per_point,
        );
        primitives.retain_mut(|item| {
            item.clip_rect = item.clip_rect.intersect(bounds);
            item.clip_rect.is_positive()
        });
        let (vertices, indices) =
            primitives
                .iter()
                .try_fold((0, 0), |(vertices, indices), item| match &item.primitive {
                    Primitive::Mesh(mesh) => {
                        Ok((vertices + mesh.vertices.len(), indices + mesh.indices.len()))
                    }
                    Primitive::Callback(_) => {
                        Err("The R3.1 fixture unexpectedly generated a paint callback")
                    }
                })?;
        if indices == 0 {
            return Err("The R3.1 fixture produced no editor triangles".into());
        }
        let updates = textures.0.set.values().map(|deltas| deltas.len()).sum();
        let prepared = Instant::now();
        let (presented, image) = self.gpu.paint(
            &self.window,
            &primitives,
            &mut textures.0,
            pixels_per_point,
            capture,
        )?;
        self.present_count += 1;
        if let Some(image) = image {
            self.gpu.finish()?;
            save_png(destination, &image)?;
            let report = self.report(painter, pixels_per_point);
            return Ok(Some(report));
        }
        if self.present_count > WARMUP {
            let previous = self.previous.ok_or("Missing R3.1 warm-up endpoint")?;
            self.samples.push(Sample {
                interval_ms: milliseconds(previous, presented),
                egui_ms: milliseconds(start, prepared),
                paint_present_ms: milliseconds(prepared, presented),
                meshes: primitives.len(),
                vertices,
                indices,
                texture_updates: updates,
            });
        }
        self.previous = Some(presented);
        Ok(None)
    }

    fn report(&mut self, painter: PainterKind, scale: f32) -> Report {
        let mut intervals = self
            .samples
            .iter()
            .map(|sample| sample.interval_ms)
            .collect::<Vec<_>>();
        intervals.sort_by(f64::total_cmp);
        let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
        let refresh_millihertz = self
            .window
            .current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz());
        Report {
            painter,
            native_scale: self.window.scale_factor(),
            pixels_per_point: scale,
            egui_zoom: self.context.zoom_factor(),
            client_pixels: SIZE,
            refresh_millihertz,
            device: self.gpu.info.clone(),
            warmup_frames: WARMUP,
            presented_frames: self.present_count,
            capture_frame: self.present_count,
            mean_interval_ms: mean,
            fps: 1000. / mean,
            p95_interval_ms: intervals[(intervals.len() * 95).div_ceil(100) - 1],
            max_interval_ms: intervals[intervals.len() - 1],
            frame_budget_ms: BUDGET_MS,
            intervals_over_budget: intervals
                .iter()
                .filter(|&&sample| sample > BUDGET_MS)
                .count(),
            validation_errors: 0,
            samples: std::mem::take(&mut self.samples),
        }
    }
}

fn milliseconds(start: Instant, end: Instant) -> f64 {
    end.duration_since(start).as_secs_f64() * 1000.
}

fn save_png(output: &Path, rgba: &[u8]) -> Result<(), String> {
    let path = output.join("editor.png");
    let file = fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), SIZE[0], SIZE[1]);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct Sample {
    interval_ms: f64,
    egui_ms: f64,
    paint_present_ms: f64,
    meshes: usize,
    vertices: usize,
    indices: usize,
    texture_updates: usize,
}

#[derive(Serialize)]
struct Report {
    painter: PainterKind,
    native_scale: f64,
    pixels_per_point: f32,
    egui_zoom: f32,
    client_pixels: [u32; 2],
    refresh_millihertz: Option<u32>,
    device: DeviceInfo,
    warmup_frames: usize,
    presented_frames: usize,
    capture_frame: usize,
    mean_interval_ms: f64,
    fps: f64,
    p95_interval_ms: f64,
    max_interval_ms: f64,
    frame_budget_ms: f64,
    intervals_over_budget: usize,
    validation_errors: u64,
    samples: Vec<Sample>,
}
