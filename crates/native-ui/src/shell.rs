//! The winit event loop owns platform input and the egui root viewport.
use crate::{
    Gpu, PainterKind,
    viewports::{EVENT_LOOP, Viewports},
};
use egui::{Context, RawInput, ViewportBuilder, ViewportCommand, ViewportId, ViewportInfo};
use egui_winit::ActionRequested;
use std::{cell::RefCell, rc::Rc};
use std::{path::PathBuf, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

pub trait NativeApp {
    /// Root input processing through tessellation, excluding the root Vulkan paint.
    fn frame_prepared(&mut self, _elapsed: std::time::Duration) {}
    fn on_exit(&mut self) {}
    fn raw_input(&mut self, _ctx: &Context, _input: &mut RawInput) {}
    fn ui(&mut self, ui: &mut egui::Ui, window: &Window);
}

pub fn run<
    A: NativeApp,
    F: FnOnce(&Context, &Arc<Window>, crate::FrameBridge) -> Result<Box<A>, String>,
>(
    root: PathBuf,
    builder: ViewportBuilder,
    create: F,
) -> Result<(), String> {
    run_with_meshlets(root, builder, false, create)
}

pub fn run_with_meshlets<
    A: NativeApp,
    F: FnOnce(&Context, &Arc<Window>, crate::FrameBridge) -> Result<Box<A>, String>,
>(
    root: PathBuf,
    builder: ViewportBuilder,
    meshlets: bool,
    create: F,
) -> Result<(), String> {
    match std::env::var("MODULE_ENGINE_GUI_BACKEND").as_deref() {
        Ok("ash") | Err(std::env::VarError::NotPresent) => {}
        _ => {
            return Err(
                "Native Editor and Player require MODULE_ENGINE_GUI_BACKEND=ash or an unset value"
                    .into(),
            );
        }
    }
    let mut builder_loop = EventLoop::<Instant>::with_user_event();
    match std::env::var("MODULE_ENGINE_WINDOW_SYSTEM").as_deref() {
        Ok("auto") | Err(std::env::VarError::NotPresent) => {}
        #[cfg(target_os = "linux")]
        Ok("x11") => {
            winit::platform::x11::EventLoopBuilderExtX11::with_x11(&mut builder_loop);
        }
        #[cfg(target_os = "linux")]
        Ok("wayland") => {
            winit::platform::wayland::EventLoopBuilderExtWayland::with_wayland(&mut builder_loop);
        }
        _ => return Err("Unsupported MODULE_ENGINE_WINDOW_SYSTEM for this platform".into()),
    }
    let event_loop = builder_loop.build().map_err(|e| e.to_string())?;
    let proxy = event_loop.create_proxy();
    let context = Context::default();
    context.set_request_repaint_callback(move |request| {
        if let Some(at) = Instant::now().checked_add(request.delay) {
            let _ = proxy.send_event(at);
        }
    });
    let mut host = Host {
        meshlets,
        root,
        builder,
        create: Some(Box::new(create)),
        context,
        session: None,
        deadline: None,
        error: None,
    };
    event_loop.run_app(&mut host).map_err(|e| e.to_string())?;

    host.error.map_or(Ok(()), Err)
}

struct Host<A, F> {
    meshlets: bool,
    root: PathBuf,
    builder: ViewportBuilder,
    create: Option<F>,
    context: Context,
    session: Option<Session<A>>,
    deadline: Option<Instant>,
    error: Option<String>,
}

struct Session<A> {
    app: Box<A>,
    viewports: Rc<RefCell<Viewports>>,
    window: Arc<Window>,
    input: egui_winit::State,
    info: ViewportInfo,
    capture_path: Option<PathBuf>,
    presented_frames: usize,
}

impl<A: NativeApp> Session<A> {
    fn redraw(&mut self, ctx: &Context, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let started = Instant::now();
        egui_winit::update_viewport_info(&mut self.info, ctx, &self.window, false);
        let mut input = self.input.take_egui_input(&self.window);
        {
            let mut state = self.viewports.borrow_mut();
            state.root_info = self.info.clone();
            state.fill_input(&mut input);
        }
        let close = self.info.close_requested();
        self.info.events.clear();
        self.app.raw_input(ctx, &mut input);
        let output = EVENT_LOOP.set(event_loop, || {
            ctx.run_ui(input, |ui| self.app.ui(ui, &self.window))
        });
        let mut viewports = self.viewports.borrow_mut();
        if let Some(error) = viewports.error.take() {
            return Err(error);
        }
        viewports.prune(&output.viewport_output)?;
        self.input.handle_platform_output_with_event_loop(
            &self.window,
            event_loop,
            output.platform_output,
        );
        let mut actions = Vec::new();
        if let Some(root) = output.viewport_output.get(&ViewportId::ROOT) {
            if close && !root.commands.contains(&ViewportCommand::CancelClose) {
                event_loop.exit();
            }
            egui_winit::process_viewport_commands(
                ctx,
                &mut self.info,
                root.commands.iter().cloned(),
                &self.window,
                &mut actions,
            );
        }
        if self.info.close_requested() {
            self.window.request_redraw();
        }
        let capture_file = self.capture_path.is_some() && self.presented_frames >= 1;
        let capture = capture_file
            || actions
                .iter()
                .any(|a| matches!(a, ActionRequested::Screenshot(_)));
        let shapes = ctx.tessellate(output.shapes, output.pixels_per_point);
        self.app.frame_prepared(started.elapsed());
        let mut textures = output.textures_delta;
        let presented =
            viewports
                .gpu
                .paint(&shapes, &mut textures, output.pixels_per_point, capture);
        textures.clear();
        let presented = presented?;
        if let Some((_, pixels)) = &presented {
            self.presented_frames += 1;
            if capture_file {
                let path = self.capture_path.take().expect("armed window capture");
                let [width, height] = viewports.gpu.extent();
                let file = std::fs::File::create(&path)
                    .map_err(|e| format!("Capture {}: {e}", path.display()))?;
                let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
                writer
                    .write_image_data(pixels.as_deref().ok_or("Missing window capture pixels")?)
                    .map_err(|e| e.to_string())?;
                writer.finish().map_err(|e| e.to_string())?;
                event_loop.exit();
            } else if self.capture_path.is_some() {
                self.window.request_redraw();
            }
        }
        for action in actions {
            let event = match action {
                ActionRequested::Copy => egui::Event::Copy,
                ActionRequested::Cut => egui::Event::Cut,
                ActionRequested::Paste => {
                    let Some(text) = self.input.clipboard_text() else {
                        continue;
                    };
                    egui::Event::Paste(text)
                }
                ActionRequested::Screenshot(user_data) => {
                    let Some((_, Some(pixels))) = &presented else {
                        continue;
                    };
                    let [w, h] = viewports.gpu.extent();
                    egui::Event::Screenshot {
                        viewport_id: ViewportId::ROOT,
                        user_data,
                        image: Arc::new(egui::ColorImage::from_rgba_unmultiplied(
                            [w as usize, h as usize],
                            pixels,
                        )),
                    }
                }
            };
            self.input.egui_input_mut().events.push(event);
            self.window.request_redraw();
        }
        if presented.is_none()
            && self.window.inner_size().width > 0
            && self.window.inner_size().height > 0
        {
            self.window.request_redraw();
        }
        Ok(())
    }
}

impl<A: NativeApp, F: FnOnce(&Context, &Arc<Window>, crate::FrameBridge) -> Result<Box<A>, String>>
    ApplicationHandler<Instant> for Host<A, F>
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(create) = self.create.take() else {
            return;
        };
        let session = (|| {
            let window = Arc::new(
                egui_winit::create_window(&self.context, event_loop, &self.builder)
                    .map_err(|e| e.to_string())?,
            );
            let gpu = Gpu::new(
                window.clone(),
                &self.root,
                PainterKind::Library,
                false,
                self.meshlets,
            )?;
            let input = egui_winit::State::new(
                self.context.clone(),
                ViewportId::ROOT,
                &window,
                Some(window.scale_factor() as f32),
                window.theme(),
                None,
            );
            let mut info = ViewportInfo::default();
            egui_winit::update_viewport_info(&mut info, &self.context, &window, true);
            let app = create(&self.context, &window, gpu.interactive_bridge())?;
            window.request_redraw();
            Ok::<_, String>(Session {
                app,
                viewports: Viewports::install(&self.context, gpu),
                window,
                input,
                info,
                capture_path: std::env::var_os("EFRAME_SCREENSHOT_TO").map(PathBuf::from),
                presented_frames: 0,
            })
        })();
        match session {
            Ok(s) => self.session = Some(s),
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(s) = self.session.as_mut() else {
            return;
        };
        if s.window.id() != id {
            if s.viewports.borrow_mut().event(id, &event) {
                s.window.request_redraw();
            }
            return;
        }
        let response = s.input.on_window_event(&s.window, &event);
        match event {
            WindowEvent::RedrawRequested => {
                if s.info.events.is_empty()
                    && s.input.egui_input().events.is_empty()
                    && s.viewports.borrow().gpu.software_present_busy()
                {
                    return;
                }
                if let Err(e) = s.redraw(&self.context, event_loop) {
                    self.error = Some(e);
                    event_loop.exit();
                }
            }
            WindowEvent::CloseRequested => {
                s.info.events.push(egui::ViewportEvent::Close);
                s.window.request_redraw();
            }
            _ if response.repaint => s.window.request_redraw(),
            _ => {}
        }
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        // Wayland display handles belong to the event loop. Release all app
        // workers, Vulkan surfaces and winit windows while it is still live.
        if let Some(mut session) = self.session.take() {
            session.app.on_exit();
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, at: Instant) {
        self.deadline = Some(self.deadline.map_or(at, |d| d.min(at)));
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.deadline.is_some_and(|at| at <= Instant::now()) {
            self.deadline = None;
            if let Some(s) = &self.session {
                s.window.request_redraw();
            }
        }
        event_loop.set_control_flow(
            self.deadline
                .map_or(ControlFlow::Wait, ControlFlow::WaitUntil),
        );
    }
}
