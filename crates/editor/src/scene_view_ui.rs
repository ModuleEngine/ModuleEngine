//! Authored Scene viewport presentation and interaction, independent of runtime.
//!
//! `interact` produces commands before `paint`: the host validates/applies them
//! and obtains a fresh neutral snapshot, so transform feedback renders in the
//! same frame. This window never mutates the world, files, or undo history.
use crate::app::editor_ui::{
    CHROME_GAP, CHROME_PAD, SCENE_GROUP_HEIGHT, SCENE_SPEED_WIDTH, SCENE_TOOL_SIZE,
};
use crate::{
    assets_ui::{AssetDrag, AssetKind},
    scene_tools::{self, Axis, DragGesture, GizmoGeometry, Snap, Space, Tool},
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use module_engine_core::{
    Camera, EcsWorld, Frame, SceneSnapshot, Transform, WorldObject, transform_point,
};
use module_engine_physics_rapier::{DebugLine, DebugLineKind};

#[derive(Clone, Debug, PartialEq)]
pub enum SceneAction {
    Select(Option<u64>),
    BeginGesture { id: u64 },
    SetTransform { id: u64, transform: Transform },
    CommitGesture,
    CancelGesture { id: u64, transform: Transform },
    DropModel { asset_ref: String, point: [f32; 3] },
    DropTexture { asset_ref: String, id: u64 },
    Error(String),
}

pub struct SceneViewportState {
    /// Personal view state: the host can persist this in PaneSnapshot.
    pub camera: Camera,
    pub camera_speed: f32,
    pub tool: Tool,
    pub space: Space,
    pub snap: Snap,
    pub gesture: Option<DragGesture>,
    pub rect: Option<Rect>,
    pub overlay_rects: Vec<Rect>,
    pub overlay_targets: std::collections::BTreeMap<String, Rect>,
    pub texture: Option<egui::TextureHandle>,
    gpu_texture: module_engine_player::gpu_presentation::GpuTexture,
    gpu_bridge:
        Option<std::sync::Arc<module_engine_player::gpu_presentation::GpuPresentationBridge>>,
    displayed_camera: Option<Camera>,
    presentation_owner: u64,
    pub error: Option<String>,
    hovered_axis: Option<Axis>,
    locked: bool,
    camera_drag: bool,
    camera_fly: bool,
    camera_input_frame: Option<u64>,
    drop_hint: Option<(String, bool)>,
    render_revision: Option<u64>,
    rendered_key: Option<SceneRenderKey>,
    displayed_key: Option<SceneRenderKey>,
    presentation_pending: bool,
    rendered_at: Option<f64>,
    rendered_frame: Option<u64>,
    render_count: u64,
    render_failure: Option<String>,
    physics: PhysicsOverlay,
}

#[derive(Default)]
struct PhysicsOverlay {
    available: bool,
    enabled: bool,
    cached: Option<(u64, Result<Vec<DebugLine>, String>)>,
}

#[derive(Clone, Copy, PartialEq)]
struct SceneRenderKey {
    revision: Option<u64>,
    camera: Camera,
    size: [usize; 2],
}

/// Frozen producer metadata travels with its completed pixels. The current
/// viewport camera can already belong to a newer request by the time it arrives.
pub struct PresentedSceneFrame {
    pub frame: Frame,
    pub camera: Camera,
    pub revision: Option<u64>,
    pub pending: bool,
}

fn render_size(rect: Rect, dpi: f32) -> [usize; 2] {
    let physical = rect.size() * dpi;
    [
        physical.x.round().max(1.) as usize,
        physical.y.round().max(1.) as usize,
    ]
}

fn next_presentation_owner() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn scene_owns_press(response: &egui::Response, pointer: Pos2) -> bool {
    let ctx = &response.ctx;
    let local_pointer = ctx
        .layer_transform_from_global(response.layer_id)
        .map_or(pointer, |transform| transform * pointer);
    if !response.enabled()
        || !response.interact_rect.contains(local_pointer)
        || !ctx.memory(|memory| memory.allows_interaction(response.layer_id))
        || ctx.layer_id_at(pointer) != Some(response.layer_id)
    {
        return false;
    }
    // egui's current interaction snapshot uses the final pointer position of
    // the input frame. Check origin ownership against the same previous-pass
    // widget rectangles, without letting a later same-layer control be crossed.
    ctx.viewport(|viewport| {
        let widgets = &viewport.prev_pass.widgets;
        let Some((layer, order)) = widgets.order(response.id) else {
            return false;
        };
        layer == response.layer_id
            && !widgets.get_layer(layer).skip(order + 1).any(|widget| {
                widget.enabled
                    && widget.sense.interactive()
                    && widget.interact_rect.contains(local_pointer)
            })
    })
}

fn camera_wheel_steps(event: &egui::Event, options: &egui::InputOptions) -> f32 {
    if let egui::Event::MouseWheel {
        unit,
        delta,
        modifiers,
        ..
    } = event
        && delta.is_finite()
    {
        let horizontal = modifiers.matches_any(options.horizontal_scroll_modifier);
        let vertical = modifiers.matches_any(options.vertical_scroll_modifier);
        if modifiers.matches_any(options.zoom_modifier) || (horizontal && !vertical) {
            return 0.;
        }
        let amount = if vertical && !horizontal {
            delta.x + delta.y
        } else {
            delta.y
        };
        match unit {
            egui::MouseWheelUnit::Point => amount / 40.,
            egui::MouseWheelUnit::Line => amount,
            egui::MouseWheelUnit::Page => amount * 10.,
        }
    } else {
        0.
    }
}

/// Route each wheel event to the RMB owner at that point in the event stream.
/// A complete press/wheel/release may arrive together in one UI frame.
fn camera_wheel_route(
    events: &[egui::Event],
    final_right_down: bool,
    was_flying: bool,
    options: &egui::InputOptions,
    owns_press: impl Fn(Pos2) -> bool,
) -> (f32, f32) {
    let right = egui::PointerButton::Secondary;
    let mut held = events
        .iter()
        .find_map(|event| match event {
            egui::Event::PointerButton {
                button, pressed, ..
            } if *button == right => Some(!pressed),
            _ => None,
        })
        .unwrap_or(final_right_down);
    let mut owned = was_flying && held;
    let (mut speed, mut zoom) = (0., 0.);
    for event in events {
        match event {
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                ..
            } if *button == right => {
                held = *pressed;
                owned = held && owns_press(*pos);
            }
            _ => {
                let steps = camera_wheel_steps(event, options);
                if owned {
                    speed += steps;
                } else if !held {
                    zoom += steps;
                }
            }
        }
    }
    (
        if speed.is_finite() { speed } else { 0. },
        if zoom.is_finite() { zoom } else { 0. },
    )
}

/// A fixed group box keeps every overlay control on the same 32-point row.
fn option_group(ui: &mut egui::Ui, width: f32, contents: impl FnOnce(&mut egui::Ui)) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, SCENE_GROUP_HEIGHT), Sense::hover());
    ui.painter()
        .rect_filled(rect, 4., crate::app::editor_ui::RAISED);
    let mut group = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink(CHROME_PAD as f32))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    group.set_clip_rect(ui.clip_rect().intersect(rect));
    crate::app::editor_ui::compact_spacing(&mut group);
    group.spacing_mut().interact_size.y = SCENE_TOOL_SIZE;
    contents(&mut group);
}

impl Default for SceneViewportState {
    fn default() -> Self {
        Self {
            camera: Camera {
                yaw: 0.55,
                pitch: 0.65,
                distance: 20.,
                target: [0.; 3],
            },
            camera_speed: scene_tools::DEFAULT_CAMERA_SPEED,
            tool: Tool::Translate,
            space: Space::World,
            snap: Snap::default(),
            gesture: None,
            rect: None,
            overlay_rects: vec![],
            overlay_targets: Default::default(),
            texture: None,
            gpu_texture: Default::default(),
            gpu_bridge: None,
            displayed_camera: None,
            presentation_owner: next_presentation_owner(),
            error: None,
            hovered_axis: None,
            locked: false,
            camera_drag: false,
            camera_fly: false,
            camera_input_frame: None,
            drop_hint: None,
            render_revision: None,
            rendered_key: None,
            displayed_key: None,
            presentation_pending: false,
            rendered_at: None,
            rendered_frame: None,
            render_count: 0,
            render_failure: None,
            physics: PhysicsOverlay::default(),
        }
    }
}
impl SceneViewportState {
    /// Process-local routing only; personal/authored serialization never stores it.
    pub fn presentation_owner(&self) -> u64 {
        self.presentation_owner
    }
    pub fn request_presentation_refresh(&mut self) {
        self.presentation_pending = true;
    }
    /// The same selected image painted by this viewport, independent of whether
    /// its representation is a software texture or a native GPU display.
    pub fn texture_id(&self) -> Option<egui::TextureId> {
        module_engine_player::ui::presentation_texture_id(&self.texture, &self.gpu_texture)
    }

    fn visible_camera(&self) -> Camera {
        self.displayed_camera.unwrap_or(self.camera)
    }
    /// Host revisions cover accepted scene/content/GPU changes. Camera and size
    /// are tracked here so interaction feedback never waits for the idle poll.
    pub fn set_render_revision(&mut self, revision: u64) {
        self.render_revision = Some(revision);
    }

    pub fn render_count(&self) -> u64 {
        self.render_count
    }

    pub fn set_physics_available(&mut self, available: bool) {
        if available {
            self.physics.available = true;
        } else {
            self.physics = PhysicsOverlay::default();
        }
    }

    pub fn refresh_physics(&mut self, revision: u64, world: &EcsWorld) {
        if !self.physics.enabled {
            self.physics.cached = None;
        } else if self
            .physics
            .cached
            .as_ref()
            .is_none_or(|(previous, _)| *previous != revision)
        {
            let lines = module_engine_physics_rapier::debug_lines(world)
                .map_err(|error| format!("Physics debug draw: {error}"));
            self.physics.cached = Some((revision, lines));
        }
    }

    pub fn with_camera(camera: Camera) -> Self {
        Self {
            camera,
            ..Self::default()
        }
    }
    /// Call before a scene switch, Play, Undo, or another document transition.
    pub fn cancel_gesture(&mut self) -> Option<SceneAction> {
        self.camera_drag = false;
        self.camera_fly = false;
        self.camera_input_frame = None;
        let gesture = self.gesture.take()?;
        self.hovered_axis = None;
        Some(SceneAction::CancelGesture {
            id: gesture.id,
            transform: gesture.cancel(),
        })
    }

    /// Allocate toolbar/canvas and return validated-by-host authoring requests.
    /// Only the focused canvas consumes G/R/S/F; text editors retain their input.
    pub fn interact(
        &mut self,
        ui: &mut egui::Ui,
        objects: &[WorldObject],
        snapshot: &SceneSnapshot,
        selected: Option<u64>,
        locked: bool,
    ) -> Vec<SceneAction> {
        let mut actions = vec![];
        let mut frame_selected = false;
        self.locked = locked;
        let size = ui.available_size().max(Vec2::splat(1.));
        let (rect, response) = crate::app::editor_ui::image_area(ui, size, Sense::click_and_drag());
        self.rect = Some(rect);
        frame_selected |= self.overlays(ui, rect, selected, locked);
        // Never test newly edited geometry against a retained older image.
        // An already active gesture keeps using its captured camera/geometry.
        let image_current = self.render_revision.is_none()
            || self.displayed_key.is_some_and(|key| {
                key.revision == self.render_revision
                    && key.size == render_size(rect, ui.ctx().pixels_per_point())
            });
        self.drop_hint = None;
        let pointer = ui.input(|input| input.pointer.interact_pos()).filter(|p| {
            self.gesture.is_some() || !self.overlay_rects.iter().any(|r| r.contains(*p))
        });
        let primary_pressed =
            ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Primary));
        // A press and movement can arrive in the same input frame. The latest
        // pointer position is for updating a gesture, not choosing its handle.
        let (primary_press, primary_release) = ui.input(|input| {
            let press = input
                .events
                .iter()
                .enumerate()
                .find_map(|(index, event)| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        ..
                    } => Some((index, *pos)),
                    _ => None,
                });
            let release_after = if self.gesture.is_some() {
                0
            } else {
                press.map_or(input.events.len(), |(index, _)| index + 1)
            };
            let release = input.events[release_after..]
                .iter()
                .find_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    } => Some(*pos),
                    _ => None,
                });
            (press.map(|(_, pos)| pos), release)
        });
        let primary_press = primary_press.filter(|pointer| {
            !self.overlay_rects.iter().any(|r| r.contains(*pointer))
                && scene_owns_press(&response, *pointer)
        });
        if primary_press.is_some()
            || (response.contains_pointer()
                && ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Middle)))
        {
            response.request_focus();
        }
        let keyboard = response.has_focus() && !ui.ctx().text_edit_focused();
        if keyboard && !ui.input(|input| input.pointer.button_down(egui::PointerButton::Secondary))
        {
            frame_selected |=
                ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F));
            if !locked && self.gesture.is_none() {
                for (key, tool) in [
                    (egui::Key::G, Tool::Translate),
                    (egui::Key::R, Tool::Rotate),
                    (egui::Key::S, Tool::Scale),
                ] {
                    if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
                        self.tool = tool;
                    }
                }
            }
        }
        let cancel = self.gesture.is_some()
            && (locked
                || !ui.input(|input| input.focused)
                || self.gesture.as_ref().is_some_and(|gesture| {
                    selected != Some(gesture.id)
                        || !objects.iter().any(|object| object.id == gesture.id)
                })
                // egui releases widget focus on Escape before our response is
                // read. An active gesture still owns Escape unless a text
                // editor has explicitly acquired keyboard focus.
                || (!ui.ctx().text_edit_focused()
                    && ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    })));
        let mut handled_gesture = false;
        if cancel {
            if let Some(action) = self.cancel_gesture() {
                actions.push(action);
            }
            handled_gesture = true;
        }
        if self.gesture.is_none() && !handled_gesture {
            self.camera_input(ui, &response, rect);
            if frame_selected && let Some(id) = selected {
                self.focus_object(objects, snapshot, id, rect);
            }
        }
        let geometry = (!locked && image_current && !self.camera_fly)
            .then(|| {
                selected.and_then(|id| {
                    GizmoGeometry::build(
                        objects,
                        id,
                        self.tool,
                        self.space,
                        self.visible_camera(),
                        rect,
                    )
                    .ok()
                })
            })
            .flatten();
        self.hovered_axis = pointer
            .filter(|pointer| rect.contains(*pointer))
            .and_then(|pointer| {
                geometry
                    .as_ref()
                    .and_then(|geometry| geometry.hit_test(pointer))
            });
        if self.hovered_axis.is_some() && !locked && self.gesture.is_none() && response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        let dragging_asset = egui::DragAndDrop::has_payload_of_type::<AssetDrag>(ui.ctx());
        let pressed_handle = primary_press
            .filter(|pointer| rect.contains(*pointer))
            .and_then(|pointer| {
                geometry
                    .as_ref()
                    .and_then(|geometry| geometry.hit_test(pointer))
                    .map(|axis| (axis, pointer))
            });
        if !locked
            && !handled_gesture
            && self.gesture.is_none()
            && !dragging_asset
            && primary_pressed
            && let (Some(id), Some((axis, pointer))) = (selected, pressed_handle)
        {
            match DragGesture::begin(
                objects,
                id,
                self.tool,
                self.space,
                axis,
                self.visible_camera(),
                rect,
                pointer,
            ) {
                Ok(gesture) => {
                    self.gesture = Some(gesture);
                    self.error = None;
                    actions.push(SceneAction::BeginGesture { id });
                }
                Err(error) => self.report_error(error, &mut actions),
            }
            handled_gesture = true;
        }
        if let Some(gesture) = &self.gesture {
            handled_gesture = true;
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            // Stop at this gesture's first release, even if another press or
            // pointer movement follows it in the same input frame.
            if let Some(pointer) = primary_release.or(pointer) {
                match gesture.update(pointer, self.snap) {
                    Ok(transform) => {
                        self.error = None;
                        actions.push(SceneAction::SetTransform {
                            id: gesture.id,
                            transform,
                        });
                    }
                    Err(error) => self.report_error(error, &mut actions),
                }
            }
            if primary_release.is_some()
                || !ui.input(|input| input.pointer.button_down(egui::PointerButton::Primary))
            {
                self.gesture = None;
                actions.push(SceneAction::CommitGesture);
            }
            ui.ctx().request_repaint();
        }
        if !handled_gesture
            && !self.camera_fly
            && image_current
            && response.clicked_by(egui::PointerButton::Primary)
            && !dragging_asset
            && let Some(pointer) = pointer
        {
            let id = scene_tools::camera_ray(self.visible_camera(), rect, pointer)
                .and_then(|ray| scene_tools::pick(snapshot, ray))
                .map(|hit| hit.id);
            actions.push(SceneAction::Select(id));
        }
        if !self.camera_fly {
            self.asset_drop(ui, &response, rect, snapshot, image_current, &mut actions);
        }
        actions
    }

    /// Draw only after the host has applied interact() actions and updated its snapshot.
    #[cfg(any(test, feature = "verification"))]
    pub fn paint(
        &mut self,
        ui: &mut egui::Ui,
        objects: &[WorldObject],
        snapshot: &SceneSnapshot,
        selected: Option<u64>,
        render: &mut dyn FnMut(Camera, usize, usize) -> Result<Frame, String>,
    ) {
        let revision = self.render_revision;
        self.paint_presented(
            ui,
            objects,
            snapshot,
            selected,
            &mut |camera, width, height| {
                render(camera, width, height).map(|frame| {
                    Some(PresentedSceneFrame {
                        frame,
                        camera,
                        revision,
                        pending: false,
                    })
                })
            },
        );
    }

    pub fn set_gpu_presentation(
        &mut self,
        bridge: Option<
            std::sync::Arc<module_engine_player::gpu_presentation::GpuPresentationBridge>,
        >,
    ) {
        self.gpu_bridge = bridge;
    }

    pub fn paint_presented(
        &mut self,
        ui: &mut egui::Ui,
        objects: &[WorldObject],
        snapshot: &SceneSnapshot,
        selected: Option<u64>,
        render: &mut dyn FnMut(Camera, usize, usize) -> Result<Option<PresentedSceneFrame>, String>,
    ) {
        let Some(rect) = self
            .rect
            .filter(|rect| rect.width() > 2. && rect.height() > 2.)
        else {
            return;
        };
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        painter.rect_filled(rect, 0., Color32::from_rgb(21, 29, 43));
        let dpi = ui.ctx().pixels_per_point();
        let [width, height] = render_size(rect, dpi);
        let now = ui.input(|input| input.time);
        let frame_number = ui.ctx().cumulative_frame_nr();
        let key = SceneRenderKey {
            revision: self.render_revision,
            camera: self.camera,
            size: [width, height],
        };
        let expired = self
            .rendered_at
            .is_none_or(|previous| now < previous || now - previous >= 0.25);
        let untracked_frame =
            self.render_revision.is_none() && self.rendered_frame != Some(frame_number);
        if self.presentation_pending || self.rendered_key != Some(key) || expired || untracked_frame
        {
            self.rendered_key = Some(key);
            self.rendered_at = Some(now);
            self.rendered_frame = Some(frame_number);
            self.render_count += 1;
            match render(self.camera, width, height) {
                Ok(Some(presented)) if valid_frame(&presented.frame) => {
                    let frame = &presented.frame;
                    self.presentation_pending = presented.pending;
                    match module_engine_player::ui::update_presented_texture(
                        ui,
                        &mut self.texture,
                        &mut self.gpu_texture,
                        self.gpu_bridge.as_deref(),
                        frame,
                    ) {
                        Ok(true) => {
                            if std::env::var_os("MODULE_ENGINE_SCENE_IMAGE_TRACE").is_some() {
                                eprintln!(
                                    "Scene image: {}",
                                    serde_json::json!({"logical_area":[rect.width(),rect.height()],"pixels_per_point":dpi,"requested_pixels":[width,height],"displayed_frame_pixels":[frame.width,frame.height],"settled_1_to_1":([width,height]==[frame.width,frame.height]),"pending":presented.pending,"gpu_resident":frame.gpu.is_some()})
                                );
                            }
                            self.displayed_camera = Some(presented.camera);
                            let completed_key = SceneRenderKey {
                                revision: presented.revision,
                                camera: presented.camera,
                                size: [frame.width, frame.height],
                            };
                            self.displayed_key = Some(completed_key);
                            self.rendered_key = Some(completed_key);
                            if presented.pending || completed_key != key {
                                ui.ctx().request_repaint();
                            }
                            if self
                                .render_failure
                                .as_ref()
                                .is_some_and(|error| self.error.as_ref() == Some(error))
                            {
                                self.error = None;
                            }
                            self.render_failure = None;
                        }
                        Ok(false) => {
                            self.rendered_key = None;
                            ui.ctx()
                                .request_repaint_after(std::time::Duration::from_millis(1));
                        }
                        Err(error) => {
                            self.render_failure = Some(error.clone());
                            self.error = Some(error);
                        }
                    }
                }
                Ok(None) => {
                    self.presentation_pending = true;
                    self.rendered_key = None;
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(1));
                }
                Ok(Some(_)) => {
                    self.presentation_pending = false;
                    let error = "Scene renderer returned malformed frame pixels".to_owned();
                    self.render_failure = Some(error.clone());
                    self.error = Some(error);
                }
                Err(error) => {
                    self.presentation_pending = false;
                    self.render_failure = Some(error.clone());
                    self.error = Some(error);
                }
            }
        }
        if self.render_revision.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
        }
        if let Some(texture) = self.texture_id() {
            painter.image(
                texture,
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
        }
        let image_current = self.render_revision.is_none()
            || self.displayed_key.is_some_and(|displayed| {
                displayed.revision == self.render_revision && displayed.size == [width, height]
            });
        if image_current
            && self.texture_id().is_some()
            && let Some((_, Ok(lines))) = &self.physics.cached
        {
            let camera = self.visible_camera();
            for line in lines {
                if let (Some(start), Some(end)) = (
                    scene_tools::project_point(camera, rect, line.start),
                    scene_tools::project_point(camera, rect, line.end),
                ) {
                    let color = match line.kind {
                        DebugLineKind::Body => Color32::from_rgb(182, 196, 213),
                        DebugLineKind::Collider => Color32::from_rgb(102, 214, 196),
                        DebugLineKind::Sensor => Color32::from_rgb(131, 181, 243),
                        DebugLineKind::Joint => Color32::from_rgb(241, 196, 119),
                    };
                    painter.line_segment([start, end], Stroke::new(1.25, color));
                }
            }
        }
        if image_current && let Some(id) = selected {
            if let Some(bounds) = object_bounds(snapshot, id) {
                draw_bounds(&painter, self.visible_camera(), rect, bounds);
            }
            if !self.locked
                && let Ok(geometry) = GizmoGeometry::build(
                    objects,
                    id,
                    self.tool,
                    self.space,
                    self.visible_camera(),
                    rect,
                )
            {
                let active = self.gesture.as_ref().map(|gesture| gesture.axis);
                for axis in &geometry.axes {
                    if !axis.direction.iter().all(|value| value.is_finite()) {
                        continue;
                    }
                    let color = axis_color(axis.axis, active, self.hovered_axis);
                    painter.line_segment([axis.start, axis.end], Stroke::new(3., color));
                    match self.tool {
                        Tool::Scale => {
                            painter.rect_filled(
                                Rect::from_center_size(axis.end, Vec2::splat(9.)),
                                1.,
                                color,
                            );
                        }
                        _ => {
                            let direction = axis.end - axis.start;
                            if direction.length_sq() > 1. {
                                let direction = direction.normalized();
                                let normal = Vec2::new(-direction.y, direction.x);
                                painter.add(egui::Shape::convex_polygon(
                                    vec![
                                        axis.end,
                                        axis.end - direction * 12. + normal * 5.,
                                        axis.end - direction * 12. - normal * 5.,
                                    ],
                                    color,
                                    Stroke::NONE,
                                ));
                            }
                        }
                    }
                }
                for ring in &geometry.rings {
                    let color = axis_color(ring.axis, active, self.hovered_axis);
                    for &segment in &ring.segments {
                        painter.line_segment(
                            segment,
                            Stroke::new(if active == Some(ring.axis) { 3. } else { 2. }, color),
                        );
                    }
                }
                painter.circle_filled(geometry.center, 3., Color32::WHITE);
            }
        }
        if let Some((hint, allowed)) = &self.drop_hint {
            let color = if *allowed {
                Color32::from_rgb(89, 220, 158)
            } else {
                Color32::from_rgb(245, 145, 135)
            };
            painter.rect_stroke(
                rect.shrink(2.),
                0.,
                Stroke::new(2., color),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.center_top() + Vec2::new(0., 22.),
                egui::Align2::CENTER_CENTER,
                hint,
                FontId::proportional(13.),
                color,
            );
        }
        let help = if self.gesture.is_some() {
            "Esc: cancel · release LMB: apply"
        } else {
            "RMB: WASD fly · Q/E down/up · wheel speed · MMB orbit · Shift+MMB pan · wheel zoom · F focus"
        };
        painter.text(
            rect.left_bottom() + Vec2::new(9., -9.),
            egui::Align2::LEFT_BOTTOM,
            help,
            FontId::proportional(11.),
            Color32::from_rgb(187, 201, 216),
        );
        let physics_error = self
            .physics
            .cached
            .as_ref()
            .and_then(|(_, lines)| lines.as_ref().err());
        if let Some(error) = self.error.as_ref().or(physics_error) {
            painter.text(
                rect.left_top() + Vec2::splat(9.),
                egui::Align2::LEFT_TOP,
                error,
                FontId::proportional(12.),
                Color32::from_rgb(245, 145, 135),
            );
        }
    }

    fn orientation_controls(&mut self, ui: &mut egui::Ui) {
        use crate::app::editor_ui::{self, Icon};
        let (icon, help) = match self.space {
            Space::World => (Icon::World, "World axes · click to switch to Local axes"),
            Space::Local => (Icon::Local, "Local axes · click to switch to World axes"),
        };
        let response = editor_ui::icon_button_sized(
            ui,
            icon,
            help,
            true,
            self.space == Space::Local,
            SCENE_TOOL_SIZE,
        );
        self.overlay_targets
            .insert("Orientation".into(), response.rect);
        if response.clicked() {
            self.space = match self.space {
                Space::World => Space::Local,
                Space::Local => Space::World,
            };
        }
    }

    fn camera_speed_control(&mut self, ui: &mut egui::Ui) {
        let response = ui
            .scope(|ui| {
                // Keep "Speed 1000.0" readable inside the fixed compact field.
                // DragValue otherwise expands past it using the form's 8px padding.
                ui.spacing_mut().button_padding.x = CHROME_GAP;
                ui.add_sized(
                    egui::vec2(SCENE_SPEED_WIDTH, SCENE_TOOL_SIZE),
                    egui::DragValue::new(&mut self.camera_speed)
                        .speed(0.1)
                        .range(scene_tools::MIN_CAMERA_SPEED..=scene_tools::MAX_CAMERA_SPEED)
                        .fixed_decimals(1)
                        .prefix("Speed "),
                )
            })
            .inner
            .on_hover_text(
                "Camera flight speed · world units/s · Hold RMB and scroll to change speed",
            );
        self.overlay_targets
            .insert("CameraSpeed".into(), response.rect);
    }

    fn snap_steps(&mut self, ui: &mut egui::Ui) {
        ui.strong("Snap steps");
        for (name, value, range, prefix, suffix) in [
            (
                "MoveStep",
                &mut self.snap.translation,
                0.0001..=100000.,
                "Move: ",
                "",
            ),
            (
                "RotateStep",
                &mut self.snap.rotation_degrees,
                0.1..=180.,
                "Rotate: ",
                "°",
            ),
            (
                "ScaleStep",
                &mut self.snap.scale,
                0.001..=100.,
                "Scale: ",
                "",
            ),
        ] {
            let response = ui.add(
                egui::DragValue::new(value)
                    .range(range)
                    .prefix(prefix)
                    .suffix(suffix),
            );
            self.overlay_targets.insert(name.into(), response.rect);
        }
    }

    fn snap_controls(&mut self, ui: &mut egui::Ui, compact: bool) {
        use crate::app::editor_ui::{self, Icon};
        // Popup areas do not inherit enabled state from their invoking Ui.
        let enabled = ui.is_enabled();
        ui.horizontal(|ui| {
            let response = editor_ui::icon_button_sized(
                ui,
                Icon::Snap,
                "Snap · enable move, rotate and scale increments",
                true,
                self.snap.enabled,
                SCENE_TOOL_SIZE,
            );
            self.overlay_targets.insert("Snap".into(), response.rect);
            if response.clicked() {
                self.snap.enabled = !self.snap.enabled;
            }
            if compact {
                // A real submenu keeps the compact parent open. A standalone
                // Popup::menu would replace that parent in egui's popup memory.
                let (response, _) = egui::containers::menu::SubMenuButton::new("Snap steps")
                    .config(
                        egui::containers::menu::MenuConfig::new()
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                    )
                    .ui(ui, |ui| {
                        ui.add_enabled_ui(enabled, |ui| self.snap_steps(ui));
                    });
                self.overlay_targets
                    .insert("SnapSettings".into(), response.rect);
                return;
            }
            let response = editor_ui::icon_button_sized(
                ui,
                Icon::Settings,
                "Snap settings · configure move, rotate and scale increments",
                true,
                false,
                SCENE_TOOL_SIZE,
            );
            self.overlay_targets
                .insert("SnapSettings".into(), response.rect);
            if let Some(popup) = egui::Popup::menu(&response)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.add_enabled_ui(enabled, |ui| self.snap_steps(ui));
                })
            {
                self.overlay_rects.push(popup.response.rect);
            }
        });
    }

    fn physics_toggle(&mut self, ui: &mut egui::Ui, enabled: bool) {
        let response = ui
            .add_enabled(
                enabled,
                egui::Button::new("Physics")
                    .selected(self.physics.enabled)
                    .min_size(egui::vec2(66., SCENE_TOOL_SIZE)),
            )
            .on_hover_text("Show or hide authored rigid bodies, colliders, sensors and joints");
        self.overlay_targets.insert("Physics".into(), response.rect);
        if response.clicked() {
            self.physics.enabled = !self.physics.enabled;
        }
    }

    fn overlays(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        selected: Option<u64>,
        locked: bool,
    ) -> bool {
        use crate::app::editor_ui::{self, Icon};
        self.overlay_rects.clear();
        self.overlay_targets.clear();
        if rect.width() < 64. || rect.height() < 28. {
            return false;
        }
        let ctx = ui.ctx().clone();
        let editing = ui.is_enabled() && !locked && self.gesture.is_none() && !self.camera_fly;
        let navigation = ui.is_enabled() && self.gesture.is_none() && !self.camera_fly;
        let can_toggle_physics = navigation && !self.camera_drag;
        let can_frame =
            ui.is_enabled() && selected.is_some() && self.gesture.is_none() && !self.camera_fly;
        let surface = || {
            egui::Frame::new()
                .fill(editor_ui::RAISED)
                .inner_margin(CHROME_PAD)
                .corner_radius(4)
        };
        let tools = [
            (Tool::Translate, Icon::Move, "Move · G"),
            (Tool::Rotate, Icon::Rotate, "Rotate · R"),
            (Tool::Scale, Icon::Scale, "Scale · S"),
        ];
        let mut frame = false;
        if rect.width() < 400. || rect.height() < 160. {
            let compact =
                egui::Area::new(ui.make_persistent_id(("scene-compact", self.presentation_owner)))
                    .order(egui::Order::Foreground)
                    .enabled(ui.is_enabled())
                    .fixed_pos(rect.min + Vec2::splat(4.))
                    .constrain_to(rect)
                    .show(&ctx, |ui| {
                        surface().show(ui, |ui| {
                            let (menu, _) = egui::containers::menu::MenuButton::new(format!(
                                "Scene · {:?}",
                                self.tool
                            ))
                            .config(
                                egui::containers::menu::MenuConfig::new()
                                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                            )
                            .ui(ui, |ui| {
                                ui.add_enabled_ui(editing, |ui| {
                                    for (tool, _, label) in tools {
                                        ui.selectable_value(&mut self.tool, tool, label);
                                    }
                                    ui.separator();
                                    ui.strong("Transform axes");
                                    self.orientation_controls(ui);
                                    ui.separator();
                                    ui.strong("Snapping");
                                    self.snap_controls(ui, true);
                                });
                                ui.separator();
                                ui.strong("Camera");
                                ui.add_enabled_ui(navigation, |ui| self.camera_speed_control(ui));
                                if self.physics.available {
                                    ui.separator();
                                    self.physics_toggle(ui, can_toggle_physics);
                                }
                                ui.separator();
                                if ui
                                    .add_enabled(can_frame, egui::Button::new("Frame selected · F"))
                                    .clicked()
                                {
                                    frame = true;
                                    ui.close();
                                }
                            });
                            self.overlay_targets.insert("Compact".into(), menu.rect);
                        });
                    });
            self.overlay_rects.push(compact.response.rect);
            return frame;
        }
        let rail = egui::Area::new(ui.make_persistent_id(("scene-tools", self.presentation_owner)))
            .order(egui::Order::Foreground)
            .enabled(ui.is_enabled())
            .fixed_pos(rect.min + Vec2::splat(CHROME_GAP))
            .constrain_to(rect)
            .show(&ctx, |ui| {
                surface().show(ui, |ui| {
                    editor_ui::compact_spacing(ui);
                    ui.vertical(|ui| {
                        for (tool, icon, label) in tools {
                            let response = editor_ui::icon_button_sized(
                                ui,
                                icon,
                                label,
                                editing,
                                self.tool == tool,
                                SCENE_TOOL_SIZE,
                            );
                            self.overlay_targets
                                .insert(format!("{tool:?}"), response.rect);
                            if response.clicked() {
                                self.tool = tool;
                            }
                        }
                    });
                });
            });
        self.overlay_rects.push(rail.response.rect);
        let speed_width = SCENE_SPEED_WIDTH + 2. * CHROME_PAD as f32;
        let snap_width = 2. * SCENE_TOOL_SIZE + CHROME_GAP + 2. * CHROME_PAD as f32;
        let physics_width = 66. + 2. * CHROME_PAD as f32;
        let mut right_width = 2. * SCENE_GROUP_HEIGHT + speed_width + snap_width + 3. * CHROME_GAP;
        if self.physics.available {
            right_width += physics_width + CHROME_GAP;
        }
        let right =
            egui::Area::new(ui.make_persistent_id(("scene-options", self.presentation_owner)))
                .order(egui::Order::Foreground)
                .enabled(ui.is_enabled())
                .fixed_pos(egui::pos2(
                    rect.right() - CHROME_GAP - right_width,
                    rect.top() + CHROME_GAP,
                ))
                .constrain_to(rect)
                .show(&ctx, |ui| {
                    editor_ui::compact_spacing(ui);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                        option_group(ui, SCENE_GROUP_HEIGHT, |ui| {
                            ui.add_enabled_ui(editing, |ui| self.orientation_controls(ui));
                        });
                        option_group(ui, speed_width, |ui| {
                            ui.add_enabled_ui(navigation, |ui| self.camera_speed_control(ui));
                        });
                        option_group(ui, snap_width, |ui| {
                            ui.add_enabled_ui(editing, |ui| self.snap_controls(ui, false));
                        });
                        option_group(ui, SCENE_GROUP_HEIGHT, |ui| {
                            let response = editor_ui::icon_button_sized(
                                ui,
                                Icon::Frame,
                                "Frame selected · F",
                                can_frame,
                                false,
                                SCENE_TOOL_SIZE,
                            );
                            self.overlay_targets.insert("Frame".into(), response.rect);
                            frame |= response.clicked();
                        });
                        if self.physics.available {
                            option_group(ui, physics_width, |ui| {
                                self.physics_toggle(ui, can_toggle_physics);
                            });
                        }
                    });
                });
        self.overlay_rects.push(right.response.rect);
        frame
    }

    fn report_error(&mut self, error: String, actions: &mut Vec<SceneAction>) {
        if self.error.as_ref() != Some(&error) {
            actions.push(SceneAction::Error(error.clone()));
        }
        self.error = Some(error);
    }
    fn camera_input(&mut self, ui: &egui::Ui, response: &egui::Response, rect: Rect) {
        let middle = egui::PointerButton::Middle;
        let right = egui::PointerButton::Secondary;
        let frame = ui.ctx().cumulative_frame_nr();
        // A hidden area must not resume an old held-button capture on return.
        if self.camera_input_frame.is_some_and(|last| frame > last + 1) {
            self.camera_fly = false;
            self.camera_drag = false;
        }
        self.camera_input_frame = Some(frame);
        if !ui.is_enabled() || !ui.input(|i| i.focused) || egui::Popup::is_any_open(ui.ctx()) {
            self.camera_drag = false;
            self.camera_fly = false;
            return;
        }
        let was_flying = self.camera_fly;
        if was_flying
            && (!response.has_focus()
                || ui.ctx().text_edit_focused()
                || ui.input(|i| i.pointer.latest_pos().is_none()))
        {
            self.camera_fly = false;
            return;
        }
        let can_start = !self.camera_drag
            && !egui::DragAndDrop::has_any_payload(ui.ctx())
            && !ui.input(|i| {
                i.pointer.button_down(egui::PointerButton::Primary) || i.pointer.button_down(middle)
            });
        let owns_press = |pointer| {
            can_start
                && !self.overlay_rects.iter().any(|r| r.contains(pointer))
                && scene_owns_press(response, pointer)
        };
        // Ownership probes read Context layers/memory. Do not run them while
        // Context::input holds its lock; copy only the small navigation events.
        let (wheel_events, final_right_down) = ui.input(|input| {
            (
                input
                    .events
                    .iter()
                    .filter(|event| {
                        matches!(
                            event,
                            egui::Event::MouseWheel { .. }
                                | egui::Event::PointerButton {
                                    button: egui::PointerButton::Secondary,
                                    ..
                                }
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
                input.pointer.button_down(right),
            )
        });
        let wheel_options = ui.ctx().options(|options| options.input_options);
        let (speed_wheel, zoom_wheel) = camera_wheel_route(
            &wheel_events,
            final_right_down,
            was_flying,
            &wheel_options,
            owns_press,
        );
        let right_press = wheel_events.iter().rev().find_map(|event| match event {
            egui::Event::PointerButton {
                pos,
                button,
                pressed: true,
                ..
            } if *button == right => Some(*pos),
            _ => None,
        });
        let starting_fly =
            right_press.is_some_and(owns_press) && ui.input(|i| i.pointer.button_down(right));
        if ui.input(|i| i.pointer.button_released(right) || !i.pointer.button_down(right)) {
            self.camera_fly = false;
        }
        if starting_fly {
            response.request_focus();
            self.camera_fly = true;
        }
        if ui.ctx().text_edit_focused() {
            self.camera_drag = false;
            self.camera_fly = false;
            return;
        }
        if self.camera_fly
            && (!response.has_focus()
                || ui.input(|i| i.pointer.latest_pos().is_none())
                || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)))
        {
            self.camera_fly = false;
            return;
        }
        if speed_wheel != 0. {
            self.camera_speed = scene_tools::camera_speed(
                self.camera_speed * 1.25_f32.powf(speed_wheel.clamp(-64., 64.)),
            );
            ui.input_mut(|input| input.smooth_scroll_delta = Vec2::ZERO);
            ui.ctx().request_repaint();
        }
        if zoom_wheel != 0.
            && response.hovered()
            && !ui
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|pointer| self.overlay_rects.iter().any(|r| r.contains(pointer)))
        {
            self.camera.distance =
                (self.camera.distance * (-zoom_wheel * 0.08).exp()).clamp(0.2, 1000.);
        }
        if self.camera_fly {
            let (look, movement, seconds) = ui.input(|input| {
                let axis = |positive, negative| {
                    i32::from(input.key_down(positive)) as f32
                        - i32::from(input.key_down(negative)) as f32
                };
                (
                    if starting_fly {
                        Vec2::ZERO
                    } else {
                        input.pointer.motion().unwrap_or(input.pointer.delta())
                    },
                    [
                        axis(egui::Key::D, egui::Key::A),
                        axis(egui::Key::E, egui::Key::Q),
                        axis(egui::Key::W, egui::Key::S),
                    ],
                    input.stable_dt,
                )
            });
            scene_tools::fly_camera(
                &mut self.camera,
                rect,
                look,
                movement,
                self.camera_speed,
                seconds,
            );
            ui.input_mut(|input| input.smooth_scroll_delta = Vec2::ZERO);
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            ui.ctx().request_repaint();
            return;
        }
        if response.drag_started_by(middle) {
            self.camera_drag = true;
        }
        if ui.input(|i| i.pointer.button_released(middle) || !i.pointer.button_down(middle)) {
            self.camera_drag = false;
        }
        let over_overlay = ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| self.overlay_rects.iter().any(|r| r.contains(p)));
        if over_overlay && !self.camera_drag {
            return;
        }
        if self.camera_drag {
            let delta = ui.input(|input| input.pointer.delta());
            if ui.input(|input| input.modifiers.shift) {
                if let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
                    && let Some(center_ray) =
                        scene_tools::camera_ray(self.camera, rect, rect.center())
                    && let Some(previous) = scene_tools::camera_ray(
                        self.camera,
                        rect,
                        pointer - delta,
                    )
                    .and_then(|ray| {
                        scene_tools::plane_hit(ray, self.camera.target, center_ray.direction)
                    })
                    && let Some(current) = scene_tools::camera_ray(self.camera, rect, pointer)
                        .and_then(|ray| {
                            scene_tools::plane_hit(ray, self.camera.target, center_ray.direction)
                        })
                {
                    for index in 0..3 {
                        self.camera.target[index] += previous[index] - current[index];
                    }
                }
            } else {
                self.camera.yaw -= delta.x * 0.009;
                self.camera.pitch = (self.camera.pitch + delta.y * 0.007).clamp(-1.45, 1.45);
            }
        }
    }
    fn focus_object(
        &mut self,
        objects: &[WorldObject],
        snapshot: &SceneSnapshot,
        id: u64,
        rect: Rect,
    ) {
        if let Some([min, max]) = object_bounds(snapshot, id) {
            self.camera.target = std::array::from_fn(|index| (min[index] + max[index]) * 0.5);
            let radius = (0..3)
                .map(|index| ((max[index] - min[index]) * 0.5).powi(2))
                .sum::<f32>()
                .sqrt()
                .max(0.1);
            self.camera.distance =
                (radius / (55_f32.to_radians() * 0.5).tan() / rect.aspect_ratio().min(1.) * 1.5)
                    .clamp(0.2, 1000.);
        } else if let Ok(matrices) = scene_tools::object_matrices(objects, id) {
            self.camera.target = std::array::from_fn(|row| matrices.world[row][3]);
        }
    }
    fn asset_drop(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        snapshot: &SceneSnapshot,
        image_current: bool,
        actions: &mut Vec<SceneAction>,
    ) {
        let locked = self.locked;
        if ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| self.overlay_rects.iter().any(|r| r.contains(p)))
        {
            return;
        }
        if let Some(payload) = response.dnd_hover_payload::<AssetDrag>() {
            let pointer = ui.input(|input| input.pointer.interact_pos());
            let ray = pointer
                .and_then(|pointer| scene_tools::camera_ray(self.visible_camera(), rect, pointer));
            let (hint, allowed) = if locked {
                ("Editing is available after Stop", false)
            } else if !image_current {
                ("Scene image is updating", false)
            } else {
                match payload.kind {
                    AssetKind::Model => (
                        "Drop to create an object with the model",
                        ray.and_then(|ray| scene_tools::placement(snapshot, ray))
                            .is_some(),
                    ),
                    AssetKind::Texture => (
                        "Drop onto an object to assign the texture",
                        ray.and_then(|ray| scene_tools::pick(snapshot, ray))
                            .is_some(),
                    ),
                    _ => ("This asset cannot be assigned through Scene", false),
                }
            };
            self.drop_hint = Some((hint.into(), allowed));
        }
        if let Some(payload) = response.dnd_release_payload::<AssetDrag>() {
            if locked || !image_current {
                return;
            }
            let Some(ray) = ui
                .input(|input| input.pointer.interact_pos())
                .and_then(|pointer| scene_tools::camera_ray(self.visible_camera(), rect, pointer))
            else {
                return;
            };
            match payload.kind {
                AssetKind::Model => {
                    if let Some(point) = scene_tools::placement(snapshot, ray) {
                        actions.push(SceneAction::DropModel {
                            asset_ref: payload.reference.clone(),
                            point,
                        });
                    } else {
                        self.report_error(
                            "No surface or ground under the cursor to place the model".into(),
                            actions,
                        );
                    }
                }
                AssetKind::Texture => {
                    if let Some(hit) = scene_tools::pick(snapshot, ray) {
                        actions.push(SceneAction::DropTexture {
                            asset_ref: payload.reference.clone(),
                            id: hit.id,
                        });
                    } else {
                        self.report_error(
                            "Drag the texture onto an existing object".into(),
                            actions,
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

fn valid_frame(frame: &Frame) -> bool {
    frame.is_valid()
}
fn axis_color(axis: Axis, active: Option<Axis>, hovered: Option<Axis>) -> Color32 {
    if active == Some(axis) || hovered == Some(axis) {
        return Color32::from_rgb(255, 219, 79);
    }
    match axis {
        Axis::X => Color32::from_rgb(235, 83, 80),
        Axis::Y => Color32::from_rgb(111, 215, 103),
        Axis::Z => Color32::from_rgb(99, 161, 249),
    }
}
fn object_bounds(snapshot: &SceneSnapshot, id: u64) -> Option<[[f32; 3]; 2]> {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut found = false;
    for item in snapshot.items.iter().filter(|item| item.id == id) {
        for &vertex in &item.mesh.vertices {
            let point = transform_point(item.model, vertex);
            if !point.iter().all(|value| value.is_finite()) {
                continue;
            }
            for index in 0..3 {
                min[index] = min[index].min(point[index]);
                max[index] = max[index].max(point[index]);
            }
            found = true;
        }
    }
    found.then_some([min, max])
}
fn draw_bounds(painter: &egui::Painter, camera: Camera, rect: Rect, [min, max]: [[f32; 3]; 2]) {
    let corners: [Option<Pos2>; 8] = std::array::from_fn(|index| {
        scene_tools::project_point(
            camera,
            rect,
            std::array::from_fn(|axis| {
                if index & (1 << axis) == 0 {
                    min[axis]
                } else {
                    max[axis]
                }
            }),
        )
    });
    for index in 0..8 {
        for axis in 0..3 {
            let other = index ^ (1 << axis);
            if index < other
                && let (Some(a), Some(b)) = (corners[index], corners[other])
            {
                painter.line_segment([a, b], Stroke::new(1.5, Color32::from_rgb(255, 185, 63)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scene_default_uses_all_physical_pixels_without_the_old_720p_cap() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600., 1000.));
        assert_eq!(render_size(rect, 1.), [1600, 1000]);
        assert_eq!(render_size(rect, 1.5), [2400, 1500]);
        assert_eq!(render_size(rect, 2.), [3200, 2000]);
        let portrait = Rect::from_min_size(Pos2::ZERO, Vec2::new(768., 1080.));
        assert_eq!(render_size(portrait, 1.), [768, 1080]);
    }
    #[test]
    fn large_scene_painter_requests_full_physical_area() {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.5);
        let mut state = SceneViewportState::default();
        let mut requested = [0, 0];
        let mut raw = input(0., vec![]);
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600., 1000.)));
        // A native renderer supplies this limit. Bound the root child because
        // egui's initial sizing pass otherwise exposes a 10000px probe surface.
        raw.max_texture_side = Some(4096);
        ctx.run_ui(raw, |ui| {
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600., 1000.))),
                |ui| {
                    state.interact(ui, &objects(), &snapshot(), Some(7), false);
                    state.paint(
                        ui,
                        &objects(),
                        &snapshot(),
                        Some(7),
                        &mut |_camera, width, height| {
                            requested = [width, height];
                            Ok(Frame {
                                width,
                                height,
                                rgba: vec![255; width * height * 4],
                                gpu: None,
                                timings: None,
                                stats: RenderStats::default(),
                            })
                        },
                    );
                },
            );
        })
        .drop_without_applying_deltas();
        let physical = state.rect.unwrap().size() * 1.5;
        assert_eq!(
            requested,
            [physical.x.round() as usize, physical.y.round() as usize]
        );
        assert_eq!(requested, [2400, 1500]);
    }
    #[test]
    fn fractional_dock_rect_uses_pixel_aligned_image_and_input_at_each_dpi() {
        for dpi in [1., 1.5, 2.] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(dpi);
            let mut state = SceneViewportState::default();
            let mut requested = [0, 0];
            let output = ctx.run_ui(input(0., vec![]), |ui| {
                let mut area = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(
                    Pos2::new(10.37, 12.23),
                    Vec2::new(601.2, 403.3),
                )));
                state.interact(&mut area, &objects(), &snapshot(), None, false);
                state.paint(
                    &mut area,
                    &objects(),
                    &snapshot(),
                    None,
                    &mut |_, width, height| {
                        requested = [width, height];
                        Ok(Frame {
                            width,
                            height,
                            rgba: vec![255; width * height * 4],
                            gpu: None,
                            timings: None,
                            stats: RenderStats::default(),
                        })
                    },
                );
            });
            let rect = state.rect.unwrap();
            for coordinate in [rect.min.x, rect.min.y, rect.max.x, rect.max.y] {
                assert!((coordinate * dpi - (coordinate * dpi).round()).abs() < 0.001);
            }
            assert_eq!(
                requested,
                [
                    (rect.width() * dpi).round() as usize,
                    (rect.height() * dpi).round() as usize
                ]
            );
            let texture = state.texture.as_ref().unwrap().id();
            let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
            let mesh = primitives
                .iter()
                .find_map(|shape| match &shape.primitive {
                    egui::epaint::Primitive::Mesh(mesh) if mesh.texture_id == texture => Some(mesh),
                    _ => None,
                })
                .expect("Scene image mesh");
            assert_eq!(
                mesh.calc_bounds(),
                rect,
                "Image quad and Scene input must share one rect"
            );
            output.drop_without_applying_deltas();
        }
    }
    use module_engine_core::{Material, Mesh, RenderItem, RenderStats};
    fn objects() -> Vec<WorldObject> {
        vec![WorldObject {
            id: 7,
            parent: None,
            transform: Transform::default(),
            color: [255; 4],
        }]
    }
    fn snapshot() -> SceneSnapshot {
        SceneSnapshot {
            items: vec![RenderItem {
                id: 7,
                mesh: Mesh::cube(),
                model: Transform::default().matrix(),
                color: [255; 4],
                material: Material::default(),
            }],
            time: 0.,
        }
    }
    fn input(time: f64, events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            time: Some(time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 700.))),
            events,
            ..Default::default()
        }
    }
    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn frame(
        ctx: &egui::Context,
        state: &mut SceneViewportState,
        events: Vec<egui::Event>,
        selected: Option<u64>,
        locked: bool,
        time: f64,
    ) -> Vec<SceneAction> {
        frame_with_overlay(ctx, state, events, selected, locked, time, None)
    }
    #[derive(Clone, Copy)]
    enum SceneOverlay {
        Foreground,
        SameLayer,
    }
    fn frame_with_overlay(
        ctx: &egui::Context,
        state: &mut SceneViewportState,
        events: Vec<egui::Event>,
        selected: Option<u64>,
        locked: bool,
        time: f64,
        overlay: Option<(SceneOverlay, Rect)>,
    ) -> Vec<SceneAction> {
        let mut actions = vec![];
        let output = ctx.run_ui(input(time, events), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                actions = state.interact(ui, &objects(), &snapshot(), selected, locked);
                if let Some((SceneOverlay::SameLayer, rect)) = overlay {
                    ui.interact(
                        rect,
                        egui::Id::new("scene-press-overlay"),
                        Sense::click_and_drag(),
                    );
                }
            });
            if let Some((SceneOverlay::Foreground, rect)) = overlay {
                egui::Area::new(egui::Id::new("scene-press-overlay-area"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(rect.min)
                    .movable(false)
                    .show(ui.ctx(), |ui| {
                        ui.allocate_exact_size(rect.size(), Sense::click_and_drag());
                    });
            }
        });
        output.drop_without_applying_deltas();
        actions
    }
    #[test]
    fn scene_click_picks_actual_object_and_empty_click_clears_selection() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], None, false, 0.);
        let center =
            scene_tools::project_point(state.camera, state.rect.unwrap(), [0.; 3]).unwrap();
        frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(center), button(center, true)],
            None,
            false,
            0.1,
        );
        let actions = frame(
            &ctx,
            &mut state,
            vec![button(center, false)],
            None,
            false,
            0.2,
        );
        assert!(actions.contains(&SceneAction::Select(Some(7))));
        let empty = state.rect.unwrap().left_bottom() + Vec2::new(20., -20.);
        frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(empty), button(empty, true)],
            Some(7),
            false,
            0.3,
        );
        let actions = frame(
            &ctx,
            &mut state,
            vec![button(empty, false)],
            Some(7),
            false,
            0.4,
        );
        assert!(actions.contains(&SceneAction::Select(None)));
    }
    #[test]
    fn handle_drag_begins_updates_cancels_with_escape_and_release_commits_once() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], Some(7), false, 0.);
        let geometry = GizmoGeometry::build(
            &objects(),
            7,
            state.tool,
            state.space,
            state.camera,
            state.rect.unwrap(),
        )
        .unwrap();
        let handle = geometry
            .axes
            .iter()
            .find(|axis| axis.axis == Axis::X)
            .unwrap()
            .end;
        let start = frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(handle), button(handle, true)],
            Some(7),
            false,
            0.1,
        );
        assert!(start.contains(&SceneAction::BeginGesture { id: 7 }));
        let updates = frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(handle + Vec2::new(20., 0.))],
            Some(7),
            false,
            0.2,
        );
        assert!(updates.iter().any(|action|matches!(action,SceneAction::SetTransform{id:7,transform} if transform.translation != [0.;3])));
        let canceled = frame(
            &ctx,
            &mut state,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            Some(7),
            false,
            0.3,
        );
        assert!(canceled.contains(&SceneAction::CancelGesture {
            id: 7,
            transform: Transform::default()
        }));
        assert!(state.gesture.is_none());
        frame(
            &ctx,
            &mut state,
            vec![button(handle, false)],
            Some(7),
            false,
            0.4,
        );
        frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(handle), button(handle, true)],
            Some(7),
            false,
            0.5,
        );
        let released = frame(
            &ctx,
            &mut state,
            vec![
                egui::Event::PointerMoved(handle + Vec2::new(25., 0.)),
                button(handle + Vec2::new(25., 0.), false),
            ],
            Some(7),
            false,
            0.6,
        );
        assert_eq!(
            released
                .iter()
                .filter(|action| matches!(action, SceneAction::CommitGesture))
                .count(),
            1
        );
        assert!(state.gesture.is_none());
        assert!(
            !frame(&ctx, &mut state, vec![], Some(7), false, 0.7)
                .contains(&SceneAction::CommitGesture)
        );
    }
    #[derive(Clone, Copy)]
    enum DragInputBatching {
        SplitFrames,
        PressAndMove,
        CompleteGesture,
        PressAway,
        ReleaseBeforePress,
        MoveAfterRelease,
        MoveOutsideViewport,
    }
    fn assert_handle_drag_event_batching(batching: DragInputBatching) {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], Some(7), false, 0.);
        let geometry = GizmoGeometry::build(
            &objects(),
            7,
            state.tool,
            state.space,
            state.camera,
            state.rect.unwrap(),
        )
        .unwrap();
        let handle = geometry
            .axes
            .iter()
            .find(|axis| axis.axis == Axis::X)
            .unwrap()
            .end;
        let mut end = handle + (handle - geometry.center).normalized() * 40.;
        if matches!(batching, DragInputBatching::MoveOutsideViewport) {
            end.x = state.rect.unwrap().right() + 2.;
            assert!(input(0., vec![]).screen_rect.unwrap().contains(end));
            assert!(!state.rect.unwrap().contains(end));
        } else {
            assert!(state.rect.unwrap().contains(end));
        }
        assert_eq!(geometry.hit_test(handle), Some(Axis::X));
        assert_eq!(geometry.hit_test(end), None);

        let mut events = vec![egui::Event::PointerMoved(handle), button(handle, true)];
        if matches!(batching, DragInputBatching::ReleaseBeforePress) {
            let preceding = frame(
                &ctx,
                &mut state,
                vec![egui::Event::PointerMoved(end), button(end, true)],
                Some(7),
                false,
                0.05,
            );
            assert!(!preceding.contains(&SceneAction::BeginGesture { id: 7 }));
            events.insert(0, button(end, false));
        }
        if matches!(batching, DragInputBatching::PressAway) {
            let actions = frame(
                &ctx,
                &mut state,
                vec![
                    egui::Event::PointerMoved(end),
                    button(end, true),
                    egui::Event::PointerMoved(handle),
                ],
                Some(7),
                false,
                0.1,
            );
            assert!(!actions.iter().any(|action| matches!(
                action,
                SceneAction::BeginGesture { .. } | SceneAction::SetTransform { .. }
            )));
            assert!(state.gesture.is_none());
            let released = frame(
                &ctx,
                &mut state,
                vec![button(handle, false)],
                Some(7),
                false,
                0.2,
            );
            assert!(!released.contains(&SceneAction::CommitGesture));
            return;
        }
        let mut actions = vec![];
        if matches!(batching, DragInputBatching::SplitFrames) {
            actions.extend(frame(&ctx, &mut state, events, Some(7), false, 0.1));
            events = vec![];
        }
        events.push(egui::Event::PointerMoved(end));
        let released_in_frame = matches!(
            batching,
            DragInputBatching::CompleteGesture | DragInputBatching::MoveAfterRelease
        );
        if released_in_frame {
            events.push(button(end, false));
        }
        if matches!(batching, DragInputBatching::MoveAfterRelease) {
            events.push(egui::Event::PointerMoved(handle));
        }
        actions.extend(frame(&ctx, &mut state, events, Some(7), false, 0.2));
        assert_eq!(
            actions
                .iter()
                .filter(|action| matches!(action, SceneAction::BeginGesture { id: 7 }))
                .count(),
            1,
            "The press position must start the gesture even when movement shares its input frame"
        );
        assert!(
            actions.iter().any(|action| matches!(action,
                SceneAction::SetTransform { id: 7, transform }
                    if transform.translation != Transform::default().translation
            )),
            "The gesture must preserve displacement from the actual press position"
        );
        if !released_in_frame {
            assert!(state.gesture.is_some());
            assert!(!actions.contains(&SceneAction::CommitGesture));
            actions.extend(frame(
                &ctx,
                &mut state,
                vec![button(end, false)],
                Some(7),
                false,
                0.3,
            ));
        }
        assert_eq!(
            actions
                .iter()
                .filter(|action| matches!(action, SceneAction::CommitGesture))
                .count(),
            1
        );
        assert!(state.gesture.is_none());
        assert!(
            !frame(&ctx, &mut state, vec![], Some(7), false, 0.4)
                .contains(&SceneAction::CommitGesture)
        );
    }
    #[test]
    fn handle_drag_event_batching_split_frames_positive_control() {
        assert_handle_drag_event_batching(DragInputBatching::SplitFrames);
    }
    #[test]
    fn handle_drag_event_batching_coalesced_press_move_preserves_origin() {
        assert_handle_drag_event_batching(DragInputBatching::PressAndMove);
    }
    #[test]
    fn handle_drag_event_batching_complete_gesture_preserves_origin_and_commit() {
        assert_handle_drag_event_batching(DragInputBatching::CompleteGesture);
    }
    #[test]
    fn handle_drag_event_batching_press_away_does_not_capture_handle() {
        assert_handle_drag_event_batching(DragInputBatching::PressAway);
    }
    #[test]
    fn handle_drag_event_batching_release_before_press_does_not_commit_new_gesture() {
        assert_handle_drag_event_batching(DragInputBatching::ReleaseBeforePress);
    }
    #[test]
    fn handle_drag_event_batching_move_after_release_preserves_completed_displacement() {
        assert_handle_drag_event_batching(DragInputBatching::MoveAfterRelease);
    }
    #[test]
    fn handle_drag_event_batching_move_outside_viewport_preserves_origin_ownership() {
        assert_handle_drag_event_batching(DragInputBatching::MoveOutsideViewport);
    }
    fn assert_overlay_press_does_not_start_scene_gesture(overlay: SceneOverlay) {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], Some(7), false, 0.);
        let geometry = GizmoGeometry::build(
            &objects(),
            7,
            state.tool,
            state.space,
            state.camera,
            state.rect.unwrap(),
        )
        .unwrap();
        let handle = geometry
            .axes
            .iter()
            .find(|axis| axis.axis == Axis::X)
            .unwrap()
            .end;
        let end = handle + (handle - geometry.center).normalized() * 40.;
        let overlay_rect = Rect::from_center_size(handle, Vec2::splat(24.));
        assert_eq!(geometry.hit_test(handle), Some(Axis::X));
        assert!(state.rect.unwrap().contains(end));
        assert!(!overlay_rect.contains(end));
        for time in [0.05, 0.1] {
            frame_with_overlay(
                &ctx,
                &mut state,
                vec![],
                Some(7),
                false,
                time,
                Some((overlay, overlay_rect)),
            );
        }
        match overlay {
            SceneOverlay::Foreground => {
                assert_eq!(
                    ctx.layer_id_at(handle).unwrap().order,
                    egui::Order::Foreground
                );
                assert_eq!(ctx.layer_id_at(end).unwrap().order, egui::Order::Background);
            }
            SceneOverlay::SameLayer => {
                let response = ctx
                    .read_response(egui::Id::new("scene-press-overlay"))
                    .unwrap();
                assert!(response.enabled());
                assert!(response.sense.interactive());
                assert!(response.interact_rect.contains(handle));
                assert!(!response.interact_rect.contains(end));
                assert_eq!(ctx.layer_id_at(handle), Some(response.layer_id));
            }
        }
        let actions = frame_with_overlay(
            &ctx,
            &mut state,
            vec![
                egui::Event::PointerMoved(handle),
                button(handle, true),
                egui::Event::PointerMoved(end),
            ],
            Some(7),
            false,
            0.2,
            Some((overlay, overlay_rect)),
        );
        assert!(!actions.iter().any(|action| matches!(
            action,
            SceneAction::BeginGesture { .. }
                | SceneAction::SetTransform { .. }
                | SceneAction::CommitGesture
        )));
        assert!(state.gesture.is_none());
        let released = frame_with_overlay(
            &ctx,
            &mut state,
            vec![button(end, false)],
            Some(7),
            false,
            0.3,
            Some((overlay, overlay_rect)),
        );
        assert!(!released.contains(&SceneAction::CommitGesture));
    }
    #[test]
    fn handle_drag_event_batching_foreground_overlay_owns_press_origin() {
        assert_overlay_press_does_not_start_scene_gesture(SceneOverlay::Foreground);
    }
    #[test]
    fn handle_drag_event_batching_same_layer_overlay_owns_press_origin() {
        assert_overlay_press_does_not_start_scene_gesture(SceneOverlay::SameLayer);
    }
    #[test]
    fn play_lock_and_lost_window_focus_restore_gesture_baseline() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], Some(7), false, 0.);
        let start = state.rect.unwrap().center();
        state.gesture = Some(
            DragGesture::begin(
                &objects(),
                7,
                Tool::Translate,
                Space::World,
                Axis::X,
                state.camera,
                state.rect.unwrap(),
                start,
            )
            .unwrap(),
        );
        let actions = frame(&ctx, &mut state, vec![], Some(7), true, 0.1);
        assert!(actions.contains(&SceneAction::CancelGesture {
            id: 7,
            transform: Transform::default()
        }));
        let geometry = GizmoGeometry::build(
            &objects(),
            7,
            state.tool,
            state.space,
            state.camera,
            state.rect.unwrap(),
        )
        .unwrap();
        let handle = geometry.axes[0].end;
        let actions = frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(handle), button(handle, true)],
            Some(7),
            true,
            0.2,
        );
        assert!(!actions.iter().any(|action| matches!(
            action,
            SceneAction::BeginGesture { .. } | SceneAction::SetTransform { .. }
        )));
        state.gesture = Some(
            DragGesture::begin(
                &objects(),
                7,
                Tool::Translate,
                Space::World,
                Axis::X,
                state.camera,
                state.rect.unwrap(),
                start,
            )
            .unwrap(),
        );
        let mut lost_focus = input(0.3, vec![]);
        lost_focus.focused = false;
        let mut actions = vec![];
        let output = ctx.run_ui(lost_focus, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                actions = state.interact(ui, &objects(), &snapshot(), Some(7), false);
            });
        });
        output.drop_without_applying_deltas();
        assert!(actions.contains(&SceneAction::CancelGesture {
            id: 7,
            transform: Transform::default()
        }));
    }
    #[test]
    fn shortcuts_require_scene_focus_and_text_edit_keeps_its_keyboard() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        let key = || egui::Event::Key {
            key: egui::Key::R,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, &mut state, vec![key()], None, false, 0.);
        assert_eq!(state.tool, Tool::Translate);
        let center = state.rect.unwrap().center();
        frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(center), button(center, true)],
            None,
            false,
            0.1,
        );
        frame(
            &ctx,
            &mut state,
            vec![button(center, false)],
            None,
            false,
            0.2,
        );
        frame(&ctx, &mut state, vec![key()], None, false, 0.3);
        assert_eq!(state.tool, Tool::Rotate);
        let mut text = String::new();
        let output = ctx.run_ui(input(0.4, vec![]), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.text_edit_singleline(&mut text).request_focus();
                state.interact(ui, &objects(), &snapshot(), None, false);
            });
        });
        output.drop_without_applying_deltas();
        state.tool = Tool::Translate;
        let output = ctx.run_ui(
            input(0.5, vec![key(), egui::Event::Text("r".into())]),
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.text_edit_singleline(&mut text);
                    state.interact(ui, &objects(), &snapshot(), None, false);
                });
            },
        );
        output.drop_without_applying_deltas();
        assert_eq!(text, "r");
        assert_eq!(state.tool, Tool::Translate);
    }
    #[test]
    fn typed_resource_drop_returns_id_reference_and_play_blocks_authoring() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], None, false, 0.);
        let center = state.rect.unwrap().center();
        let payload = |kind| AssetDrag {
            id: "same-file".into(),
            reference: "asset://same-file".into(),
            path: "assets/example.glb".into(),
            kind,
        };
        for (index, kind) in [AssetKind::Model, AssetKind::Texture]
            .into_iter()
            .enumerate()
        {
            let time = 0.1 + index as f64 * 0.3;
            frame(
                &ctx,
                &mut state,
                vec![egui::Event::PointerMoved(center), button(center, true)],
                None,
                false,
                time,
            );
            egui::DragAndDrop::set_payload(&ctx, payload(kind));
            let actions = frame(
                &ctx,
                &mut state,
                vec![button(center, false)],
                None,
                false,
                time + 0.1,
            );
            assert!(
                !actions
                    .iter()
                    .any(|action| matches!(action, SceneAction::Select(_)))
            );
            if kind == AssetKind::Model {
                assert!(actions.iter().any(|action|matches!(action,SceneAction::DropModel{asset_ref,..} if asset_ref=="asset://same-file")));
            } else {
                assert!(actions.contains(&SceneAction::DropTexture {
                    asset_ref: "asset://same-file".into(),
                    id: 7
                }));
            }
        }
        frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(center), button(center, true)],
            None,
            true,
            0.8,
        );
        egui::DragAndDrop::set_payload(&ctx, payload(AssetKind::Model));
        let actions = frame(
            &ctx,
            &mut state,
            vec![button(center, false)],
            None,
            true,
            0.9,
        );
        assert!(!actions.iter().any(|action| matches!(
            action,
            SceneAction::DropModel { .. } | SceneAction::DropTexture { .. }
        )));
    }
    #[test]
    fn static_scene_reuses_pixels_but_camera_size_revision_and_idle_poll_refresh() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState {
            rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(160., 90.))),
            ..Default::default()
        };
        state.set_render_revision(1);
        let authored = objects();
        let snapshot = snapshot();
        let draw = |state: &mut SceneViewportState, time, selected, fail| {
            ctx.run_ui(input(time, vec![]), |ui| {
                state.paint(
                    ui,
                    &authored,
                    &snapshot,
                    selected,
                    &mut |_camera, width, height| {
                        if fail {
                            return Err("expected render failure".into());
                        }
                        Ok(Frame {
                            width,
                            height,
                            rgba: vec![128; width * height * 4],
                            gpu: None,
                            timings: None,
                            stats: RenderStats::default(),
                        })
                    },
                );
            })
            .drop_without_applying_deltas();
        };
        draw(&mut state, 0., None, false);
        let texture = state.texture.as_ref().unwrap().id();
        assert_eq!(state.render_count(), 1);
        for frame in 1..=5 {
            // Selection/gizmo overlays must not trigger a 3D render either.
            draw(&mut state, frame as f64 * 0.01, Some(7), true);
        }
        assert_eq!(state.render_count(), 1);
        assert!(state.error.is_none());
        state.camera.yaw += 0.1;
        draw(&mut state, 0.06, None, false);
        assert_eq!(state.render_count(), 2);
        state.set_render_revision(2);
        draw(&mut state, 0.07, None, false);
        assert_eq!(state.render_count(), 3);
        state.rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(180., 90.)));
        draw(&mut state, 0.08, None, false);
        assert_eq!(state.render_count(), 4);
        draw(&mut state, 0.34, None, false);
        assert_eq!(
            state.render_count(),
            5,
            "Idle polling still observes external HLSL changes"
        );
        state.set_render_revision(3);
        draw(&mut state, 0.35, None, true);
        assert_eq!(state.render_count(), 6);
        assert_eq!(state.texture.as_ref().unwrap().id(), texture);
        assert_eq!(state.error.as_deref(), Some("expected render failure"));
        draw(&mut state, 0.36, None, true);
        assert_eq!(
            state.render_count(),
            6,
            "A failed refresh must not retry for every pan event"
        );
        state.set_render_revision(4);
        draw(&mut state, 0.37, None, false);
        assert_eq!(state.render_count(), 7);
        assert!(state.error.is_none());
        assert_eq!(state.texture.as_ref().unwrap().id(), texture);
    }

    #[test]
    fn saturated_presentation_keeps_visible_image_camera_and_retries_without_revision_change() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState {
            rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(160., 90.))),
            ..Default::default()
        };
        let objects = objects();
        let snapshot = snapshot();
        let draw = |state: &mut SceneViewportState, time: f64, busy: bool| {
            ctx.run_ui(input(time, vec![]), |ui| {
                state.paint_presented(
                    ui,
                    &objects,
                    &snapshot,
                    None,
                    &mut |camera, width, height| {
                        Ok((!busy).then(|| PresentedSceneFrame {
                            frame: Frame {
                                width,
                                height,
                                rgba: vec![128; width * height * 4],
                                gpu: None,
                                timings: None,
                                stats: RenderStats::default(),
                            },
                            camera,
                            revision: Some(1),
                            pending: false,
                        }))
                    },
                );
            })
        };
        state.set_render_revision(1);
        draw(&mut state, 0., false).drop_without_applying_deltas();
        let original_camera = state.visible_camera();
        let texture = state.texture.as_ref().unwrap().id();
        state.camera.yaw += 0.5;
        let requested_camera = state.camera;
        let output = draw(&mut state, 0.01, true);
        assert_eq!(state.visible_camera(), original_camera);
        assert_eq!(state.texture.as_ref().unwrap().id(), texture);
        assert!(state.error.is_none());
        let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        assert!(primitives.iter().any(|shape| matches!(&shape.primitive,
            egui::epaint::Primitive::Mesh(mesh) if mesh.texture_id == texture)));
        output.drop_without_applying_deltas();
        let attempts = state.render_count();
        // Same authored revision/camera, well before the 250 ms idle refresh.
        draw(&mut state, 0.02, false).drop_without_applying_deltas();
        assert_eq!(state.render_count(), attempts + 1);
        assert_eq!(state.visible_camera(), requested_camera);
    }

    #[test]
    fn delayed_completion_keeps_its_camera_and_key_and_polls_successor_before_idle_refresh() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState {
            rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(160., 90.))),
            ..Default::default()
        };
        let authored = objects();
        let snapshot = snapshot();
        let first = state.camera;
        let second = Camera {
            yaw: first.yaw + 0.3,
            ..first
        };
        let third = Camera {
            yaw: first.yaw + 0.6,
            ..first
        };
        let draw = |state: &mut SceneViewportState, time, completed: Option<Camera>, pending| {
            ctx.run_ui(input(time, vec![]), |ui| {
                state.paint_presented(
                    ui,
                    &authored,
                    &snapshot,
                    None,
                    &mut |_requested, width, height| {
                        Ok(completed.map(|camera| PresentedSceneFrame {
                            frame: Frame {
                                width,
                                height,
                                rgba: vec![128; width * height * 4],
                                gpu: None,
                                timings: None,
                                stats: RenderStats::default(),
                            },
                            camera,
                            revision: Some(1),
                            pending,
                        }))
                    },
                );
            })
            .drop_without_applying_deltas();
        };
        state.set_render_revision(1);
        draw(&mut state, 0., Some(first), false);
        state.camera = second;
        draw(&mut state, 0.01, None, true);
        assert_eq!(state.visible_camera(), first);
        state.camera = third;
        draw(&mut state, 0.02, Some(second), true);
        assert_eq!(state.visible_camera(), second);
        assert_eq!(state.rendered_key.unwrap().camera, second);
        assert!(state.presentation_pending);
        let attempts = state.render_count();
        draw(&mut state, 0.03, Some(third), false);
        assert_eq!(state.render_count(), attempts + 1);
        assert_eq!(state.visible_camera(), third);
        assert!(!state.presentation_pending);
        draw(&mut state, 0.04, None, true);
        assert_eq!(
            state.render_count(),
            attempts + 1,
            "A completed unchanged view returns to its static cache"
        );
    }

    #[test]
    fn retained_old_revision_or_size_cannot_pick_new_geometry() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], None, false, 0.);
        state.set_render_revision(2);
        let current_size = render_size(state.rect.unwrap(), ctx.pixels_per_point());
        let center =
            scene_tools::project_point(state.camera, state.rect.unwrap(), [0.; 3]).unwrap();
        let click = |state: &mut SceneViewportState, time| {
            frame(
                &ctx,
                state,
                vec![egui::Event::PointerMoved(center), button(center, true)],
                None,
                false,
                time,
            );
            frame(
                &ctx,
                state,
                vec![button(center, false)],
                None,
                false,
                time + 0.01,
            )
        };
        state.displayed_key = Some(SceneRenderKey {
            revision: Some(1),
            camera: state.camera,
            size: current_size,
        });
        let actions = click(&mut state, 0.1);
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, SceneAction::Select(_)))
        );
        state.displayed_key = Some(SceneRenderKey {
            revision: Some(2),
            camera: state.camera,
            size: [1, 1],
        });
        let actions = click(&mut state, 0.2);
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, SceneAction::Select(_)))
        );
        state.displayed_key = Some(SceneRenderKey {
            revision: Some(2),
            camera: state.camera,
            size: current_size,
        });
        assert!(click(&mut state, 0.3).contains(&SceneAction::Select(Some(7))));
    }

    #[test]
    fn asset_drop_release_uses_the_visible_camera_while_a_new_camera_is_pending() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], None, false, 0.);
        let visible = state.camera;
        state.displayed_camera = Some(visible);
        state.camera.target = [100., 0., 0.];
        let center = scene_tools::project_point(visible, state.rect.unwrap(), [0.; 3]).unwrap();
        assert!(
            scene_tools::camera_ray(state.camera, state.rect.unwrap(), center)
                .and_then(|ray| scene_tools::pick(&snapshot(), ray))
                .is_none()
        );
        frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(center), button(center, true)],
            None,
            false,
            0.1,
        );
        egui::DragAndDrop::set_payload(
            &ctx,
            AssetDrag {
                id: "visible-texture".into(),
                reference: "asset://visible-texture".into(),
                path: "assets/visible.png".into(),
                kind: AssetKind::Texture,
            },
        );
        let actions = frame(
            &ctx,
            &mut state,
            vec![button(center, false)],
            None,
            false,
            0.2,
        );
        assert!(actions.contains(&SceneAction::DropTexture {
            asset_ref: "asset://visible-texture".into(),
            id: 7,
        }));
    }

    #[test]
    fn painter_preserves_aspect_and_uses_updated_host_snapshot_without_world_mutation() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        let authored = objects();
        let before = authored.clone();
        let mut requested = (0, 0);
        let output = ctx.run_ui(input(0., vec![]), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                state.interact(ui, &authored, &snapshot(), Some(7), false);
                state.paint(
                    ui,
                    &authored,
                    &snapshot(),
                    Some(7),
                    &mut |_camera, width, height| {
                        requested = (width, height);
                        Ok(Frame {
                            width,
                            height,
                            rgba: vec![255; width * height * 4],
                            gpu: None,
                            timings: None,
                            stats: RenderStats::default(),
                        })
                    },
                );
            });
        });
        output.drop_without_applying_deltas();
        let physical = state.rect.unwrap().size() * ctx.pixels_per_point();
        assert_eq!(
            (requested.0, requested.1),
            (physical.x.round() as usize, physical.y.round() as usize)
        );
        assert!(
            (requested.0 as f32 / requested.1 as f32 - state.rect.unwrap().aspect_ratio()).abs()
                < 0.01
        );
        assert_eq!(authored, before);
        assert!(state.texture.is_some());
    }
    #[test]
    fn real_transform_overlay_owns_clicks_and_keeps_selection_and_camera() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], Some(7), false, 0.);
        frame(&ctx, &mut state, vec![], Some(7), false, 0.1);
        let at = state.overlay_targets["Rotate"].center();
        let camera = state.camera;
        let press = frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(at), button(at, true)],
            Some(7),
            false,
            0.2,
        );
        let release = frame(
            &ctx,
            &mut state,
            vec![button(at, false)],
            Some(7),
            false,
            0.3,
        );
        assert_eq!(state.tool, Tool::Rotate);
        assert_eq!(state.camera, camera);
        assert!(press.is_empty() && release.is_empty());
        assert!(state.gesture.is_none());
    }
    #[test]
    fn compact_scene_keeps_a_tool_menu_and_disabled_parent_cannot_change_preferences() {
        for width in [70., 100., 160., 200., 300.] {
            let ctx = egui::Context::default();
            let mut state = SceneViewportState::default();
            for step in 0..3 {
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 240.))),
                        time: Some(step as f64),
                        ..Default::default()
                    },
                    |ui| {
                        state.interact(ui, &objects(), &snapshot(), Some(7), false);
                    },
                );
                output.drop_without_applying_deltas();
            }
            assert!(state.overlay_targets.contains_key("Compact"));
            assert_eq!(state.overlay_rects.len(), 1);
            assert!(state.rect.unwrap().contains_rect(state.overlay_rects[0]));
        }
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], Some(7), false, 0.);
        frame(&ctx, &mut state, vec![], Some(7), false, 0.1);
        let at = state.overlay_targets["Snap"].center();
        for (step, pressed) in [(0.2, true), (0.3, false)] {
            let output = ctx.run_ui(
                input(
                    step,
                    vec![egui::Event::PointerMoved(at), button(at, pressed)],
                ),
                |ui| {
                    ui.disable();
                    state.interact(ui, &objects(), &snapshot(), Some(7), false);
                },
            );
            output.drop_without_applying_deltas();
        }
        assert!(!state.snap.enabled);
    }
    #[test]
    fn camera_drag_retains_owner_when_crossing_the_real_tools_overlay() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], Some(7), false, 0.);
        frame(&ctx, &mut state, vec![], Some(7), false, 0.1);
        let start = state.rect.unwrap().center();
        let middle = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Middle,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(start), middle(start, true)],
            Some(7),
            false,
            0.2,
        );
        frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(start + Vec2::new(30., 10.))],
            Some(7),
            false,
            0.3,
        );
        let before = state.camera;
        let overlay = state.overlay_targets["Rotate"].center();
        frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(overlay)],
            Some(7),
            false,
            0.4,
        );
        assert_ne!(state.camera, before);
        frame(
            &ctx,
            &mut state,
            vec![middle(overlay, false)],
            Some(7),
            false,
            0.5,
        );
        assert!(!state.camera_drag);
    }

    fn right_button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn key_event(key: egui::Key, pressed: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn settled_scene() -> (egui::Context, SceneViewportState) {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        frame(&ctx, &mut state, vec![], Some(7), false, 0.);
        frame(&ctx, &mut state, vec![], Some(7), false, 0.01);
        (ctx, state)
    }

    #[test]
    fn right_mouse_flight_moves_looks_and_releases_without_authoring_commands() {
        let (ctx, mut state) = settled_scene();
        let rect = state.rect.unwrap();
        let at = rect.center();
        let initial = state.camera;
        // Keys alone cannot move an unfocused/idle Scene camera.
        assert!(
            frame(
                &ctx,
                &mut state,
                vec![key_event(egui::Key::W, true)],
                Some(7),
                false,
                0.02
            )
            .is_empty()
        );
        assert_eq!(state.camera, initial);
        let actions = frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(at), right_button(at, true)],
            Some(7),
            false,
            0.03,
        );
        assert!(state.camera_fly && actions.is_empty());
        let forward = scene_tools::camera_ray(initial, rect, at)
            .unwrap()
            .direction;
        let delta: [f32; 3] = std::array::from_fn(|i| state.camera.target[i] - initial.target[i]);
        assert!(delta.iter().zip(forward).map(|(a, b)| a * b).sum::<f32>() > 0.);
        let eye = scene_tools::camera_ray(state.camera, rect, at)
            .unwrap()
            .origin;
        assert!(
            frame(
                &ctx,
                &mut state,
                vec![
                    key_event(egui::Key::W, false),
                    egui::Event::PointerMoved(at + Vec2::new(50., -15.))
                ],
                Some(7),
                false,
                0.04
            )
            .is_empty()
        );
        let turned_eye = scene_tools::camera_ray(state.camera, rect, at)
            .unwrap()
            .origin;
        assert!(
            eye.into_iter()
                .zip(turned_eye)
                .all(|(a, b)| (a - b).abs() < 0.0001)
        );
        assert_ne!(state.camera.yaw, initial.yaw);
        // S is backwards in flight and never changes the transform tool.
        assert!(
            frame(
                &ctx,
                &mut state,
                vec![key_event(egui::Key::S, true), button(at, true)],
                Some(7),
                false,
                0.05
            )
            .is_empty()
        );
        assert_eq!(state.tool, Tool::Translate);
        assert!(state.gesture.is_none());
        assert!(
            frame(
                &ctx,
                &mut state,
                vec![button(at, false)],
                Some(7),
                false,
                0.06
            )
            .is_empty()
        );
        let before_release = state.camera;
        assert!(
            frame(
                &ctx,
                &mut state,
                vec![right_button(at, false)],
                Some(7),
                false,
                0.07
            )
            .is_empty()
        );
        assert!(!state.camera_fly);
        frame(&ctx, &mut state, vec![], Some(7), false, 0.08);
        assert_eq!(state.camera, before_release);
    }

    #[test]
    fn flight_cannot_start_from_an_overlay_or_another_widgets_press_origin() {
        for overlay in [
            None,
            Some(SceneOverlay::SameLayer),
            Some(SceneOverlay::Foreground),
        ] {
            let (ctx, mut state) = settled_scene();
            let rect = Rect::from_min_size(Pos2::new(300., 240.), Vec2::new(100., 100.));
            let extra = overlay.map(|kind| (kind, rect));
            frame_with_overlay(&ctx, &mut state, vec![], Some(7), false, 0.02, extra);
            frame_with_overlay(&ctx, &mut state, vec![], Some(7), false, 0.03, extra);
            let from = overlay.map_or(state.overlay_targets["Orientation"].center(), |_| {
                rect.center()
            });
            let to = state.rect.unwrap().center() + Vec2::new(150., 100.);
            let before = state.camera;
            let actions = frame_with_overlay(
                &ctx,
                &mut state,
                vec![
                    egui::Event::PointerMoved(from),
                    right_button(from, true),
                    egui::Event::PointerMoved(to),
                    key_event(egui::Key::W, true),
                ],
                Some(7),
                false,
                0.04,
                extra,
            );
            assert!(actions.is_empty() && !state.camera_fly);
            frame_with_overlay(&ctx, &mut state, vec![], Some(7), false, 0.05, extra);
            assert_eq!(state.camera, before);
        }
    }

    #[test]
    fn flight_keeps_its_canvas_owner_across_tools_and_stops_on_cancel_lifecycle() {
        for stop in [
            "escape",
            "transition",
            "hidden",
            "popup",
            "disabled",
            "focus",
        ] {
            let (ctx, mut state) = settled_scene();
            let at = state.rect.unwrap().center();
            frame(
                &ctx,
                &mut state,
                vec![egui::Event::PointerMoved(at), right_button(at, true)],
                Some(7),
                false,
                0.02,
            );
            assert!(state.camera_fly);
            let overlay = state.overlay_targets["Orientation"].center();
            let before = state.camera;
            frame(
                &ctx,
                &mut state,
                vec![egui::Event::PointerMoved(overlay)],
                Some(7),
                false,
                0.03,
            );
            assert!(state.camera_fly);
            assert_ne!(state.camera, before);
            match stop {
                "escape" => {
                    frame(
                        &ctx,
                        &mut state,
                        vec![key_event(egui::Key::Escape, true)],
                        Some(7),
                        false,
                        0.04,
                    );
                }
                "transition" => {
                    assert!(state.cancel_gesture().is_none());
                }
                "hidden" => {
                    let output = ctx.run_ui(input(0.04, vec![]), |_| {});
                    output.drop_without_applying_deltas();
                }
                "popup" => {
                    egui::Popup::open_id(&ctx, egui::Id::new("camera-blocking-menu"));
                }
                "disabled" | "focus" => {
                    let mut raw = input(0.04, vec![]);
                    raw.focused = stop != "focus";
                    let output = ctx.run_ui(raw, |ui| {
                        if stop == "disabled" {
                            ui.disable();
                        }
                        assert!(
                            state
                                .interact(ui, &objects(), &snapshot(), Some(7), false)
                                .is_empty()
                        );
                    });
                    output.drop_without_applying_deltas();
                }
                _ => unreachable!(),
            }
            let before = state.camera;
            frame(
                &ctx,
                &mut state,
                vec![egui::Event::PointerMoved(at), key_event(egui::Key::W, true)],
                Some(7),
                false,
                0.05,
            );
            assert!(!state.camera_fly, "stop={stop}");
            assert_eq!(state.camera, before, "stop={stop}");
        }
    }

    #[test]
    fn orientation_and_snap_are_separate_icon_controls_with_a_dedicated_steps_popup() {
        let (ctx, mut state) = settled_scene();
        let camera = state.camera;
        let steps = state.snap;
        let click = |state: &mut SceneViewportState, target: &str, time: f64| {
            let at = state.overlay_targets[target].center();
            assert!(
                frame(
                    &ctx,
                    state,
                    vec![egui::Event::PointerMoved(at), button(at, true)],
                    Some(7),
                    false,
                    time
                )
                .is_empty()
            );
            assert!(
                frame(
                    &ctx,
                    state,
                    vec![button(at, false)],
                    Some(7),
                    false,
                    time + 0.01
                )
                .is_empty()
            );
        };
        click(&mut state, "Orientation", 0.02);
        assert_eq!(state.space, Space::Local);
        assert_eq!(state.snap, steps);
        assert!(!egui::Popup::is_any_open(&ctx));
        click(&mut state, "Snap", 0.04);
        assert!(state.snap.enabled);
        assert_eq!(state.space, Space::Local);
        assert!(!egui::Popup::is_any_open(&ctx));
        click(&mut state, "SnapSettings", 0.06);
        frame(&ctx, &mut state, vec![], Some(7), false, 0.08);
        assert!(egui::Popup::is_any_open(&ctx));
        for name in ["MoveStep", "RotateStep", "ScaleStep"] {
            assert!(state.overlay_targets.contains_key(name));
        }
        assert_eq!(state.camera, camera);
        assert_eq!(state.snap.translation, steps.translation);
        let at = state.overlay_targets["MoveStep"].center();
        frame(
            &ctx,
            &mut state,
            vec![egui::Event::PointerMoved(at), button(at, true)],
            Some(7),
            false,
            0.081,
        );
        let moved = at + Vec2::new(20., 0.);
        assert!(
            frame(
                &ctx,
                &mut state,
                vec![egui::Event::PointerMoved(moved)],
                Some(7),
                false,
                0.082
            )
            .is_empty()
        );
        assert!(
            frame(
                &ctx,
                &mut state,
                vec![button(moved, false)],
                Some(7),
                false,
                0.083
            )
            .is_empty()
        );
        assert_ne!(state.snap.translation, steps.translation);
        assert_eq!(state.snap.rotation_degrees, steps.rotation_degrees);
        assert_eq!(state.snap.scale, steps.scale);
        assert_eq!(state.space, Space::Local);
        // The numeric popup owns RMB/WASD as well as pointer drags.
        let at = state.overlay_targets["MoveStep"].center();
        frame(
            &ctx,
            &mut state,
            vec![
                egui::Event::PointerMoved(at),
                right_button(at, true),
                key_event(egui::Key::W, true),
            ],
            Some(7),
            false,
            0.09,
        );
        assert!(!state.camera_fly);
        assert_eq!(state.camera, camera);
    }

    #[test]
    fn compact_menu_keeps_axes_and_snap_settings_separate_and_reachable() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        let mut step = 0;
        let mut draw = |state: &mut SceneViewportState, events| {
            let mut raw = input(step as f64 * 0.02, events);
            step += 1;
            raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(240., 360.)));
            let output = ctx.run_ui(raw, |ui| {
                assert!(
                    state
                        .interact(ui, &objects(), &snapshot(), Some(7), false)
                        .is_empty()
                );
            });
            output.drop_without_applying_deltas();
        };
        draw(&mut state, vec![]);
        draw(&mut state, vec![]);
        let at = state.overlay_targets["Compact"].center();
        draw(
            &mut state,
            vec![egui::Event::PointerMoved(at), button(at, true)],
        );
        draw(&mut state, vec![button(at, false)]);
        draw(&mut state, vec![]);
        let at = state.overlay_targets["Orientation"].center();
        draw(
            &mut state,
            vec![egui::Event::PointerMoved(at), button(at, true)],
        );
        draw(&mut state, vec![button(at, false)]);
        assert_eq!(state.space, Space::Local);
        assert!(!state.snap.enabled);
        if !egui::Popup::is_any_open(&ctx) {
            let at = state.overlay_targets["Compact"].center();
            draw(
                &mut state,
                vec![egui::Event::PointerMoved(at), button(at, true)],
            );
            draw(&mut state, vec![button(at, false)]);
        }
        draw(&mut state, vec![]);
        let at = state.overlay_targets["SnapSettings"].center();
        draw(
            &mut state,
            vec![egui::Event::PointerMoved(at), button(at, true)],
        );
        draw(&mut state, vec![button(at, false)]);
        draw(&mut state, vec![]);
        assert!(state.overlay_targets.contains_key("MoveStep"));
        assert!(state.overlay_targets.contains_key("RotateStep"));
        assert!(state.overlay_targets.contains_key("ScaleStep"));
    }

    #[test]
    fn text_field_takes_keys_from_flight_and_scene_can_reacquire_on_a_new_right_press() {
        let ctx = egui::Context::default();
        let mut state = SceneViewportState::default();
        let mut text = String::new();
        let mut step = 0;
        let mut draw = |state: &mut SceneViewportState, events, focus_text: bool| {
            let output = ctx.run_ui(input(step as f64 * 0.02, events), |ui| {
                let field = ui.text_edit_singleline(&mut text);
                if focus_text {
                    field.request_focus();
                }
                assert!(
                    state
                        .interact(ui, &objects(), &snapshot(), Some(7), false)
                        .is_empty()
                );
            });
            step += 1;
            output.drop_without_applying_deltas();
        };
        draw(&mut state, vec![], false);
        draw(&mut state, vec![], false);
        let at = state.rect.unwrap().center();
        draw(
            &mut state,
            vec![egui::Event::PointerMoved(at), right_button(at, true)],
            false,
        );
        assert!(state.camera_fly);
        let before = state.camera;
        draw(&mut state, vec![key_event(egui::Key::W, true)], true);
        assert!(!state.camera_fly);
        assert_eq!(state.camera, before);
        draw(&mut state, vec![right_button(at, false)], true);
        draw(&mut state, vec![right_button(at, true)], false);
        assert!(
            state.camera_fly,
            "a new canvas RMB press can acquire focus from a text field"
        );
    }

    #[test]
    fn separated_scene_groups_fit_narrow_panes_and_logical_dpi_scales() {
        for dpi in [1., 1.5, 2.] {
            for width in [100., 300., 320., 399., 400., 480., 1000.] {
                let ctx = egui::Context::default();
                ctx.set_pixels_per_point(dpi);
                let mut state = SceneViewportState::default();
                for step in 0..3 {
                    let mut raw = input(step as f64 * 0.02, vec![]);
                    raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 360.)));
                    let output = ctx.run_ui(raw, |ui| {
                        state.interact(ui, &objects(), &snapshot(), Some(7), false);
                    });
                    output.drop_without_applying_deltas();
                }
                let rect = state.rect.unwrap();
                for (index, a) in state.overlay_rects.iter().enumerate() {
                    assert!(
                        rect.contains_rect(*a),
                        "dpi={dpi} width={width} rect={rect:?} overlay={a:?}"
                    );
                    for b in &state.overlay_rects[index + 1..] {
                        assert!(!a.intersects(*b));
                    }
                }
                if rect.width() >= 400. {
                    let axes = state.overlay_targets["Orientation"];
                    let snap = state.overlay_targets["Snap"];
                    let steps = state.overlay_targets["SnapSettings"];
                    let framing = state.overlay_targets["Frame"];
                    assert!(snap.left() - axes.right() >= 16.);
                    assert!((framing.left() - steps.right() - 4.).abs() < 0.1);
                    assert!((axes.width() - 32.).abs() < 0.1);
                    for name in [
                        "Orientation",
                        "CameraSpeed",
                        "Snap",
                        "SnapSettings",
                        "Frame",
                    ] {
                        let control = state.overlay_targets[name];
                        assert!((control.height() - 32.).abs() < 0.1, "{name}: {control:?}");
                        assert!(
                            (control.top() - axes.top()).abs() < 0.1,
                            "dpi={dpi} width={width} {name}: {control:?}, axes={axes:?}"
                        );
                    }
                } else {
                    assert!(state.overlay_targets.contains_key("Compact"));
                }
            }
        }
    }
    struct OptionsHarness {
        ctx: egui::Context,
        view: SceneViewportState,
        width: f32,
        step: u32,
        locked: bool,
        disabled: bool,
    }

    #[test]
    fn compact_speed_field_fits_min_max_and_typed_value_at_native_font_size() {
        let mut harness = OptionsHarness::new(800.);
        crate::app::editor_ui::configure(&harness.ctx);
        for value in [scene_tools::MIN_CAMERA_SPEED, scene_tools::MAX_CAMERA_SPEED] {
            harness.view.camera_speed = value;
            harness.draw(vec![]);
            let field = harness.view.overlay_targets["CameraSpeed"];
            assert!(
                (field.width() - SCENE_SPEED_WIDTH).abs() < 0.1,
                "value={value} field={field:?}"
            );
            let output = harness.ctx.run_ui(input(1., vec![]), |ui| {
                harness
                    .view
                    .interact(ui, &objects(), &snapshot(), Some(7), false);
            });
            let field = harness.view.overlay_targets["CameraSpeed"];
            let text = output
                .shapes
                .iter()
                .filter_map(|s| {
                    if let egui::Shape::Text(t) = &s.shape {
                        Some(t)
                    } else {
                        None
                    }
                })
                .find(|t| t.galley.text().starts_with("Speed "))
                .expect("Speed label drawn");
            assert!(
                field.contains_rect(Rect::from_min_size(text.pos, text.galley.size())),
                "value={value} field={field:?} text={:?}",
                text.galley.size()
            );
            output.drop_without_applying_deltas();
        }
        harness.click("CameraSpeed");
        harness.draw(vec![egui::Event::Text("1000.0".into())]);
        harness.draw(vec![key_event(egui::Key::Enter, true)]);
        assert_eq!(harness.view.camera_speed, scene_tools::MAX_CAMERA_SPEED);
        assert!(
            (harness.view.overlay_targets["CameraSpeed"].width() - SCENE_SPEED_WIDTH).abs() < 0.1
        );
    }

    fn wheel(unit: egui::MouseWheelUnit, delta: f32) -> egui::Event {
        egui::Event::MouseWheel {
            unit,
            delta: Vec2::new(0., delta),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn flight_qe_is_vertical_and_owned_wheel_changes_speed_without_a_zoom_tail() {
        let mut harness = OptionsHarness::new(1000.);
        harness.view.camera_speed = 20.;
        let at = harness.view.rect.unwrap().center();
        let camera = harness.view.camera;
        harness.draw(vec![egui::Event::PointerMoved(at), right_button(at, true)]);
        harness.draw(vec![key_event(egui::Key::Q, true)]);
        let descended = harness.view.camera.target;
        assert!(descended[1] < camera.target[1]);
        assert_eq!(descended[0], camera.target[0]);
        assert_eq!(descended[2], camera.target[2]);
        harness.draw(vec![
            key_event(egui::Key::Q, false),
            key_event(egui::Key::E, true),
        ]);
        assert!((harness.view.camera.target[1] - camera.target[1]).abs() < 0.0001);
        harness.draw(vec![
            key_event(egui::Key::E, false),
            wheel(egui::MouseWheelUnit::Line, 1.),
        ]);
        assert_eq!(harness.view.camera_speed, 25.);
        assert_eq!(harness.view.camera.distance, camera.distance);
        harness.draw(vec![]);
        assert_eq!(harness.view.camera_speed, 25.);
        harness.draw(vec![wheel(egui::MouseWheelUnit::Point, -40.)]);
        assert_eq!(harness.view.camera_speed, 20.);
        let before_release = harness.view.camera;
        harness.draw(vec![right_button(at, false)]);
        for _ in 0..12 {
            harness.draw(vec![]);
        }
        assert_eq!(
            harness.view.camera, before_release,
            "no smoothed fly-scroll zooms after release"
        );
        harness.draw(vec![wheel(egui::MouseWheelUnit::Line, 1.)]);
        assert!(
            harness.view.camera.distance < camera.distance,
            "ordinary wheel still zooms"
        );
        assert_eq!(harness.view.camera_speed, 20.);
    }

    #[test]
    fn camera_speed_text_entry_orientation_cycle_and_play_preferences_are_independent() {
        for width in [1000., 240.] {
            let mut harness = OptionsHarness::new(width);
            let camera = harness.view.camera;
            let snap = harness.view.snap;
            for expected in [Space::Local, Space::World] {
                if width < 400. && !egui::Popup::is_any_open(&harness.ctx) {
                    harness.click("Compact");
                }
                harness.click("Orientation");
                assert_eq!(harness.view.space, expected);
                assert!(!harness.view.overlay_targets.contains_key("World"));
                assert!(!harness.view.overlay_targets.contains_key("Local"));
            }
            if width < 400. && !egui::Popup::is_any_open(&harness.ctx) {
                harness.click("Compact");
            }
            harness.click("CameraSpeed");
            harness.draw(vec![egui::Event::Text("32.5".into())]);
            harness.draw(vec![key_event(egui::Key::Enter, true)]);
            assert_eq!(harness.view.camera_speed, 32.5, "width={width}");
            assert_eq!(harness.view.camera, camera);
            assert_eq!(harness.view.snap, snap);
            assert_eq!(harness.view.space, Space::World);
            // Camera preferences are allowed during Play, unlike transform axes.
            harness.locked = true;
            harness.draw(vec![]);
            if width < 400. && !egui::Popup::is_any_open(&harness.ctx) {
                harness.click("Compact");
            }
            harness.click("CameraSpeed");
            harness.draw(vec![egui::Event::Text("24".into())]);
            harness.draw(vec![key_event(egui::Key::Enter, true)]);
            assert_eq!(harness.view.camera_speed, 24.);
        }
    }

    #[test]
    fn speed_wheel_limits_and_overlay_origin_keep_other_camera_state() {
        let mut harness = OptionsHarness::new(1000.);
        for name in ["Orientation", "CameraSpeed", "SnapSettings"] {
            let at = harness.view.overlay_targets[name].center();
            let before = harness.view.camera;
            let speed = harness.view.camera_speed;
            harness.draw(vec![
                egui::Event::PointerMoved(at),
                right_button(at, true),
                wheel(egui::MouseWheelUnit::Line, 1.),
            ]);
            assert!(!harness.view.camera_fly);
            assert_eq!(harness.view.camera, before);
            assert_eq!(harness.view.camera_speed, speed);
            harness.draw(vec![right_button(at, false)]);
        }
        let at = harness.view.rect.unwrap().center();
        harness.draw(vec![egui::Event::PointerMoved(at), right_button(at, true)]);
        harness.view.camera_speed = scene_tools::MAX_CAMERA_SPEED;
        harness.draw(vec![wheel(egui::MouseWheelUnit::Line, 10000.)]);
        assert_eq!(harness.view.camera_speed, scene_tools::MAX_CAMERA_SPEED);
        harness.view.camera_speed = scene_tools::MIN_CAMERA_SPEED;
        harness.draw(vec![wheel(egui::MouseWheelUnit::Line, -10000.)]);
        assert_eq!(harness.view.camera_speed, scene_tools::MIN_CAMERA_SPEED);
    }

    #[test]
    fn wheel_uses_rmb_ownership_at_its_event_not_the_final_button_state() {
        let mut observed = vec![];
        for case in 0..5 {
            let mut harness = OptionsHarness::new(1000.);
            let at = harness.view.rect.unwrap().center();
            harness.draw(vec![egui::Event::PointerMoved(at)]);
            if case < 2 {
                harness.draw(vec![right_button(at, true)]);
            }
            let events = match case {
                0 => vec![
                    wheel(egui::MouseWheelUnit::Line, 1.),
                    right_button(at, false),
                ],
                1 => vec![
                    right_button(at, false),
                    wheel(egui::MouseWheelUnit::Line, 1.),
                ],
                2 => vec![
                    wheel(egui::MouseWheelUnit::Line, 1.),
                    right_button(at, true),
                ],
                3 => vec![
                    right_button(at, true),
                    wheel(egui::MouseWheelUnit::Line, 1.),
                ],
                _ => vec![
                    right_button(at, true),
                    wheel(egui::MouseWheelUnit::Line, 1.),
                    right_button(at, false),
                ],
            };
            let distance = harness.view.camera.distance;
            harness.draw(events);
            observed.push((
                case,
                harness.view.camera_speed,
                harness.view.camera.distance < distance,
                harness.view.camera_fly,
            ));
        }
        assert_eq!(
            observed,
            vec![
                (0, 10., false, false),
                (1, 8., true, false),
                (2, 8., true, true),
                (3, 10., false, true),
                (4, 10., false, false)
            ]
        );
    }

    #[test]
    fn camera_wheel_keeps_configured_zoom_and_horizontal_modifiers_reserved() {
        for flying in [false, true] {
            for modifiers in [
                egui::Modifiers::CTRL,
                egui::Modifiers::COMMAND,
                egui::Modifiers::SHIFT,
                egui::Modifiers::ALT,
            ] {
                let mut harness = OptionsHarness::new(1000.);
                if modifiers.alt {
                    harness.ctx.options_mut(|options| {
                        options.input_options.zoom_modifier = egui::Modifiers::ALT
                    });
                }
                let at = harness.view.rect.unwrap().center();
                harness.draw(vec![egui::Event::PointerMoved(at)]);
                if flying {
                    harness.draw(vec![right_button(at, true)]);
                }
                let before = harness.view.camera;
                let speed = harness.view.camera_speed;
                let mut event = wheel(egui::MouseWheelUnit::Line, 1.);
                if let egui::Event::MouseWheel {
                    modifiers: current, ..
                } = &mut event
                {
                    *current = modifiers;
                }
                harness.draw(vec![event]);
                assert_eq!(
                    harness.view.camera, before,
                    "flying={flying}, modifiers={modifiers:?}"
                );
                assert_eq!(harness.view.camera_speed, speed);
            }
        }
    }

    #[test]
    fn speed_field_respects_disabled_parent_and_fly_wheel_is_applied_once_per_frame() {
        for width in [1000., 240.] {
            let mut harness = OptionsHarness::new(width);
            if width < 400. {
                harness.click("Compact");
            }
            harness.click("CameraSpeed");
            harness.disabled = true;
            harness.draw(vec![
                egui::Event::Text("100".into()),
                key_event(egui::Key::Enter, true),
            ]);
            assert_eq!(harness.view.camera_speed, scene_tools::DEFAULT_CAMERA_SPEED);
        }
        let mut harness = OptionsHarness::new(1000.);
        let at = harness.view.rect.unwrap().center();
        harness.draw(vec![egui::Event::PointerMoved(at), right_button(at, true)]);
        let mut passes = 0;
        let output = harness.ctx.run_ui(
            input(0.06, vec![wheel(egui::MouseWheelUnit::Line, 1.)]),
            |ui| {
                passes += 1;
                if passes == 1 {
                    ui.ctx().request_discard("repeat overlay layout");
                }
                assert!(
                    harness
                        .view
                        .interact(ui, &objects(), &snapshot(), Some(7), false)
                        .is_empty()
                );
            },
        );
        output.drop_without_applying_deltas();
        assert!(passes >= 2);
        assert_eq!(harness.view.camera_speed, 10.);
    }

    #[test]
    fn crossing_another_scene_keeps_the_held_flight_wheel_owner_in_both_draw_orders() {
        for (active_column, release) in [(0, false), (1, false), (0, true), (1, true)] {
            let ctx = egui::Context::default();
            let mut active = SceneViewportState::default();
            let mut other = SceneViewportState::default();
            let mut step = 0;
            let mut draw =
                |active: &mut SceneViewportState, other: &mut SceneViewportState, events| {
                    let output = ctx.run_ui(input(step as f64 * 0.02, events), |ui| {
                        ui.columns(2, |columns| {
                            for (index, column) in columns.iter_mut().enumerate() {
                                let view = if index == active_column {
                                    &mut *active
                                } else {
                                    &mut *other
                                };
                                assert!(
                                    view.interact(column, &objects(), &snapshot(), Some(7), false)
                                        .is_empty()
                                );
                            }
                        });
                    });
                    step += 1;
                    output.drop_without_applying_deltas();
                };
            draw(&mut active, &mut other, vec![]);
            draw(&mut active, &mut other, vec![]);
            let at = active.rect.unwrap().center();
            let destination = other.rect.unwrap().center();
            let other_camera = other.camera;
            draw(
                &mut active,
                &mut other,
                vec![egui::Event::PointerMoved(at), right_button(at, true)],
            );
            assert!(active.camera_fly);
            let mut events = vec![
                egui::Event::PointerMoved(destination),
                wheel(egui::MouseWheelUnit::Line, 1.),
            ];
            if release {
                events.push(right_button(destination, false));
            }
            draw(&mut active, &mut other, events);
            assert_eq!(active.camera_speed, 10.);
            assert_eq!(active.camera_fly, !release);
            assert!(!other.camera_fly);
            assert_eq!(other.camera_speed, scene_tools::DEFAULT_CAMERA_SPEED);
            assert_eq!(other.camera, other_camera, "draw order {active_column}");
        }
    }
    impl OptionsHarness {
        fn new(width: f32) -> Self {
            let mut harness = Self {
                ctx: egui::Context::default(),
                view: SceneViewportState::default(),
                width,
                step: 0,
                locked: false,
                disabled: false,
            };
            harness.draw(vec![]);
            harness.draw(vec![]);
            harness
        }
        fn draw(&mut self, events: Vec<egui::Event>) {
            let mut raw = input(self.step as f64 * 0.02, events);
            self.step += 1;
            raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(self.width, 500.)));
            let output = self.ctx.run_ui(raw, |ui| {
                if self.disabled {
                    ui.disable();
                }
                assert!(
                    self.view
                        .interact(ui, &objects(), &snapshot(), Some(7), self.locked)
                        .is_empty()
                );
            });
            output.drop_without_applying_deltas();
        }
        fn click(&mut self, target: &str) {
            let at = self.view.overlay_targets[target].center();
            self.draw(vec![egui::Event::PointerMoved(at), button(at, true)]);
            self.draw(vec![button(at, false)]);
            self.draw(vec![]);
        }
        fn open_steps(&mut self) {
            if self.width < 400. {
                self.click("Compact");
            }
            self.click("SnapSettings");
            assert!(self.view.overlay_targets.contains_key("MoveStep"));
        }
    }

    #[test]
    fn physics_overlay_paints_author_preview_lines_only_for_the_current_image() {
        let authored = objects();
        let mut world = EcsWorld::from_objects(authored.clone()).unwrap();
        world
            .insert_component(
                7,
                module_engine_physics_rapier::COLLIDER,
                serde_json::json!({}),
            )
            .unwrap();
        let mut view = SceneViewportState {
            rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500., 300.))),
            ..Default::default()
        };
        view.set_physics_available(true);
        view.physics.enabled = true;
        view.set_render_revision(1);
        view.refresh_physics(1, &world);
        let ctx = egui::Context::default();
        let draw = |view: &mut SceneViewportState, time, render| {
            let revision = view.render_revision;
            let output = ctx.run_ui(input(time, vec![]), |ui| {
                view.paint_presented(
                    ui,
                    &authored,
                    &snapshot(),
                    None,
                    &mut |camera, width, height| {
                        Ok(if render {
                            Some(PresentedSceneFrame {
                                frame: Frame {
                                    width,
                                    height,
                                    rgba: vec![128; width * height * 4],
                                    gpu: None,
                                    timings: None,
                                    stats: RenderStats::default(),
                                },
                                camera,
                                revision,
                                pending: false,
                            })
                        } else {
                            None
                        })
                    },
                );
            });
            let lines = output
                .shapes
                .iter()
                .filter(|shape| {
                    matches!(shape.shape,
                egui::Shape::LineSegment { stroke, .. }
                if stroke.color == Color32::from_rgb(102, 214, 196))
                })
                .count();
            output.drop_without_applying_deltas();
            lines
        };
        assert!(draw(&mut view, 0.1, true) > 0);
        view.set_render_revision(2);
        assert_eq!(draw(&mut view, 0.2, false), 0);
        view.refresh_physics(2, &world);
        assert!(draw(&mut view, 0.3, true) > 0);
        view.physics.enabled = false;
        view.refresh_physics(2, &world);
        assert_eq!(draw(&mut view, 0.4, true), 0);
    }

    #[test]
    fn physics_toggle_is_available_in_normal_and_compact_scene_without_picking() {
        for width in [800., 240.] {
            let mut harness = OptionsHarness::new(width);
            assert!(!harness.view.overlay_targets.contains_key("Physics"));
            harness.view.set_physics_available(true);
            harness.draw(vec![]);
            harness.draw(vec![]);
            if width < 400. {
                harness.click("Compact");
            }
            harness.click("Physics");
            assert!(harness.view.physics.enabled);
            harness.click("Physics");
            assert!(!harness.view.physics.enabled);
            harness.view.set_physics_available(false);
            harness.draw(vec![]);
            assert!(!harness.view.overlay_targets.contains_key("Physics"));
        }
    }

    #[test]
    fn open_snap_steps_respect_play_and_disabled_boundaries() {
        for width in [1000., 240.] {
            for disabled in [true, false] {
                let mut harness = OptionsHarness::new(width);
                harness.open_steps();
                let at = harness.view.overlay_targets["MoveStep"].center();
                let before = harness.view.snap;
                harness.disabled = disabled;
                harness.locked = !disabled;
                harness.draw(vec![]);
                harness.draw(vec![egui::Event::PointerMoved(at), button(at, true)]);
                let moved = at + Vec2::new(20., 0.);
                harness.draw(vec![egui::Event::PointerMoved(moved)]);
                harness.draw(vec![button(moved, false)]);
                assert_eq!(
                    harness.view.snap, before,
                    "width={width}, disabled={disabled}"
                );
            }
        }
    }

    #[test]
    fn snap_steps_accept_numeric_text_in_both_normal_and_compact_menus() {
        for width in [1000., 240.] {
            for (field, value) in [("MoveStep", 1.25), ("RotateStep", 30.), ("ScaleStep", 0.25)] {
                let mut harness = OptionsHarness::new(width);
                harness.open_steps();
                harness.click(field);
                harness.draw(vec![egui::Event::Text(value.to_string())]);
                harness.draw(vec![key_event(egui::Key::Enter, true)]);
                let actual = match field {
                    "MoveStep" => harness.view.snap.translation,
                    "RotateStep" => harness.view.snap.rotation_degrees,
                    _ => harness.view.snap.scale,
                };
                assert!(
                    (actual - value).abs() < 0.001,
                    "width={width} field={field}: expected {value}, actual {actual}"
                );
            }
        }
    }

    #[test]
    fn flight_applies_elapsed_time_once_when_egui_repeats_a_layout_pass() {
        let mut harness = OptionsHarness::new(1000.);
        let at = harness.view.rect.unwrap().center();
        harness.draw(vec![egui::Event::PointerMoved(at), right_button(at, true)]);
        assert!(harness.view.camera_fly);
        let before = harness.view.camera.target;
        let mut passes = 0;
        let mut expected_distance = 0.;
        let output = harness
            .ctx
            .run_ui(input(0.06, vec![key_event(egui::Key::W, true)]), |ui| {
                passes += 1;
                if passes == 1 {
                    expected_distance = ui.input(|i| i.stable_dt).min(0.1) * 8.;
                    ui.ctx().request_discard("exercise a second layout pass");
                }
                assert!(
                    harness
                        .view
                        .interact(ui, &objects(), &snapshot(), Some(7), false)
                        .is_empty()
                );
            });
        output.drop_without_applying_deltas();
        assert!(passes >= 2);
        let distance = harness
            .view
            .camera
            .target
            .into_iter()
            .zip(before)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt();
        assert!(
            (distance - expected_distance).abs() < 0.0001,
            "{passes} passes: expected {expected_distance}, actual {distance}"
        );
    }
}
