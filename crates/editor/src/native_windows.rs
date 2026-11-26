//! Native areas share Editor ownership; only the root advances its workflow.
use super::{
    dock::{DetachedArea, EditorDescriptor},
    *,
};

/// Native readback is persistence, not a new command to the OS. Keep the last
/// observed model separately from builder requests, whose patch is applied only
/// after the immediate viewport callback has read the previous native geometry.
#[derive(Default)]
pub(super) struct AreaGeometryTracker {
    last: Option<AreaGeometryInput>,
}
#[derive(PartialEq)]
struct AreaGeometryInput {
    position: Option<[f32; 2]>,
    size: [f32; 2],
    scale: f32,
    monitors: Vec<egui::Rect>,
}
#[derive(Clone, Copy, PartialEq)]
enum GeometryForce {
    Observe,
    Explicit,
    Recreate,
}
struct GeometryRequest {
    builder: egui::ViewportBuilder,
    commands: Vec<egui::ViewportCommand>,
}
impl AreaGeometryTracker {
    fn apply(
        &mut self,
        mut builder: egui::ViewportBuilder,
        area: &DetachedArea,
        monitors: &[egui::Rect],
        scale: f32,
        target: (egui::Pos2, egui::Vec2),
        force: GeometryForce,
    ) -> GeometryRequest {
        let input = AreaGeometryInput {
            position: area.position,
            size: area.size,
            scale,
            monitors: monitors.to_vec(),
        };
        let create = self.last.is_none() || force == GeometryForce::Recreate;
        let mut commands = Vec::new();
        if create || force == GeometryForce::Explicit || self.last.as_ref() != Some(&input) {
            if create {
                builder = builder.with_inner_size(target.1);
                if !monitors.is_empty() {
                    builder = builder.with_position(target.0);
                }
            } else {
                // A cached builder may already contain this target even when the
                // user has since resized/moved the native window. Explicit intent
                // therefore uses commands, once, after the child exists.
                commands.push(egui::ViewportCommand::InnerSize(target.1));
                if !monitors.is_empty() {
                    commands.push(egui::ViewportCommand::OuterPosition(target.0));
                }
            }
        }
        self.last = Some(input);
        GeometryRequest { builder, commands }
    }
    fn observed(&mut self, position: Option<[f32; 2]>, size: [f32; 2]) {
        if let Some(last) = &mut self.last {
            last.position = position;
            last.size = size;
        }
    }
}
fn dispatch_geometry(ctx: &egui::Context, commands: &mut Vec<egui::ViewportCommand>) {
    // A discarded immediate pass is rerun; its viewport commands are retained.
    for command in commands.drain(..) {
        ctx.send_viewport_cmd(command);
    }
}

/// Moves use the native outer origin but retain the entire client size.
/// With a stable OS title bar, using the inner origin would add its height on
/// every client-header drag. All inputs here are egui logical coordinates.
fn native_drag_geometry(
    client: egui::Rect,
    outer: Option<egui::Rect>,
    inner: Option<egui::Rect>,
) -> egui::Rect {
    let offset = outer
        .zip(inner)
        .map_or(egui::Vec2::ZERO, |(outer, inner)| outer.min - inner.min);
    egui::Rect::from_min_size(client.min + offset, client.size())
}

fn placement(
    area: &DetachedArea,
    work_areas: &[egui::Rect],
    scale: f32,
) -> (egui::Pos2, egui::Vec2) {
    let scale = scale.max(0.5);
    let desired = egui::Rect::from_min_size(
        area.position
            .map(egui::Pos2::from)
            .unwrap_or(egui::pos2(64., 64.)),
        egui::Vec2::from(area.size) * scale,
    );
    let fallback = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280., 720.));
    let work = work_areas
        .iter()
        .max_by(|a, b| {
            let coverage = |r: &egui::Rect| {
                let intersection = r.intersect(desired);
                if intersection.is_positive() {
                    intersection.area()
                } else {
                    0.
                }
            };
            coverage(a).total_cmp(&coverage(b))
        })
        .copied()
        .unwrap_or(fallback);
    // Leave room for native borders and title bar inside the OS work area.
    let bounds = work.shrink2(egui::vec2(8., 40.));
    let size = desired
        .size()
        .min(bounds.size())
        .max(egui::vec2(160., 120.));
    let pos = desired.min.clamp(bounds.min, bounds.max - size);
    (pos / scale, size / scale)
}

#[cfg(target_os = "windows")]
fn os_work_areas() -> Vec<egui::Rect> {
    use std::ffi::c_void;
    #[repr(C)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    struct MonitorInfo {
        size: u32,
        monitor: WinRect,
        work: WinRect,
        flags: u32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumDisplayMonitors(
            hdc: *mut c_void,
            clip: *const WinRect,
            callback: unsafe extern "system" fn(
                *mut c_void,
                *mut c_void,
                *mut WinRect,
                isize,
            ) -> i32,
            data: isize,
        ) -> i32;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
    }
    unsafe extern "system" fn collect(
        monitor: *mut c_void,
        _: *mut c_void,
        _: *mut WinRect,
        data: isize,
    ) -> i32 {
        let mut info = MonitorInfo {
            size: std::mem::size_of::<MonitorInfo>() as u32,
            monitor: WinRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            work: WinRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            flags: 0,
        };
        // EnumDisplayMonitors calls synchronously while this Vec is alive.
        if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            unsafe { &mut *(data as *mut Vec<egui::Rect>) }.push(egui::Rect::from_min_max(
                egui::pos2(info.work.left as f32, info.work.top as f32),
                egui::pos2(info.work.right as f32, info.work.bottom as f32),
            ));
        }
        1
    }
    let mut result = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            collect,
            &mut result as *mut Vec<egui::Rect> as isize,
        );
    }
    result
}
#[cfg(not(target_os = "windows"))]
fn os_work_areas() -> Vec<egui::Rect> {
    Vec::new()
}

impl Editor {
    pub(super) fn active_layout_mut(&mut self) -> &mut DockLayout {
        match self.workspace {
            WorkspaceKind::Engine => &mut self.engine_layout,
            WorkspaceKind::Project => &mut self.project_layout,
        }
    }
    pub(super) fn detach_editor_area(&mut self, id: u64, ctx: &egui::Context) {
        if ctx.embed_viewports() {
            self.status = "This host does not support separate OS windows".into();
            return;
        }
        self.cancel_active_scene_gestures();
        match self.active_layout_mut().detach(id) {
            Ok(()) => {
                self.personal_dirty = true;
                self.pending_area_focus = Some(id);
                self.status =
                    "Area moved to a separate window; drag to dock or close this area".into();
            }
            Err(error) => self.status = error,
        }
    }
    pub(super) fn clamp_main_window_once(&self, ctx: &egui::Context) {
        if ctx.embed_viewports() {
            return;
        }
        let key = egui::Id::new("main-work-area-clamped");
        if ctx.data(|d| d.get_temp::<bool>(key)).unwrap_or(false) {
            return;
        }
        let rect = ctx
            .input(|i| i.viewport().inner_rect)
            .unwrap_or_else(|| ctx.viewport_rect());
        let areas = os_work_areas();
        if areas.is_empty() {
            #[cfg(feature = "verification")]
            if let Some(geometry) = ctx.data(|d| {
                d.get_temp::<crate::ui_capture::CaptureGeometry>(egui::Id::new(
                    "diagnostic-geometry",
                ))
            }) {
                let size = egui::Vec2::from(geometry.size) / ctx.pixels_per_point();
                if (rect.size() - size).length() > 1. {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                    ctx.request_repaint();
                    return;
                }
            }
            ctx.data_mut(|d| d.insert_temp(key, true));
            return;
        }
        let current = DetachedArea {
            id: 0,
            editor: EditorType::Scene,
            tree: None,
            neighbor: 0,
            axis: dock::Axis::Horizontal,
            before: false,
            fallback: None,
            position: Some((rect.min * ctx.pixels_per_point()).into()),
            size: rect.size().into(),
        };
        #[cfg(feature = "verification")]
        let mut current = current;
        #[cfg(feature = "verification")]
        if let Some(geometry) = ctx.data(|d| {
            d.get_temp::<crate::ui_capture::CaptureGeometry>(egui::Id::new("diagnostic-geometry"))
        }) {
            current.size = geometry.size.map(|v| v / ctx.pixels_per_point());
        }
        let (position, size) = placement(&current, &areas, ctx.pixels_per_point());
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
        if (rect.size() - size).length() > 1. {
            ctx.request_repaint();
            return;
        }
        ctx.data_mut(|d| d.insert_temp(key, true));
    }
    pub(super) fn draw_detached_areas(
        &mut self,
        ctx: &egui::Context,
        available: &[EditorDescriptor],
    ) {
        let areas = self.active_layout_mut().detached.clone();
        let workspace = self.workspace_key();
        let focus = self.pending_area_focus.take();
        let monitors = os_work_areas();
        self.area_geometry
            .retain(|(key, id), _| key == &workspace && areas.iter().any(|area| area.id == *id));
        for area in areas {
            let geometry_key = (workspace.clone(), area.id);
            #[cfg(windows)]
            let before_windows = area_drag::native_pointer::own_windows();
            let dragging = self.area_drag.as_ref().is_some_and(|drag| {
                area.panes().iter().any(|(id, _)| *id == drag.id) && !drag.cancelled
            });
            let viewport = egui::ViewportId::from_hash_of(("editor-area", &workspace, area.id));
            let mut builder = egui::ViewportBuilder::default()
                .with_icon(crate::app_icon::icon())
                .with_title(format!(
                    "{} · Module Engine",
                    area.panes()
                        .first()
                        .map_or(area.editor, |(_, editor)| *editor)
                        .label()
                ))
                .with_min_inner_size([240., 200.])
                // `active` is creation-only in egui: changing it recreates the
                // HWND and loses the initiating pointer capture. Focus is an
                // explicit command after a completed move/open instead.
                .with_active(false)
                .with_decorations(true)
                .with_mouse_passthrough(dragging)
                .with_window_level(if dragging && self.area_drag_over_main() {
                    egui::WindowLevel::AlwaysOnBottom
                } else {
                    egui::WindowLevel::Normal
                })
                .with_clamp_size_to_monitor_size(true);
            let target_scale = ctx
                .input(|i| {
                    i.raw
                        .viewports
                        .get(&viewport)
                        .and_then(|v| v.native_pixels_per_point)
                })
                .map(|native| native * ctx.zoom_factor())
                .unwrap_or_else(|| ctx.pixels_per_point());
            let (position, size) = if dragging {
                (
                    egui::Pos2::from(area.position.unwrap_or([64., 64.])) / target_scale,
                    egui::Vec2::from(area.size),
                )
            } else {
                placement(&area, &monitors, target_scale)
            };
            #[cfg(windows)]
            let recreated = self
                .area_host_windows
                .get(&Some(geometry_key.clone()))
                .is_none_or(|window| !before_windows.contains(window));
            #[cfg(not(windows))]
            let recreated = ctx.input(|i| {
                i.raw
                    .viewports
                    .get(&viewport)
                    .is_none_or(|info| info.inner_rect.is_none())
            });
            let force = if recreated {
                GeometryForce::Recreate
            } else if dragging || self.pending_area_geometry.contains(&geometry_key) {
                GeometryForce::Explicit
            } else {
                GeometryForce::Observe
            };
            let request = self
                .area_geometry
                .entry(geometry_key.clone())
                .or_default()
                .apply(
                    builder,
                    &area,
                    &monitors,
                    target_scale,
                    (position, size),
                    force,
                );
            builder = request.builder;
            let mut geometry_commands = request.commands;
            let geometry_requested = builder.inner_size.is_some() || !geometry_commands.is_empty();
            let mut close = false;
            let mut return_to_main = false;
            let mut actual = None;
            ctx.show_viewport_immediate(viewport, builder, |ctx, class| {
                if class == egui::ViewportClass::EmbeddedWindow {
                    return_to_main = true;
                    self.status = "Separate OS windows are unavailable; area returned".into();
                    return;
                }
                #[cfg(windows)]
                {
                    let current = area_drag::native_pointer::own_windows();
                    let key = Some((workspace.clone(), area.id));
                    let previous = self.area_host_windows.get(&key).copied();
                    area_drag::bind_created_host(&mut self.area_host_windows, &workspace, area.id, &before_windows, &current);
                    if let Some(window) = self.area_host_windows.get(&key).copied()
                        && previous != Some(window)
                        && let Err(error) = crate::app_icon::apply_to_window(window) {
                        eprintln!("Editor icon: {error}");
                    }
                }
                close = ctx.input(|i| i.viewport().close_requested());
                actual = ctx.input(|i| {
                    i.viewport()
                        .outer_rect
                        .zip(i.viewport().inner_rect)
                        .map(|(outer, inner)| {
                            egui::Rect::from_min_size(
                                outer.min * ctx.pixels_per_point(),
                                inner.size(),
                            )
                        })
                });
                dispatch_geometry(ctx, &mut geometry_commands);
                if std::env::var_os("MODULE_ENGINE_AREA_GEOMETRY_TRACE").is_some() {
                    let key = egui::Id::new(("area-geometry-trace", area.id));
                    let sample = (actual, position, size, ctx.pixels_per_point(), geometry_requested);
                    let changed = ctx.data_mut(|data| {
                        let count_key = key.with("count");
                        let count = data.get_temp::<u32>(count_key).unwrap_or(0);
                        if count >= 2048 { return false; }
                        let previous = data.get_temp::<(Option<egui::Rect>, egui::Pos2, egui::Vec2, f32, bool)>(key);
                        data.insert_temp(key, sample);
                        let changed = previous != Some(sample);
                        if changed { data.insert_temp(count_key, count + 1); }
                        changed
                    });
                    if changed {
                        eprintln!("Area geometry {}: t={} actual={:?} desired_position={:?} desired_size={:?} ppp={} geometry_request={geometry_requested}", area.id, ctx.input(|i| i.time), actual, position, size, ctx.pixels_per_point());
                    }
                }
                if focus.is_some_and(|focus| area.panes().iter().any(|(id, _)| *id == focus)) && !dragging {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                self.workflow_shortcuts(ctx);
                let command_ctx = ctx.clone();
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(editor_ui::CANVAS).inner_margin(0))
                    .show(ctx, |ui| {
                        if self.area_drag.is_some() {
                            ui.disable();
                        }
                        if self.workflow.blocks_document_edits() || self.scene_source_ui.active() {
                            ui.label(
                                "Complete the current dialog or operation in the main window.",
                            );
                            if ui.button("Focus main window").clicked() {
                                command_ctx.send_viewport_cmd_to(
                                    egui::ViewportId::ROOT,
                                    egui::ViewportCommand::Focus,
                                );
                            }
                            return;
                        }
                        let active = self.active_window();
                        let response = self.active_layout_mut().draw_host(ui, available, active, Some(area.id));
                        self.personal_dirty |= response.changed;
                        if response.changed { self.cancel_active_scene_gestures(); }
                        let surviving = self.active_layout_mut().panes().into_iter().map(|(id, _)| id).collect::<std::collections::BTreeSet<_>>();
                        for (id, _) in area.panes() {
                            if !surviving.contains(&id) {
                                self.area_was_closed_during_drag(id, &command_ctx);
                                match self.workspace {
                                    WorkspaceKind::Engine => { self.engine_panes.remove(&id); }
                                    WorkspaceKind::Project => { self.project_panes.remove(&id); }
                                }
                            }
                        }
                        if let Some(id) = response.activated_pane { self.activate_area(id); }
                        let scale = command_ctx.pixels_per_point();
                        let origin = command_ctx.input(|i| i.viewport().inner_rect.map(|rect| rect.min)).unwrap_or_default() * scale;
                        // Register each actual leaf, so a drop into a floating host
                        // resolves to that tree rather than silently splitting main.
                        let host_ids = area.panes().into_iter().map(|(id, _)| id).collect::<std::collections::BTreeSet<_>>();
                        self.area_drop_targets.retain(|(id, _)| !host_ids.contains(id));
                        self.area_drop_targets.extend(response.panes.iter().filter(|pane| !dragging && surviving.contains(&pane.id)).map(|pane| (
                            pane.id,
                            egui::Rect::from_min_max(origin + pane.area_rect.min.to_vec2() * scale, origin + pane.area_rect.max.to_vec2() * scale),
                        )));
                        if let Some((id, rect, start)) = response.drag_pane {
                            let (outer, inner) = command_ctx.input(|i| (i.viewport().outer_rect, i.viewport().inner_rect));
                            let client = if area.panes().len() == 1 { command_ctx.viewport_rect() } else { rect };
                            let geometry = native_drag_geometry(client, outer, inner);
                            self.begin_area_drag(id, geometry, start, &command_ctx);
                            ui.disable();
                        }
                        let dividers = response.divider_input;
                        ui.push_id(&workspace, |ui| {
                            for pane in response.panes {
                                if surviving.contains(&pane.id) {
                                    self.draw_editor_area(ui, &pane, available, command_ctx.input(|i| i.time));
                                }
                            }
                        });
                        dividers.finish(ui);
                        for id in response.detach_panes { self.detach_editor_area(id, &command_ctx); }
                        self.draw_area_drop_preview(ui);
                    });
                ctx.request_repaint_after(std::time::Duration::from_millis(if self.playing {
                    16
                } else {
                    100
                }));
            });
            if self.allow_area_geometry_readback(&workspace, area.id)
                && let Some(rect) = actual
                && let Some(stored) = self
                    .active_layout_mut()
                    .detached
                    .iter_mut()
                    .find(|p| p.id == area.id)
            {
                let position = Some(rect.min.into());
                let size = rect.size().into();
                if stored.position != position || stored.size != size {
                    stored.position = position;
                    stored.size = size;
                    self.personal_dirty = true;
                }
                // Do not turn this observed geometry into a builder delta next frame.
                if let Some(tracker) = self.area_geometry.get_mut(&geometry_key) {
                    tracker.observed(position, size);
                }
            }
            if close {
                self.cancel_active_scene_gestures();
                self.area_host_was_closed_during_drag(area.id, ctx);
                let closed = self.active_layout_mut().close_host(area.id);
                self.area_host_windows
                    .remove(&Some((workspace.clone(), area.id)));
                if !closed.is_empty() {
                    for id in closed {
                        self.area_was_closed_during_drag(id, ctx);
                        match self.workspace {
                            WorkspaceKind::Engine => {
                                self.engine_panes.remove(&id);
                            }
                            WorkspaceKind::Project => {
                                self.project_panes.remove(&id);
                            }
                        }
                    }
                    self.personal_dirty = true;
                    self.status = "Window areas closed".into();
                }
            } else if return_to_main {
                self.cancel_active_scene_gestures();
                match self.active_layout_mut().return_area(area.id) {
                    Ok(()) => {
                        self.personal_dirty = true;
                        ctx.send_viewport_cmd_to(
                            egui::ViewportId::ROOT,
                            egui::ViewportCommand::Focus,
                        );
                    }
                    Err(error) => self.status = error,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn floating_scene() -> DetachedArea {
        let mut layout = DockLayout::single(EditorType::Scene);
        layout.detach(1).unwrap();
        layout.detached.remove(0)
    }
    fn geometry_delta(
        tracker: &mut AreaGeometryTracker,
        area: &DetachedArea,
        monitors: &[egui::Rect],
        scale: f32,
        force: GeometryForce,
    ) -> GeometryRequest {
        tracker.apply(
            egui::ViewportBuilder::default(),
            area,
            monitors,
            scale,
            placement(area, monitors, scale),
            force,
        )
    }
    #[test]
    fn observed_native_resize_never_becomes_an_os_geometry_command() {
        let monitors = [egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1920., 1040.),
        )];
        let mut area = floating_scene();
        let mut tracker = AreaGeometryTracker::default();
        let initial = geometry_delta(&mut tracker, &area, &monitors, 1., GeometryForce::Observe);
        let mut backend = initial.builder;
        assert!(backend.inner_size.is_some());
        assert!(initial.commands.is_empty());
        for width in [787., 789., 787., 789., 789., 789.] {
            area.size = [width, 764.];
            area.position = Some([797., 156.]);
            tracker.observed(area.position, area.size);
            let delta = geometry_delta(&mut tracker, &area, &monitors, 1., GeometryForce::Observe);
            assert!(delta.builder.inner_size.is_none() && delta.builder.position.is_none());
            assert!(delta.commands.is_empty());
            let (commands, recreate) = backend.patch(delta.builder);
            assert!(!recreate);
            assert!(commands.is_empty());
        }
        area.editor = EditorType::Assets;
        assert!(
            geometry_delta(&mut tracker, &area, &monitors, 1., GeometryForce::Observe)
                .commands
                .is_empty()
        );
    }
    #[test]
    fn explicit_original_target_is_dispatched_once_after_native_readback() {
        let monitors = [egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1920., 1040.),
        )];
        let mut area = floating_scene();
        area.position = Some([64., 64.]);
        area.size = [760., 510.];
        let mut tracker = AreaGeometryTracker::default();
        let mut backend =
            geometry_delta(&mut tracker, &area, &monitors, 1., GeometryForce::Observe).builder;
        // Native movement/resize changes the actual model, but not the builder's cached request.
        area.position = Some([797., 156.]);
        area.size = [800., 600.];
        tracker.observed(area.position, area.size);
        assert!(
            geometry_delta(&mut tracker, &area, &monitors, 1., GeometryForce::Observe)
                .commands
                .is_empty()
        );
        let old = (area.position, area.size);
        area.position = Some([64., 64.]);
        area.size = [760., 510.];
        let target = (area.position, area.size);
        let mut request =
            geometry_delta(&mut tracker, &area, &monitors, 1., GeometryForce::Observe);
        assert!(request.builder.position.is_none() && request.builder.inner_size.is_none());
        assert!(
            backend.patch(request.builder).0.is_empty(),
            "builder cannot implement this intent"
        );
        let ctx = egui::Context::default();
        let mut passes = 0;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            passes += 1;
            dispatch_geometry(ui.ctx(), &mut request.commands);
            if passes == 1 {
                ui.ctx()
                    .request_discard("geometry command once across discarded passes");
            }
        });
        assert!(passes >= 2);
        assert!(request.commands.is_empty());
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert_eq!(commands.iter().filter(|cmd|matches!(cmd,egui::ViewportCommand::InnerSize(size) if *size==egui::vec2(760.,510.))).count(),1);
        assert_eq!(commands.iter().filter(|cmd|matches!(cmd,egui::ViewportCommand::OuterPosition(pos) if *pos==egui::pos2(64.,64.))).count(),1);
        output.drop_without_applying_deltas();
        // Callback may read the pre-command native size once; it must not echo it.
        for (position, size) in [old, target, target] {
            area.position = position;
            area.size = size;
            tracker.observed(position, size);
            let next = geometry_delta(&mut tracker, &area, &monitors, 1., GeometryForce::Observe);
            assert!(next.commands.is_empty());
            assert!(next.builder.inner_size.is_none());
        }
        let forced = geometry_delta(&mut tracker, &area, &monitors, 1., GeometryForce::Explicit);
        assert!(forced.builder.inner_size.is_none());
        assert_eq!(forced.commands.len(), 2);
    }
    #[test]
    fn monitor_scale_and_native_recreation_keep_explicit_placement() {
        let first = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1920., 1040.));
        let second = egui::Rect::from_min_max(egui::pos2(1920., 0.), egui::pos2(4480., 1400.));
        let mut area = floating_scene();
        area.position = Some([2100., 160.]);
        area.size = [800., 600.];
        let mut tracker = AreaGeometryTracker::default();
        let initial = geometry_delta(
            &mut tracker,
            &area,
            &[first, second],
            1.,
            GeometryForce::Observe,
        );
        assert!(initial.builder.inner_size.is_some());
        assert!(initial.commands.is_empty());
        let scaled = geometry_delta(
            &mut tracker,
            &area,
            &[first, second],
            2.,
            GeometryForce::Observe,
        );
        assert!(scaled.commands.iter().any(|cmd|matches!(cmd,egui::ViewportCommand::OuterPosition(pos) if *pos==egui::pos2(1050.,80.))));
        let target = placement(&area, &[first], 2.);
        let moved = geometry_delta(&mut tracker, &area, &[first], 2., GeometryForce::Observe);
        assert!(first.contains_rect(egui::Rect::from_min_size(target.0 * 2., target.1 * 2.)));
        assert!(
            moved
                .commands
                .iter()
                .any(|cmd| matches!(cmd,egui::ViewportCommand::InnerSize(size) if *size==target.1))
        );
        let recreated = geometry_delta(&mut tracker, &area, &[first], 2., GeometryForce::Recreate);
        assert_eq!(recreated.builder.inner_size, Some(target.1));
        assert_eq!(recreated.builder.position, Some(target.0));
        assert!(recreated.commands.is_empty());
        let wayland = geometry_delta(&mut tracker, &area, &[], 2., GeometryForce::Recreate);
        assert!(wayland.builder.inner_size.is_some());
        assert!(wayland.builder.position.is_none());
        assert!(
            geometry_delta(&mut tracker, &area, &[], 2., GeometryForce::Explicit)
                .commands
                .iter()
                .all(|cmd| !matches!(cmd, egui::ViewportCommand::OuterPosition(_)))
        );
    }
    #[test]
    fn split_host_tearoff_uses_only_leaf_size_and_preserves_native_grab_offset() {
        let leaf = egui::Rect::from_min_size(egui::pos2(384., 0.), egui::vec2(376., 510.));
        let outer = egui::Rect::from_min_size(egui::pos2(600., 250.), egui::vec2(762., 542.));
        let inner = egui::Rect::from_min_size(egui::pos2(601., 281.), egui::vec2(760., 510.));
        let geometry = native_drag_geometry(leaf, Some(outer), Some(inner));
        assert_eq!(geometry.size(), leaf.size());
        assert_eq!(geometry.min, leaf.min + (outer.min - inner.min));
        let press = inner.min + leaf.min.to_vec2() + egui::vec2(200., 20.);
        let offset = press - (inner.min + geometry.min.to_vec2());
        assert_eq!(press - offset, outer.min + leaf.min.to_vec2());
    }
    #[test]
    fn decorated_header_moves_keep_outer_origin_and_full_client_size() {
        let client = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(760., 510.));
        let outer = egui::Rect::from_min_size(egui::pos2(600., 250.), egui::vec2(762., 542.));
        let inner = egui::Rect::from_min_size(egui::pos2(601., 281.), client.size());
        let geometry = native_drag_geometry(client, Some(outer), Some(inner));
        assert_eq!(geometry.size(), client.size());
        let press = inner.min + egui::vec2(400., 20.);
        let offset = press - (inner.min + geometry.min.to_vec2());
        assert_eq!(
            press + egui::vec2(100., 0.) - offset,
            outer.min + egui::vec2(100., 0.)
        );
        assert_eq!(native_drag_geometry(client, None, None), client);
    }
    #[test]
    fn removed_monitor_and_large_dpi_placement_remain_inside_work_area() {
        let mut layout = DockLayout::single(EditorType::Scene);
        layout.detach(1).unwrap();
        let mut area = layout.detached[0].clone();
        area.position = Some([9000., 3000.]);
        area.size = [1580., 940.];
        let work = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280., 680.));
        let (pos, size) = placement(&area, &[work], 2.);
        let physical = egui::Rect::from_min_size(pos * 2., size * 2.);
        assert!(work.contains_rect(physical));
        assert!(physical.height() <= 600.);
    }
    #[test]
    fn secondary_scale_uses_physical_saved_origin_on_its_own_monitor() {
        let mut layout = DockLayout::single(EditorType::Scene);
        layout.detach(1).unwrap();
        let mut area = layout.detached[0].clone();
        area.position = Some([2100., 160.]);
        area.size = [800., 600.];
        let monitors = [
            egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(1920., 1040.)),
            egui::Rect::from_min_max(egui::pos2(1920., 0.), egui::pos2(4480., 1400.)),
        ];
        let (pos, size) = placement(&area, &monitors, 2.);
        let physical = egui::Rect::from_min_size(pos * 2., size * 2.);
        assert!(monitors[1].contains_rect(physical));
        assert_eq!(physical.left(), 2100.);
        let (pos, size) = placement(&area, &monitors[..1], 1.);
        assert!(monitors[0].contains_rect(egui::Rect::from_min_size(pos, size)));
    }
}
