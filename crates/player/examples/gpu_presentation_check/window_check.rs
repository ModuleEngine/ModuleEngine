//! Draw the producer image through the native window painter and sample the swapchain.
use super::*;
use module_engine_native_ui::{FrameBridge, FrameTexture, NativeApp, Window};

pub(super) fn run(report: &Path) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    let source = root.clone();
    module_engine_native_ui::run(
        root,
        egui::ViewportBuilder::default()
            .with_title("ModuleEngine / graph image verification")
            .with_inner_size([320., 240.]),
        move |ctx, _, bridge| {
            let mut registry = ModuleRegistry::builtin();
            module_engine_vulkan::install(&mut registry, &source).map_err(|e| e.to_string())?;
            let mut doc = custom_alpha_document(RenderPath::Forward)?;
            let shader = doc
                .shader_sources
                .get_mut("fixture-final-alpha")
                .ok_or("Missing color fixture")?;
            *shader = shader.replace(
                "max(encoded, float3(0.25,0.5,0.75))",
                "float3(64,128,192)/255.0",
            );
            let mut renderer = registry.create_renderer(renderer_node(&doc)?, &doc)?;
            renderer.set_presentation_target(Some(bridge.target()))?;
            let scene = ObjectsWorld::demo().snapshot();
            let camera = Camera::default();
            let controls = RenderOverrides::default();
            let mut leases = Vec::new();
            for _ in 0..4 {
                leases.push(presented(
                    &mut *renderer,
                    &scene,
                    camera,
                    [96, 64],
                    &controls,
                )?);
            }
            ensure(
                matches!(
                    renderer.render_presented(&scene, camera, 128, 64, &controls)?,
                    RenderSubmission::Busy
                ),
                "A held window frame was recycled while its consumer was live",
            )?;
            ensure(
                renderer.set_presentation_target(None).is_err(),
                "Detached a live target",
            )?;
            drop(leases.pop());
            let resized = presented(&mut *renderer, &scene, camera, [128, 64], &controls)?;
            ensure(
                resized.width == 128 && resized.height == 64,
                "Incorrect resized output",
            )?;
            let frame = leases.pop().ok_or("Missing original window frame")?;
            drop(resized);
            drop(leases);
            ensure(frame.rgba.is_empty(), "Window fixture received a CPU image")?;
            let texture = bridge.bind(&frame)?;
            // The view and its device must outlive the producing renderer.
            drop(renderer);
            drop(frame);
            Ok(Box::new(Probe {
                texture,
                bridge,
                output,
                atlas: ctx.load_texture(
                    "partial delta fixture",
                    egui::ColorImage::new(
                        [3, 1],
                        vec![
                            egui::Color32::RED,
                            egui::Color32::BLACK,
                            egui::Color32::BLACK,
                        ],
                    ),
                    egui::TextureOptions::NEAREST,
                ),
                requested: false,
                started: Instant::now(),
            }))
        },
    )?;
    let evidence = output_result(&result)?;
    if let Some(parent) = report.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        report,
        serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    ensure(
        evidence["passed"] == true,
        format!("Window pixel verification failed: {evidence}"),
    )
}
fn output_result(result: &Mutex<Option<Value>>) -> Result<Value, String> {
    result
        .lock()
        .map_err(|_| "Result lock poisoned".to_string())?
        .take()
        .ok_or_else(|| "Window closed before returning the capture".into())
}

struct Probe {
    texture: FrameTexture,
    bridge: FrameBridge,
    output: Arc<Mutex<Option<Value>>>,
    atlas: egui::TextureHandle,
    requested: bool,
    started: Instant,
}
impl NativeApp for Probe {
    fn ui(&mut self, ui: &mut egui::Ui, _: &Window) {
        let ctx = ui.ctx();
        let scale = ctx.pixels_per_point();
        let rect = egui::Rect::from_min_size(
            egui::pos2(16. / scale, 16. / scale),
            egui::vec2(96. / scale, 64. / scale),
        );
        ui.painter().rect_filled(rect, 0., egui::Color32::BLACK);
        ui.painter().image(
            self.texture.id,
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            egui::Color32::WHITE,
        );
        let frame = ctx.cumulative_frame_nr();
        if frame == 1 || frame == 2 {
            let color = if frame == 1 {
                egui::Color32::GREEN
            } else {
                egui::Color32::BLUE
            };
            self.atlas.set_partial(
                [frame as usize, 0],
                egui::ColorImage::new([1, 1], vec![color]),
                egui::TextureOptions::NEAREST,
            );
        }
        ui.painter().image(
            self.atlas.id(),
            egui::Rect::from_min_size(
                egui::pos2(16. / scale, 100. / scale),
                egui::vec2(33. / scale, 11. / scale),
            ),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            egui::Color32::WHITE,
        );
        let image = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let observed: Vec<_> = (0..3)
                .map(|x| image[(16 + 30 + x, 32)].to_array())
                .collect();
            let expected = [[0, 0, 0, 255], [32, 64, 96, 255], [64, 128, 192, 255]];
            let atlas = [21, 32, 43].map(|x| image[(x, 105)].to_array());
            let atlas_expected = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
            *self.output.lock().unwrap() = Some(
                json!({"passed":observed==expected && atlas==atlas_expected,"partial_atlas":atlas,"expected_atlas":atlas_expected,"adapter":self.bridge.adapter,"surface_mode":self.bridge.present_mode,"source":"leased RGBA8 graph output after renderer drop","expected":expected,"sampled_swapchain":observed}),
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if frame >= 3 && !self.requested {
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        } else if self.started.elapsed() > Duration::from_secs(10) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}
