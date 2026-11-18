//! One move transaction across native viewports; only personal layout is edited.
use super::{
    Editor,
    dock::{Axis, DockLayout, WorkspaceKind},
};
use eframe::egui::{self, Pos2, Rect, Vec2};

pub(super) struct AreaDrag {
    pub id: u64,
    workspace: WorkspaceKind,
    original: DockLayout,
    source_slot: Option<(Option<u64>, Rect)>,
    was_dirty: bool,
    offset: Vec2,
    capture: usize,
    pub pointer: Pos2,
    pub cancelled: bool,
    refocus: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Destination {
    id: u64,
    axis: Axis,
    before: bool,
    preview: Rect,
}

fn destination(targets: &[(u64, Rect)], pointer: Pos2) -> Option<Destination> {
    let &(id, rect) = targets.iter().find(|(_, rect)| rect.contains(pointer))?;
    // Pick the closest normalized edge, including for narrow/short areas.
    let p = (pointer - rect.min) / rect.size();
    let edges = [p.x, 1. - p.x, p.y, 1. - p.y];
    let edge = (0..4).min_by(|&a, &b| edges[a].total_cmp(&edges[b]))?;
    let axis = if edge < 2 {
        Axis::Horizontal
    } else {
        Axis::Vertical
    };
    let before = edge % 2 == 0;
    let mut preview = rect;
    match (axis, before) {
        (Axis::Horizontal, true) => preview.max.x = rect.center().x,
        (Axis::Horizontal, false) => preview.min.x = rect.center().x,
        (Axis::Vertical, true) => preview.max.y = rect.center().y,
        (Axis::Vertical, false) => preview.min.y = rect.center().y,
    }
    Some(Destination {
        id,
        axis,
        before,
        preview,
    })
}

#[cfg(any(windows, test))]
pub(super) fn created_window(
    before: &std::collections::BTreeSet<usize>,
    after: &std::collections::BTreeSet<usize>,
) -> Option<usize> {
    let mut created = after.difference(before);
    let handle = *created.next()?;
    created.next().is_none().then_some(handle)
}

#[cfg(any(windows, test))]
pub(super) fn bind_created_host(
    windows: &mut std::collections::BTreeMap<Option<(String, u64)>, usize>,
    workspace: &str,
    host: u64,
    before: &std::collections::BTreeSet<usize>,
    current: &std::collections::BTreeSet<usize>,
) {
    let key = Some((workspace.to_owned(), host));
    if !windows
        .get(&key)
        .is_some_and(|handle| current.contains(handle))
    {
        windows.remove(&key);
        let created = created_window(before, current);
        if std::env::var_os("MODULE_ENGINE_DOCK_TRACE").is_some() {
            static WARNINGS: std::sync::atomic::AtomicUsize =
                std::sync::atomic::AtomicUsize::new(0);
            if WARNINGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 64 {
                eprintln!(
                    "Dock bind {key:?}: before={before:?} current={current:?} created={created:?}"
                );
            }
        }
        if let Some(handle) = created {
            windows.insert(key, handle);
        }
    }
}

// Windows pointer capture belongs to the initiating OS window until completion.
// Global physical coordinates remain valid when the cursor leaves its client area.
#[cfg(windows)]
pub(super) mod native_pointer {
    use super::*;
    use std::ffi::c_void;
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
        fn GetAsyncKeyState(key: i32) -> i16;
        fn GetActiveWindow() -> *mut c_void;
        fn GetForegroundWindow() -> *mut c_void;
        fn GetWindowThreadProcessId(window: *mut c_void, process: *mut u32) -> u32;
        fn SetCapture(window: *mut c_void) -> *mut c_void;
        fn GetCapture() -> *mut c_void;
        fn ReleaseCapture() -> i32;
        fn GetTopWindow(window: *mut c_void) -> *mut c_void;
        fn GetWindow(window: *mut c_void, command: u32) -> *mut c_void;
        fn IsWindowVisible(window: *mut c_void) -> i32;
        fn IsIconic(window: *mut c_void) -> i32;
        fn GetWindowRect(window: *mut c_void, rect: *mut WinRect) -> i32;
        fn GetWindowLongPtrW(window: *mut c_void, index: i32) -> isize;
    }
    pub fn own_windows() -> std::collections::BTreeSet<usize> {
        let mut handles = std::collections::BTreeSet::new();
        unsafe {
            let mut window = GetTopWindow(std::ptr::null_mut());
            while !window.is_null() {
                let mut process = 0;
                GetWindowThreadProcessId(window, &mut process);
                if process == std::process::id() {
                    handles.insert(window as usize);
                }
                window = GetWindow(window, 2);
            }
        }
        handles
    }
    /// Actual HWND identity owns the drop, independent of title/geometry/order
    /// of creation. Foreign or unbound covering windows block hidden targets.
    pub fn drop_host(
        point: Pos2,
        hosts: &std::collections::BTreeMap<Option<(String, u64)>, usize>,
        moving: Option<u64>,
        workspace: &str,
    ) -> Option<Option<u64>> {
        unsafe {
            let mut window = GetTopWindow(std::ptr::null_mut());
            while !window.is_null() {
                let mut bounds = WinRect {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                if IsWindowVisible(window) != 0
                    && IsIconic(window) == 0
                    && GetWindowRect(window, &mut bounds) != 0
                    && Rect::from_min_max(
                        egui::pos2(bounds.left as f32, bounds.top as f32),
                        egui::pos2(bounds.right as f32, bounds.bottom as f32),
                    )
                    .contains(point)
                {
                    let mut process = 0;
                    GetWindowThreadProcessId(window, &mut process);
                    let extended_style = GetWindowLongPtrW(window, -20);
                    if std::env::var_os("MODULE_ENGINE_DOCK_TRACE").is_some() {
                        eprintln!(
                            "Dock native hit: point={point:?} hwnd={} pid={process} exstyle={:x} moving={moving:?} workspace={workspace} bindings={hosts:?}",
                            window as usize, extended_style
                        );
                    }
                    // Layered WS_EX_TRANSPARENT windows pass mouse input through
                    // (Win32 Window Features / Layered Windows). The transparent
                    // style alone only controls painting order.
                    if extended_style & 0x0008_0020 == 0x0008_0020 {
                        window = GetWindow(window, 2);
                        continue;
                    }
                    if process != std::process::id() {
                        return None;
                    }
                    let key = hosts
                        .iter()
                        .find(|(key, handle)| {
                            **handle == window as usize
                                && key.as_ref().is_none_or(|(space, _)| space == workspace)
                        })?
                        .0;
                    let host = key.as_ref().map(|(_, host)| *host);
                    if Some(host) != moving.map(Some) {
                        return Some(host);
                    }
                }
                window = GetWindow(window, 2);
            }
        }
        None
    }
    pub fn sample() -> Option<(Pos2, bool, bool)> {
        let mut point = Point { x: 0, y: 0 };
        unsafe {
            if GetCursorPos(&mut point) == 0 {
                return None;
            }
            Some((
                egui::pos2(point.x as f32, point.y as f32),
                GetAsyncKeyState(1) < 0,
                GetAsyncKeyState(27) < 0,
            ))
        }
    }
    pub fn capture() -> usize {
        unsafe {
            let window = GetActiveWindow();
            if !window.is_null() {
                SetCapture(window);
            }
            window as usize
        }
    }
    pub fn release(owner: usize) {
        unsafe {
            if owner != 0 && GetCapture() as usize == owner {
                ReleaseCapture();
            }
        }
    }
    pub fn owns_capture(owner: usize) -> bool {
        owner != 0 && unsafe { GetCapture() as usize == owner }
    }
    pub fn foreground_is_ours() -> bool {
        let mut process = 0;
        unsafe {
            GetWindowThreadProcessId(GetForegroundWindow(), &mut process);
        }
        process == std::process::id()
    }
}

fn pointer(ctx: &egui::Context) -> Option<(Pos2, bool, bool)> {
    #[cfg(windows)]
    if !ctx.embed_viewports() {
        return native_pointer::sample();
    }
    let scale = ctx.pixels_per_point();
    ctx.input(|i| {
        let origin = i
            .viewport()
            .inner_rect
            .map(|rect| rect.min)
            .unwrap_or_default();
        i.pointer.interact_pos().map(|p| {
            (
                (origin + p.to_vec2()) * scale,
                i.pointer.primary_down(),
                i.key_pressed(egui::Key::Escape),
            )
        })
    })
}

impl Editor {
    /// Closing the origin host commits the moved leaf's current membership.
    /// A later Escape must not reconstruct the now-closed origin window.
    pub(super) fn area_host_was_closed_during_drag(&mut self, host: u64, ctx: &egui::Context) {
        if let Some(id) = self
            .area_drag
            .as_ref()
            .filter(|drag| {
                drag.original.host_for(drag.id) == Some(host)
                    || self.project_layout.host_for(drag.id) == Some(host)
                        && drag.workspace == WorkspaceKind::Project
                    || self.engine_layout.host_for(drag.id) == Some(host)
                        && drag.workspace == WorkspaceKind::Engine
            })
            .map(|drag| drag.id)
        {
            self.area_was_closed_during_drag(id, ctx);
            self.personal_dirty = true;
        }
    }
    /// An explicit close is not a cancelled move. It must survive any later
    /// Escape/capture loss, including when another area's move is in flight.
    pub(super) fn area_was_closed_during_drag(&mut self, id: u64, ctx: &egui::Context) {
        if let Some(mut drag) = self.area_drag.take() {
            if drag.id == id {
                #[cfg(windows)]
                native_pointer::release(drag.capture);
                ctx.stop_dragging();
            } else {
                drag.original.close(id);
                drag.was_dirty = true;
                self.area_drag = Some(drag);
            }
        }
        if self.pending_area_focus == Some(id) {
            self.pending_area_focus = None;
        }
        self.pending_area_geometry
            .remove(&(self.workspace_key(), id));
    }
    pub(super) fn allow_area_geometry_readback(&mut self, workspace: &str, id: u64) -> bool {
        // The callback sees geometry from before its builder delta is applied.
        // A held/cancelled move owns it; completion skips one such stale sample.
        if self.area_drag.as_ref().is_some_and(|drag| {
            let layout = match drag.workspace {
                WorkspaceKind::Engine => &self.engine_layout,
                WorkspaceKind::Project => &self.project_layout,
            };
            drag.id == id || layout.host_for(drag.id) == Some(id)
        }) {
            return false;
        }
        !self
            .pending_area_geometry
            .remove(&(workspace.to_owned(), id))
    }
    pub(super) fn area_drag_over_main(&self) -> bool {
        self.area_drag.as_ref().is_some_and(|drag| {
            !drag.cancelled
                && self
                    .area_drop_destination(drag, drag.pointer, true)
                    .is_some()
        })
    }
    fn area_drop_destination(
        &self,
        drag: &AreaDrag,
        point: Pos2,
        native: bool,
    ) -> Option<Destination> {
        let layout = match drag.workspace {
            WorkspaceKind::Engine => &self.engine_layout,
            WorkspaceKind::Project => &self.project_layout,
        };
        let moving = layout.host_for(drag.id);
        let trace = std::env::var_os("MODULE_ENGINE_DOCK_TRACE").is_some();
        if trace {
            eprintln!(
                "Dock resolve: workspace={} leaf={} moving_host={moving:?} point={point:?} source_slot={:?} targets={:?}",
                self.workspace_key(),
                drag.id,
                drag.source_slot,
                self.area_drop_targets
            );
        }
        #[cfg(windows)]
        let receiver = if native {
            let hit = native_pointer::drop_host(
                point,
                &self.area_host_windows,
                moving,
                &self.workspace_key(),
            );
            if trace {
                eprintln!("Dock native receiver: {hit:?}");
            }
            Some(hit?)
        } else {
            None
        };
        #[cfg(not(windows))]
        let receiver: Option<Option<u64>> = {
            let _ = native;
            None
        };
        if let Some((host, rect)) = drag.source_slot
            && receiver == Some(host)
            && host != moving
            && rect.contains(point)
        {
            if trace {
                eprintln!("Dock decision: original source slot");
            }
            return destination(&[(drag.id, rect)], point);
        }
        let targets = self
            .area_drop_targets
            .iter()
            .copied()
            .filter(|(id, _)| {
                *id != drag.id
                    && (layout.host_for(*id) != moving || moving.is_none())
                    && receiver.is_none_or(|host| layout.host_for(*id) == host)
            })
            .collect::<Vec<_>>();
        let selected = destination(&targets, point);
        if trace {
            eprintln!("Dock decision: eligible_targets={targets:?} selected={selected:?}");
        }
        selected
    }
    pub(super) fn begin_area_drag(
        &mut self,
        id: u64,
        rect: Rect,
        start: Pos2,
        ctx: &egui::Context,
    ) {
        if std::env::var_os("MODULE_ENGINE_DOCK_TRACE").is_some() {
            eprintln!(
                "Dock begin {id}: native={:?}, egui={:?}, embed={}",
                pointer(ctx),
                ctx.input(|i| i.pointer.primary_down()),
                ctx.embed_viewports()
            );
        }
        if self.area_drag.is_some()
            || ctx.embed_viewports()
            || self.workflow.blocks_document_edits()
            || self.scene_source_ui.active()
        {
            return;
        }
        let Some((pointer, held, _)) = pointer(ctx) else {
            return;
        };
        #[cfg(not(windows))]
        let _ = held;
        let original = self.active_layout_mut().clone();
        let source_slot = self
            .area_drop_targets
            .iter()
            .find(|(pane, _)| *pane == id)
            .map(|(_, rect)| (original.host_for(id), *rect));
        let was_dirty = self.personal_dirty;
        let scale = ctx.pixels_per_point();
        let origin = ctx
            .input(|i| i.viewport().inner_rect.map(|rect| rect.min))
            .unwrap_or_default()
            * scale;
        let start = origin + start.to_vec2() * scale;
        let offset = start - (origin + rect.min.to_vec2() * scale);
        if original.host_for(id).is_none()
            || original.detached.iter().any(|host| {
                host.panes().len() > 1 && host.panes().iter().any(|(pane, _)| *pane == id)
            })
        {
            self.detach_editor_area(id, ctx);
        }
        let Some(area) = self
            .active_layout_mut()
            .detached
            .iter_mut()
            .find(|area| area.panes().iter().any(|(pane, _)| *pane == id))
        else {
            return;
        };
        area.size = rect.size().max(egui::vec2(240., 200.)).into();
        area.position = Some((pointer - offset).into());
        #[cfg(windows)]
        let capture = if held { native_pointer::capture() } else { 0 };
        #[cfg(not(windows))]
        let capture = 0;
        self.area_drag = Some(AreaDrag {
            id,
            workspace: self.workspace,
            original,
            source_slot,
            was_dirty,
            offset,
            capture,
            pointer,
            cancelled: false,
            refocus: true,
        });
        self.cancel_active_scene_gestures();
        ctx.request_repaint();
    }

    pub(super) fn update_area_drag(&mut self, ctx: &egui::Context) {
        self.advance_area_drag(ctx, pointer(ctx));
    }

    fn advance_area_drag(&mut self, ctx: &egui::Context, sample: Option<(Pos2, bool, bool)>) {
        let Some(mut drag) = self.area_drag.take() else {
            return;
        };
        let (point, held, escape) = sample.unwrap_or((drag.pointer, false, true));
        drag.pointer = point;
        #[cfg(windows)]
        let capture_lost = drag.capture != 0 && held && !native_pointer::owns_capture(drag.capture);
        #[cfg(not(windows))]
        let capture_lost = false;
        #[cfg(windows)]
        let deactivated = !ctx.embed_viewports() && !native_pointer::foreground_is_ours();
        #[cfg(not(windows))]
        let deactivated = false;
        if capture_lost || deactivated || sample.is_none() || self.workspace != drag.workspace {
            drag.refocus = false;
        }
        let cancel = escape || capture_lost || deactivated || self.workspace != drag.workspace;
        let drop_target = self.area_drop_destination(&drag, point, !ctx.embed_viewports());
        let layout = match drag.workspace {
            WorkspaceKind::Engine => &mut self.engine_layout,
            WorkspaceKind::Project => &mut self.project_layout,
        };
        if cancel || drag.cancelled {
            if !drag.cancelled {
                *layout = drag.original.clone();
            }
            self.personal_dirty = drag.was_dirty;
            self.pending_area_focus = None;
            drag.cancelled = true;
            // Keep the initiating capture through the physical release, so
            // Escape outside the client cannot leave egui's button held.
            if held {
                self.area_drag = Some(drag);
                ctx.request_repaint();
                return;
            }
        } else if !held {
            if let Some(area) = layout
                .detached
                .iter_mut()
                .find(|area| area.panes().iter().any(|(pane, _)| *pane == drag.id))
            {
                area.position = Some((point - drag.offset).into());
            }
            if let Some(target) = drop_target {
                if target.id == drag.id {
                    // A fast out-and-back can finish before the next root layout.
                    *layout = drag.original.clone();
                } else {
                    match layout.dock_at(drag.id, target.id, target.axis, target.before) {
                        Ok(()) => self.pending_area_focus = Some(drag.id),
                        Err(error) => self.status = error,
                    }
                }
            }
            self.personal_dirty = true;
        } else {
            if let Some(area) = layout
                .detached
                .iter_mut()
                .find(|area| area.panes().iter().any(|(pane, _)| *pane == drag.id))
            {
                area.position = Some((point - drag.offset).into());
            }
            self.area_drag = Some(drag);
            ctx.request_repaint();
            return;
        }
        #[cfg(windows)]
        native_pointer::release(drag.capture);
        #[cfg(not(windows))]
        let _capture = drag.capture;
        ctx.stop_dragging();
        if self
            .active_layout_mut()
            .detached
            .iter()
            .any(|area| area.panes().iter().any(|(pane, _)| *pane == drag.id))
        {
            let host = self.active_layout_mut().host_for(drag.id).unwrap();
            self.pending_area_geometry
                .insert((self.workspace_key(), host));
        }
        if drag.refocus {
            self.activate_area(drag.id);
            let viewport = if self
                .active_layout_mut()
                .detached
                .iter()
                .any(|area| area.panes().iter().any(|(pane, _)| *pane == drag.id))
            {
                egui::ViewportId::from_hash_of((
                    "editor-area",
                    self.workspace_key(),
                    self.active_layout_mut().host_for(drag.id).unwrap(),
                ))
            } else {
                egui::ViewportId::ROOT
            };
            ctx.send_viewport_cmd_to(viewport, egui::ViewportCommand::Focus);
        }
        ctx.request_repaint();
    }

    pub(super) fn draw_area_drop_preview(&self, ui: &egui::Ui) {
        let Some(drag) = &self.area_drag else {
            return;
        };
        if drag.cancelled {
            return;
        }
        let Some(target) = self.area_drop_destination(
            drag,
            pointer(ui.ctx()).map(|s| s.0).unwrap_or(drag.pointer),
            !ui.ctx().embed_viewports(),
        ) else {
            return;
        };
        let scale = ui.ctx().pixels_per_point();
        let origin = ui
            .input(|i| i.viewport().inner_rect.map(|rect| rect.min))
            .unwrap_or_default()
            * scale;
        let rect = Rect::from_min_max(
            (target.preview.min - origin.to_vec2()) / scale,
            (target.preview.max - origin.to_vec2()) / scale,
        );
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("area-drop-preview"),
        ));
        painter.rect_filled(
            rect.shrink(2.),
            2.,
            egui::Color32::from_rgba_unmultiplied(67, 168, 156, 90),
        );
        painter.rect_stroke(
            rect.shrink(2.),
            2.,
            egui::Stroke::new(2., egui::Color32::from_rgb(67, 168, 156)),
            egui::StrokeKind::Inside,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloned_workspace_reused_host_id_binds_new_hwnd_before_old_workspace_cleanup() {
        use std::collections::{BTreeMap, BTreeSet};
        let mut bindings = BTreeMap::from([(None, 10), (Some(("custom-1".into(), 1)), 20)]);
        let before = BTreeSet::from([10, 20]);
        let during_callback = BTreeSet::from([10, 20, 30]);
        bind_created_host(&mut bindings, "custom-2", 1, &before, &during_callback);
        assert_eq!(bindings[&Some(("custom-1".into(), 1))], 20);
        assert_eq!(bindings[&Some(("custom-2".into(), 1))], 30);
        let after_cleanup = BTreeSet::from([10, 30]);
        bind_created_host(&mut bindings, "custom-2", 1, &after_cleanup, &after_cleanup);
        assert_eq!(bindings[&Some(("custom-2".into(), 1))], 30);
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "Native Win32 z-order fixture; run explicitly and sequentially"]
    fn native_z_order_uses_bound_hwnds_for_coincident_windows_and_excludes_only_moving_host() {
        use std::ffi::c_void;
        #[link(name = "user32")]
        unsafe extern "system" {
            fn CreateWindowExW(
                ex: u32,
                class: *const u16,
                title: *const u16,
                style: u32,
                x: i32,
                y: i32,
                w: i32,
                h: i32,
                parent: *mut c_void,
                menu: *mut c_void,
                instance: *mut c_void,
                param: *mut c_void,
            ) -> *mut c_void;
            fn DestroyWindow(window: *mut c_void) -> i32;
            fn SetWindowPos(
                window: *mut c_void,
                after: *mut c_void,
                x: i32,
                y: i32,
                w: i32,
                h: i32,
                flags: u32,
            ) -> i32;
        }
        unsafe extern "system" {
            fn SetLayeredWindowAttributes(
                window: *mut c_void,
                key: u32,
                alpha: u8,
                flags: u32,
            ) -> i32;
        }
        struct OwnedWindow(*mut c_void);
        impl OwnedWindow {
            fn new() -> Self {
                Self::with_style(0x08000080)
            }
            fn with_style(extended_style: u32) -> Self {
                let class = "STATIC\0".encode_utf16().collect::<Vec<_>>();
                let title = "identical fixture title\0"
                    .encode_utf16()
                    .collect::<Vec<_>>();
                let handle = unsafe {
                    CreateWindowExW(
                        extended_style,
                        class.as_ptr(),
                        title.as_ptr(),
                        0x90000000,
                        -30000,
                        -30000,
                        760,
                        510,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                assert!(!handle.is_null());
                if extended_style & 0x00080000 != 0 {
                    assert_ne!(unsafe { SetLayeredWindowAttributes(handle, 0, 255, 2) }, 0);
                }
                Self(handle)
            }
            fn raise(&self) {
                assert_ne!(
                    unsafe { SetWindowPos(self.0, std::ptr::null_mut(), 0, 0, 0, 0, 0x13) },
                    0
                );
            }
        }
        impl Drop for OwnedWindow {
            fn drop(&mut self) {
                unsafe {
                    DestroyWindow(self.0);
                }
            }
        }
        let before = native_pointer::own_windows();
        let root = OwnedWindow::new();
        assert_eq!(
            created_window(&before, &native_pointer::own_windows()),
            Some(root.0 as usize)
        );
        let target = OwnedWindow::new();
        let moving = OwnedWindow::new();
        let hosts = std::collections::BTreeMap::from([
            (None, root.0 as usize),
            (Some(("custom-42".into(), 7)), target.0 as usize),
            (Some(("custom-42".into(), 1)), moving.0 as usize),
        ]);
        let point = egui::pos2(-29900., -29900.);
        assert_eq!(
            native_pointer::drop_host(point, &hosts, Some(1), "custom-42"),
            Some(Some(7))
        );
        root.raise();
        assert_eq!(
            native_pointer::drop_host(point, &hosts, Some(1), "custom-42"),
            Some(None)
        );
        target.raise();
        assert_eq!(
            native_pointer::drop_host(point, &hosts, Some(1), "custom-42"),
            Some(Some(7))
        );
        let cover = OwnedWindow::new();
        cover.raise();
        assert_eq!(
            native_pointer::drop_host(point, &hosts, Some(1), "custom-42"),
            None,
            "an unbound cover must not dock into hidden main"
        );
        drop(cover);
        assert_eq!(
            native_pointer::drop_host(point, &hosts, Some(1), "custom-42"),
            Some(Some(7))
        );
        let passthrough = OwnedWindow::with_style(0x080800a0);
        passthrough.raise();
        assert_eq!(
            native_pointer::drop_host(point, &hosts, Some(1), "custom-42"),
            Some(Some(7)),
            "a layered input-passthrough overlay does not cover a docking target"
        );
    }
    #[test]
    fn native_creation_binding_requires_one_new_own_window_without_geometry_guessing() {
        use std::collections::BTreeSet;
        let before = BTreeSet::from([10, 20]);
        assert_eq!(
            created_window(&before, &BTreeSet::from([10, 20, 30])),
            Some(30)
        );
        assert_eq!(created_window(&before, &before), None);
        assert_eq!(
            created_window(&before, &BTreeSet::from([10, 20, 30, 40])),
            None
        );
    }
    #[test]
    fn stationary_held_moving_host_is_never_its_own_outside_drop_target() {
        let (_fixture, mut editor) =
            super::super::editor_workflow::workflow_tests::Fixture::new(true);
        editor.workspace = WorkspaceKind::Project;
        editor.project_layout = DockLayout::single(super::super::dock::EditorType::Scene);
        let original = editor.project_layout.clone();
        editor.project_layout.detach(1).unwrap();
        let point = egui::pos2(1000., 700.);
        editor.area_drop_targets = vec![(
            1,
            Rect::from_min_size(point - egui::vec2(20., 30.), egui::vec2(760., 510.)),
        )];
        editor.area_drag = Some(AreaDrag {
            id: 1,
            workspace: WorkspaceKind::Project,
            original,
            source_slot: None,
            was_dirty: false,
            offset: egui::vec2(20., 30.),
            capture: 0,
            pointer: point,
            cancelled: false,
            refocus: false,
        });
        let ctx = egui::Context::default();
        editor.advance_area_drag(&ctx, Some((point, true, false)));
        editor.advance_area_drag(&ctx, Some((point, true, false)));
        editor.advance_area_drag(&ctx, Some((point, false, false)));
        assert!(editor.area_drag.is_none());
        assert_eq!(editor.project_layout.host_for(1), Some(1));
        assert_eq!(
            editor.project_layout.detached[0].position,
            Some([980., 670.])
        );
        assert!(editor.personal_dirty);
    }
    #[test]
    fn closing_origin_floating_host_during_tearoff_keeps_moved_leaf_without_rollback() {
        let (_fixture, mut editor) =
            super::super::editor_workflow::workflow_tests::Fixture::new(true);
        editor.workspace = WorkspaceKind::Project;
        editor.project_layout = DockLayout::single(super::super::dock::EditorType::Scene);
        editor.project_layout.detach(1).unwrap();
        let sibling = editor.project_layout.split(1, Axis::Horizontal).unwrap();
        let original = editor.project_layout.clone();
        editor.project_layout.detach(1).unwrap();
        let moved_host = editor.project_layout.host_for(1).unwrap();
        editor.area_drag = Some(AreaDrag {
            id: 1,
            workspace: WorkspaceKind::Project,
            original,
            source_slot: None,
            was_dirty: false,
            offset: Vec2::ZERO,
            capture: 0,
            pointer: Pos2::ZERO,
            cancelled: false,
            refocus: false,
        });
        let ctx = egui::Context::default();
        editor.area_host_was_closed_during_drag(1, &ctx);
        assert_eq!(editor.project_layout.close_host(1), vec![sibling]);
        editor.area_was_closed_during_drag(sibling, &ctx);
        editor.advance_area_drag(&ctx, Some((Pos2::ZERO, true, true)));
        editor.advance_area_drag(&ctx, Some((Pos2::ZERO, false, false)));
        assert!(editor.area_drag.is_none());
        assert!(editor.personal_dirty);
        assert_eq!(editor.project_layout.host_for(1), Some(moved_host));
        assert!(
            !editor
                .project_layout
                .panes()
                .iter()
                .any(|(id, _)| *id == sibling)
        );
        editor.project_layout.validate().unwrap();
    }
    #[test]
    fn explicit_close_survives_move_release_and_cancellation_without_resurrecting_siblings() {
        let (_fixture, mut editor) =
            super::super::editor_workflow::workflow_tests::Fixture::new(true);
        editor.workspace = WorkspaceKind::Project;
        let ctx = egui::Context::default();
        for closed in [1, 3] {
            editor.project_layout = DockLayout::single(super::super::dock::EditorType::Scene);
            editor.project_layout.split(1, Axis::Horizontal).unwrap(); // sibling 3
            let original = editor.project_layout.clone();
            editor.project_layout.detach(1).unwrap();
            editor.project_layout.detach(3).unwrap();
            editor.area_drag = Some(AreaDrag {
                id: 1,
                workspace: WorkspaceKind::Project,
                original,
                source_slot: None,
                was_dirty: false,
                offset: Vec2::ZERO,
                capture: 0,
                pointer: Pos2::ZERO,
                cancelled: false,
                refocus: false,
            });
            editor.pending_area_focus = Some(closed);
            assert!(editor.project_layout.close(closed));
            editor.area_was_closed_during_drag(closed, &ctx);
            editor.advance_area_drag(&ctx, Some((Pos2::ZERO, true, true)));
            editor.advance_area_drag(&ctx, Some((Pos2::ZERO, false, false)));
            assert!(
                !editor
                    .project_layout
                    .panes()
                    .iter()
                    .any(|(id, _)| *id == closed)
            );
            let sibling = if closed == 1 { 3 } else { 1 };
            assert!(
                editor
                    .project_layout
                    .panes()
                    .iter()
                    .any(|(id, _)| *id == sibling)
            );
            assert_ne!(editor.pending_area_focus, Some(closed));
        }
    }
    #[test]
    fn move_cancel_and_dock_keep_drafts_document_and_shared_undo() {
        let (_fixture, mut editor) =
            super::super::editor_workflow::workflow_tests::Fixture::new(true);
        editor.workspace = WorkspaceKind::Project;
        editor.project_layout = DockLayout::single(super::super::dock::EditorType::ProjectSettings);
        let target = editor.project_layout.split(1, Axis::Horizontal).unwrap();
        let mut memory = super::super::panes::PaneMemory::new(Default::default(), &editor.document);
        memory.current_source = "unsaved panel draft".into();
        editor.project_panes.insert(1, memory);
        let initial_doc = editor.document.clone();
        editor.document.name = "one authored edit".into();
        editor.history.record("authored edit", &editor.document);
        let authored = editor.document.clone();
        let history_id = editor.history.current_id();
        let ctx = egui::Context::default();
        for (point, held, escape) in [
            (egui::pos2(20., 20.), true, true),
            (egui::pos2(-200., 10.), false, false),
            (egui::pos2(20., 20.), false, false),
        ] {
            let original = editor.project_layout.clone();
            if editor
                .project_layout
                .docked_panes()
                .iter()
                .any(|(id, _)| *id == 1)
            {
                editor.project_layout.detach(1).unwrap();
            }
            editor.area_drop_targets = vec![(
                target,
                Rect::from_min_size(Pos2::ZERO, egui::vec2(400., 200.)),
            )];
            editor.area_drag = Some(AreaDrag {
                id: 1,
                workspace: WorkspaceKind::Project,
                original: original.clone(),
                source_slot: None,
                was_dirty: false,
                offset: Vec2::ZERO,
                capture: 0,
                pointer: point,
                cancelled: false,
                refocus: true,
            });
            editor.advance_area_drag(&ctx, Some((point, held, escape)));
            if escape {
                assert_eq!(editor.project_layout, original);
                assert!(
                    editor.area_drag.is_some(),
                    "capture is retained until physical release"
                );
                editor.advance_area_drag(&ctx, Some((point, false, false)));
            }
            assert!(editor.area_drag.is_none());
            if escape {
                assert_eq!(editor.project_layout, original);
                assert!(!editor.personal_dirty);
            } else if point.x < 0. {
                assert!(
                    editor
                        .project_layout
                        .detached
                        .iter()
                        .any(|area| area.id == 1)
                );
            } else {
                assert!(editor.project_layout.detached.is_empty());
            }
            assert_eq!(editor.document, authored);
            assert_eq!(editor.history.current_id(), history_id);
            assert_eq!(editor.history.entries().len(), 2);
            assert_eq!(
                editor.project_panes[&1].current_source,
                "unsaved panel draft"
            );
        }
        assert_eq!(editor.history.undo(), Some(initial_doc));
        assert_eq!(editor.history.redo(), Some(authored));
        let restored: DockLayout =
            serde_json::from_str(&serde_json::to_string(&editor.project_layout).unwrap()).unwrap();
        assert_eq!(restored, editor.project_layout);
        // A cancelled move caused by external deactivation/capture loss must
        // never enqueue native refocus, including the eventual release.
        let original = editor.project_layout.clone();
        editor.project_layout.detach(1).unwrap();
        editor.pending_area_focus = None;
        editor.area_drag = Some(AreaDrag {
            id: 1,
            workspace: WorkspaceKind::Project,
            original: original.clone(),
            source_slot: None,
            was_dirty: false,
            offset: Vec2::ZERO,
            capture: 0,
            pointer: Pos2::ZERO,
            cancelled: false,
            refocus: false,
        });
        editor.advance_area_drag(&ctx, Some((Pos2::ZERO, true, true)));
        assert_eq!(editor.project_layout, original);
        assert!(editor.pending_area_focus.is_none());
        editor.advance_area_drag(&ctx, Some((Pos2::ZERO, false, false)));
        assert!(editor.area_drag.is_none());
        assert!(editor.pending_area_focus.is_none());
    }
    #[test]
    fn floating_release_uses_final_pointer_and_escape_restoration_owns_geometry() {
        let (_fixture, mut editor) =
            super::super::editor_workflow::workflow_tests::Fixture::new(true);
        editor.workspace = WorkspaceKind::Project;
        editor.project_layout = DockLayout::single(super::super::dock::EditorType::ProjectSettings);
        editor.project_layout.detach(1).unwrap();
        let ctx = egui::Context::default();
        let make_drag = |original| AreaDrag {
            id: 1,
            workspace: WorkspaceKind::Project,
            original,
            source_slot: None,
            was_dirty: false,
            offset: egui::vec2(20., 30.),
            capture: 0,
            pointer: Pos2::ZERO,
            cancelled: false,
            refocus: false,
        };
        editor.area_drop_targets.clear();
        editor.area_drag = Some(make_drag(editor.project_layout.clone()));
        editor.advance_area_drag(&ctx, Some((egui::pos2(320., 230.), true, false)));
        editor.advance_area_drag(&ctx, Some((egui::pos2(420., 330.), false, false)));
        assert_eq!(
            editor.project_layout.detached[0].position,
            Some([400., 300.])
        );
        let workspace = editor.workspace_key();
        assert!(!editor.allow_area_geometry_readback(&workspace, 1));
        assert!(editor.allow_area_geometry_readback(&workspace, 1));
        let original = editor.project_layout.clone();
        editor.area_drag = Some(make_drag(original.clone()));
        editor.advance_area_drag(&ctx, Some((egui::pos2(620., 530.), true, false)));
        assert_eq!(
            editor.project_layout.detached[0].position,
            Some([600., 500.])
        );
        editor.advance_area_drag(&ctx, Some((egui::pos2(620., 530.), true, true)));
        assert_eq!(editor.project_layout, original);
        assert!(!editor.allow_area_geometry_readback(&workspace, 1));
        editor.advance_area_drag(&ctx, Some((egui::pos2(620., 530.), false, false)));
        assert_eq!(editor.project_layout, original);
        assert!(!editor.allow_area_geometry_readback(&workspace, 1));
        assert!(editor.allow_area_geometry_readback(&workspace, 1));
    }
    #[test]
    fn preview_side_drives_the_actual_stable_identity_insertion() {
        for (p, axis, before) in [
            (egui::pos2(2., 100.), Axis::Horizontal, true),
            (egui::pos2(398., 100.), Axis::Horizontal, false),
            (egui::pos2(200., 2.), Axis::Vertical, true),
            (egui::pos2(200., 198.), Axis::Vertical, false),
        ] {
            let mut layout = DockLayout::single(super::super::dock::EditorType::ProjectSettings);
            layout.detach(1).unwrap();
            let target_id = layout.docked_panes()[0].0;
            let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(400., 200.));
            let target = destination(&[(target_id, rect)], p).unwrap();
            assert_eq!((target.axis, target.before), (axis, before));
            assert!(target.preview.contains(p));
            layout
                .dock_at(1, target.id, target.axis, target.before)
                .unwrap();
            assert!(layout.detached.is_empty());
            assert_eq!(layout.panes().iter().filter(|(id, _)| *id == 1).count(), 1);
            layout.validate().unwrap();
        }
    }
    #[test]
    fn outside_and_rejected_targets_preserve_the_floating_area() {
        let mut layout = DockLayout::single(super::super::dock::EditorType::ProjectSettings);
        layout.detach(1).unwrap();
        let original = layout.clone();
        assert!(
            destination(
                &[(2, Rect::from_min_size(Pos2::ZERO, egui::vec2(400., 200.)))],
                egui::pos2(-1., 20.)
            )
            .is_none()
        );
        assert!(layout.dock_at(1, 999, Axis::Horizontal, true).is_err());
        assert_eq!(layout, original);
    }
}
