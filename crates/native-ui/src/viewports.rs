use crate::Gpu;
use egui::{Context, ImmediateViewport, ViewportBuilder, ViewportId, ViewportInfo};
use egui_winit::ActionRequested;
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc, time::Instant};
use winit::{
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

// Creating a child must happen synchronously inside egui's immediate callback,
// while winit lends the main thread its active event loop.
scoped_tls::scoped_thread_local!(pub(crate) static EVENT_LOOP: ActiveEventLoop);

pub(crate) struct Viewports {
    pub gpu: Gpu,
    windows: HashMap<ViewportId, Child>,
    pub root_info: ViewportInfo,
    started: Instant,
    pub error: Option<String>,
}

struct Child {
    window: Arc<Window>,
    input: egui_winit::State,
    info: ViewportInfo,
    builder: ViewportBuilder,
}

impl Viewports {
    pub fn install(ctx: &Context, gpu: Gpu) -> Rc<RefCell<Self>> {
        let state = Rc::new(RefCell::new(Self {
            gpu,
            windows: HashMap::new(),
            root_info: Default::default(),
            started: Instant::now(),
            error: None,
        }));
        let weak = Rc::downgrade(&state);
        ctx.set_embed_viewports(false);
        Context::set_immediate_viewport_renderer(move |ctx, viewport| {
            if let Some(state) = weak.upgrade() {
                EVENT_LOOP.with(|el| {
                    if let Err(error) = draw(&state, ctx, el, viewport) {
                        state.borrow_mut().error = Some(error);
                    }
                });
            }
        });
        state
    }

    pub fn fill_input(&self, input: &mut egui::RawInput) {
        input.time = Some(self.started.elapsed().as_secs_f64());
        input
            .viewports
            .insert(ViewportId::ROOT, self.root_info.clone());
        input
            .viewports
            .extend(self.windows.iter().map(|(id, v)| (*id, v.info.clone())));
    }

    pub fn event(&mut self, id: WindowId, event: &WindowEvent) -> bool {
        let Some(child) = self.windows.values_mut().find(|v| v.window.id() == id) else {
            return false;
        };
        let response = child.input.on_window_event(&child.window, event);
        if matches!(event, WindowEvent::CloseRequested) {
            child.info.events.push(egui::ViewportEvent::Close);
            return true;
        }
        response.repaint || matches!(event, WindowEvent::RedrawRequested)
    }

    pub fn prune(
        &mut self,
        active: &egui::OrderedViewportIdMap<egui::ViewportOutput>,
    ) -> Result<(), String> {
        let closed: Vec<_> = self
            .windows
            .keys()
            .filter(|id| !active.contains_key(id))
            .copied()
            .collect();
        for id in closed {
            self.remove(id)?;
        }
        Ok(())
    }

    fn remove(&mut self, id: ViewportId) -> Result<(), String> {
        if let Some(child) = self.windows.remove(&id) {
            self.gpu.detach(child.window.id())?;
        }
        Ok(())
    }
}

fn draw(
    state: &RefCell<Viewports>,
    ctx: &Context,
    el: &ActiveEventLoop,
    mut viewport: ImmediateViewport<'_>,
) -> Result<(), String> {
    let id = viewport.ids.this;
    let input = {
        let mut state = state.borrow_mut();
        let mut commands = Vec::new();
        if let Some(child) = state.windows.get_mut(&id) {
            let (patch, recreate) = child.builder.patch(viewport.builder.clone());
            if recreate {
                state.remove(id)?;
            } else {
                commands = patch;
            }
        }
        if !state.windows.contains_key(&id) {
            let window = Arc::new(
                egui_winit::create_window(ctx, el, &viewport.builder).map_err(|e| e.to_string())?,
            );
            state.gpu.attach(window.clone())?;
            let input = egui_winit::State::new(
                ctx.clone(),
                id,
                &window,
                Some(window.scale_factor() as f32),
                window.theme(),
                None,
            );
            let mut info = ViewportInfo {
                parent: Some(viewport.ids.parent),
                ..Default::default()
            };
            egui_winit::update_viewport_info(&mut info, ctx, &window, true);
            state.windows.insert(
                id,
                Child {
                    window,
                    input,
                    info,
                    builder: viewport.builder,
                },
            );
        }
        let child = state.windows.get_mut(&id).expect("initialized child");
        egui_winit::process_viewport_commands(
            ctx,
            &mut child.info,
            commands,
            &child.window,
            &mut Vec::new(),
        );
        egui_winit::update_viewport_info(&mut child.info, ctx, &child.window, false);
        let mut input = child.input.take_egui_input(&child.window);
        state.fill_input(&mut input);
        input
    };
    let output = ctx.run_ui(input, |ui| (viewport.viewport_ui_cb)(ui));
    let mut state = state.borrow_mut();
    let Viewports { gpu, windows, .. } = &mut *state;
    let child = windows.get_mut(&id).expect("active viewport");
    child.info.events.clear();
    child
        .input
        .handle_platform_output_with_event_loop(&child.window, el, output.platform_output);
    let mut actions = Vec::new();
    if let Some(v) = output.viewport_output.get(&id) {
        egui_winit::process_viewport_commands(
            ctx,
            &mut child.info,
            v.commands.iter().cloned(),
            &child.window,
            &mut actions,
        );
    }
    let shapes = ctx.tessellate(output.shapes, output.pixels_per_point);
    let mut textures = output.textures_delta;
    let capture = actions
        .iter()
        .any(|a| matches!(a, ActionRequested::Screenshot(_)));
    let result = gpu.paint_child(
        child.window.id(),
        &shapes,
        &mut textures,
        output.pixels_per_point,
        capture,
    );
    textures.clear();
    let pixels = result?;
    let size = child.window.inner_size();
    for action in actions {
        let event = match action {
            ActionRequested::Cut => egui::Event::Cut,
            ActionRequested::Copy => egui::Event::Copy,
            ActionRequested::Paste => {
                let Some(text) = child.input.clipboard_text() else {
                    continue;
                };
                egui::Event::Paste(text)
            }
            ActionRequested::Screenshot(user_data) => {
                let Some(bytes) = &pixels else { continue };
                let extent = [size.width as usize, size.height as usize];
                if bytes.len() != extent[0] * extent[1] * 4 {
                    continue;
                }
                egui::Event::Screenshot {
                    viewport_id: id,
                    user_data,
                    image: Arc::new(egui::ColorImage::from_rgba_unmultiplied(extent, bytes)),
                }
            }
        };
        child.input.egui_input_mut().events.push(event);
        ctx.request_repaint();
    }
    Ok(())
}
