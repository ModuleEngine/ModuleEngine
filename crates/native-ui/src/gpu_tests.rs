use super::*;
use egui::{
    Color32, Rect, TextureId,
    epaint::{Mesh, Primitive},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
};

struct RecoveryCheck;
impl ApplicationHandler for RecoveryCheck {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        let window = Arc::new(
            events
                .create_window(
                    Window::default_attributes()
                        .with_title("Acquire recovery")
                        .with_inner_size(winit::dpi::PhysicalSize::new(160, 120)),
                )
                .unwrap(),
        );
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut gpu = Gpu::new(window, &root, PainterKind::Library, false, false).unwrap();
        let errors = module_engine_vulkan::validation_error_count();
        let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(40., 40.));
        let mut mesh = Mesh::with_texture(TextureId::User(u64::MAX));
        mesh.add_rect_with_uv(rect, rect, Color32::WHITE);
        let bad = [ClippedPrimitive {
            clip_rect: rect,
            primitive: Primitive::Mesh(mesh),
        }];
        let chain = gpu.root.frames.swapchain;
        let failure = gpu
            .paint(&bad, &mut TexturesDelta::default(), 1., false)
            .unwrap_err();
        assert!(failure.contains("egui-ash-renderer draw"), "{failure}");
        let frame = gpu
            .paint(&[], &mut TexturesDelta::default(), 1., true)
            .unwrap()
            .unwrap();
        assert_ne!(
            chain, gpu.root.frames.swapchain,
            "reused a swapchain with an unpresented acquisition"
        );
        let [w, h] = gpu.extent();
        assert_eq!(frame.1.as_ref().unwrap().len(), (w * h * 4) as usize);
        let replaced = gpu.root.frames.swapchain;
        gpu.root.frames.fail_before_submit = true;
        let failure = gpu
            .paint(&[], &mut TexturesDelta::default(), 1., false)
            .unwrap_err();
        assert_eq!(failure, "injected submit failure");
        let retry = gpu
            .paint(&[], &mut TexturesDelta::default(), 1., true)
            .unwrap()
            .unwrap();
        assert_ne!(replaced, gpu.root.frames.swapchain);
        assert_eq!(retry.1, frame.1);
        // Closing immediately on the error is the normal Editor/Player path.
        assert!(
            gpu.paint(&bad, &mut TexturesDelta::default(), 1., false)
                .is_err()
        );
        drop(gpu);
        assert_eq!(module_engine_vulkan::validation_error_count(), errors);
        events.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

#[test]
#[ignore = "Requires a native Vulkan window; run on a Windows or X11 desktop"]
fn failed_recording_retires_the_acquired_slot_before_retry() {
    let mut builder = EventLoop::builder();
    #[cfg(windows)]
    winit::platform::windows::EventLoopBuilderExtWindows::with_any_thread(&mut builder, true);
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_x11().with_any_thread(true);
    }
    builder
        .build()
        .unwrap()
        .run_app(&mut RecoveryCheck)
        .unwrap();
}
