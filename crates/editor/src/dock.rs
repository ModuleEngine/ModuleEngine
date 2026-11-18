//! Stable, serializable area trees. Area chrome owns no project/runtime state.
use super::editor_ui::{AREA_HEADER_HEIGHT, CHROME_PAD, COMPACT_CONTROL_SIZE};
use eframe::egui::{self, Color32, CursorIcon, Pos2, Rect, Sense, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceKind {
    Engine,
    Project,
}
impl WorkspaceKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Engine => "engine",
            Self::Project => "project",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorType {
    Viewport,
    Game,
    Scene,
    Hierarchy,
    Inspector,
    Assets,
    GameSystems,
    ObjectTree,
    ObjectInspector,
    ProjectSettings,
    EngineSettings,
    EngineOverview,
    ModuleGraph,
    RustCustom,
    Hlsl,
    ProjectFile,
    History,
    ModuleLibrary,
    Diagnostics,
    Extension(u32),
}
impl EditorType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Viewport => "3D Viewport",
            Self::Game => "Game",
            Self::Scene => "Scene",
            Self::Hierarchy => "Hierarchy",
            Self::Inspector => "Inspector",
            Self::Assets => "Assets",
            Self::GameSystems => "Gameplay Systems",
            Self::ObjectTree => "Object Tree",
            Self::ObjectInspector => "Object / Components",
            Self::ProjectSettings => "Project Settings",
            Self::EngineSettings => "Engine Exposure",
            Self::EngineOverview => "Engine Overview",
            Self::ModuleGraph => "Module Blueprint",
            Self::RustCustom => "Custom / Rust",
            Self::Hlsl => "Custom / HLSL",
            Self::ProjectFile => "Project File",
            Self::History => "History / Merge",
            Self::ModuleLibrary => "Module Library",
            Self::Diagnostics => "Diagnostics",
            Self::Extension(_) => "Module Panel",
        }
    }
    pub fn requires_engine(self) -> bool {
        matches!(
            self,
            Self::Viewport
                | Self::ObjectTree
                | Self::ObjectInspector
                | Self::EngineSettings
                | Self::EngineOverview
                | Self::GameSystems
                | Self::ModuleGraph
                | Self::Hlsl
                | Self::ProjectFile
                | Self::ModuleLibrary
        )
    }
}
#[derive(Clone)]
pub struct EditorDescriptor {
    pub editor: EditorType,
    pub label: String,
    pub requires_engine: bool,
}
impl EditorDescriptor {
    pub fn builtins(engine: bool) -> Vec<Self> {
        [
            EditorType::Viewport,
            EditorType::Game,
            EditorType::Scene,
            EditorType::Hierarchy,
            EditorType::Inspector,
            EditorType::Assets,
            EditorType::GameSystems,
            EditorType::ObjectTree,
            EditorType::ObjectInspector,
            EditorType::ProjectSettings,
            EditorType::EngineSettings,
            EditorType::EngineOverview,
            EditorType::ModuleGraph,
            EditorType::RustCustom,
            EditorType::Hlsl,
            EditorType::ProjectFile,
            EditorType::History,
            EditorType::ModuleLibrary,
            EditorType::Diagnostics,
        ]
        .into_iter()
        .filter(|editor| engine || !editor.requires_engine())
        .map(|editor| Self {
            editor,
            label: editor.label().into(),
            requires_engine: editor.requires_engine(),
        })
        .collect()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Area {
    Pane {
        id: u64,
        editor: EditorType,
    },
    Split {
        id: u64,
        axis: Axis,
        ratio: f32,
        first: Box<Area>,
        second: Box<Area>,
    },
}
impl Area {
    fn panes(&self) -> Vec<(u64, EditorType)> {
        match self {
            Self::Pane { id, editor } => vec![(*id, *editor)],
            Self::Split { first, second, .. } => {
                let mut panes = first.panes();
                panes.extend(second.panes());
                panes
            }
        }
    }
    fn find_mut(&mut self, pane: u64) -> Option<&mut Area> {
        match self {
            Self::Pane { id, .. } if *id == pane => Some(self),
            Self::Split { first, second, .. } => {
                first.find_mut(pane).or_else(|| second.find_mut(pane))
            }
            _ => None,
        }
    }
    fn first_pane_id(&self) -> u64 {
        match self {
            Self::Pane { id, .. } => *id,
            Self::Split { first, .. } => first.first_pane_id(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DockLayout {
    pub root: Area,
    pub next_id: u64,
    #[serde(default)]
    pub detached: Vec<DetachedArea>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetachedArea {
    /// Stable native host identity, independent of its surviving leaf IDs.
    pub id: u64,
    pub editor: EditorType,
    /// Absent in legacy layouts, which describe one pane per native host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<Area>,
    pub neighbor: u64,
    pub axis: Axis,
    pub before: bool,
    pub fallback: Option<u64>,
    pub position: Option<[f32; 2]>,
    pub size: [f32; 2],
}
impl DetachedArea {
    pub fn panes(&self) -> Vec<(u64, EditorType)> {
        self.tree
            .as_ref()
            .map_or_else(|| vec![(self.id, self.editor)], Area::panes)
    }
    fn tree_mut(&mut self) -> &mut Area {
        self.tree.get_or_insert(Area::Pane {
            id: self.id,
            editor: self.editor,
        })
    }
}
#[derive(Clone)]
pub struct PaneRect {
    pub id: u64,
    pub editor: EditorType,
    /// Whole area, including its type selector and split/close controls.
    pub area_rect: Rect,
    pub rect: Rect,
}
#[derive(Default)]
pub struct DockResponse {
    pub panes: Vec<PaneRect>,
    pub activated_pane: Option<u64>,
    pub changed: bool,
    pub split_buttons: BTreeMap<(u64, &'static str), Rect>,
    pub choices: BTreeMap<u64, Rect>,
    pub options: BTreeMap<(u64, &'static str), Rect>,
    pub dividers: BTreeMap<u64, Rect>,
    pub divider_input: DividerInput,
    pub area_menus: BTreeMap<u64, Rect>,
    pub detach_panes: Vec<u64>,
    pub drag_pane: Option<(u64, Rect, Pos2)>,
    pub preview_keep: Vec<u64>,
}
enum Action {
    Split(u64, Axis),
    Close(u64),
    Choose(u64, EditorType),
    Detach(u64),
}
struct DrawOptions<'a> {
    available: &'a [EditorDescriptor],
    active_pane: Option<u64>,
    closable: bool,
    neighbors: BTreeMap<u64, Vec<(u64, EditorType)>>,
    docked: Vec<(u64, EditorType)>,
}

#[derive(Default)]
pub struct DividerInput {
    widgets: Vec<(egui::Id, Rect)>,
    header_controls: Vec<Rect>,
}
impl DividerInput {
    /// Finish after pane content. The full eight-point grab band wins ties
    /// against a ready Scene canvas. Actual header buttons keep their own input.
    pub fn finish(&self, ui: &mut egui::Ui) {
        for (id, rect) in &self.widgets {
            // Static exclusions: hit-testing uses the previous pass, so a
            // current-pointer exception would miss a batched move-and-click.
            let mut segments = vec![*rect];
            for control in &self.header_controls {
                let cut = control.expand(0.01);
                segments = segments
                    .into_iter()
                    .flat_map(|rect| {
                        let overlap = rect.intersect(cut);
                        if !overlap.is_positive() {
                            return vec![rect];
                        }
                        [
                            Rect::from_min_max(rect.min, Pos2::new(overlap.left(), rect.bottom())),
                            Rect::from_min_max(Pos2::new(overlap.right(), rect.top()), rect.max),
                            Rect::from_min_max(
                                Pos2::new(overlap.left(), rect.top()),
                                overlap.right_top(),
                            ),
                            Rect::from_min_max(
                                overlap.left_bottom(),
                                Pos2::new(overlap.right(), rect.bottom()),
                            ),
                        ]
                        .into_iter()
                        .filter(|r| r.is_positive())
                        .collect()
                    })
                    .collect();
            }
            let ids: Vec<_> = segments
                .iter()
                .enumerate()
                .map(|(index, rect)| {
                    let segment_id = id.with(index);
                    ui.interact(*rect, segment_id, Sense::click_and_drag());
                    segment_id
                })
                .collect();
            ui.ctx()
                .data_mut(|data| data.insert_temp(id.with("segments"), ids));
        }
    }

    fn read_response(ctx: &egui::Context, id: egui::Id) -> Option<egui::Response> {
        let ids = ctx
            .data(|data| data.get_temp::<Vec<egui::Id>>(id.with("segments")))
            .unwrap_or_default();
        let responses: Vec<_> = ids
            .into_iter()
            .filter_map(|id| ctx.read_response(id))
            .collect();
        responses
            .iter()
            .find(|r| {
                r.dragged() || r.hovered() || r.secondary_clicked() || r.context_menu_opened()
            })
            .cloned()
            .or_else(|| responses.into_iter().next())
    }
}

#[derive(Clone, Copy)]
struct HeaderPress {
    start: Pos2,
    moved: bool,
    frame: u64,
}

pub(super) struct AreaHeaderResponse {
    pub choice: Rect,
    pub options: Vec<(EditorType, Rect)>,
    pub menu: Rect,
    pub chosen: Option<EditorType>,
    pub drag: Option<Pos2>,
}

/// One header anatomy for docked and native areas. Only the free space moves.
pub(super) fn area_header(
    ui: &mut egui::Ui,
    header: Rect,
    id: u64,
    editor: EditorType,
    available: &[EditorDescriptor],
    active: bool,
    menu: impl FnOnce(&mut egui::Ui),
) -> AreaHeaderResponse {
    ui.painter().rect_filled(
        header,
        0.,
        if active {
            Color32::from_rgb(36, 61, 69)
        } else {
            Color32::from_rgb(30, 38, 52)
        },
    );
    let mut result = AreaHeaderResponse {
        choice: Rect::NOTHING,
        options: vec![],
        menu: Rect::NOTHING,
        chosen: None,
        drag: None,
    };
    // The menu occupies the rightmost 28-point control slot. Only the middle
    // gap owns the raw move gesture; selector and menu presses are excluded.
    let menu_rect = Rect::from_min_size(
        Pos2::new(
            header.right() - COMPACT_CONTROL_SIZE - CHROME_PAD as f32,
            header.top() + CHROME_PAD as f32,
        ),
        Vec2::splat(COMPACT_CONTROL_SIZE),
    );
    let mut selector_end = header.left();
    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt(("area-header", id))
            .max_rect(Rect::from_min_max(
                header.min + Vec2::new(1., 1.),
                Pos2::new(menu_rect.left() - 2., header.bottom() - 1.),
            )),
        |ui| {
            let choice = egui::ComboBox::from_id_salt("editor-type")
                .width((header.width() - 80.).clamp(40., 180.))
                .selected_text(editor.label())
                .show_ui(ui, |ui| {
                    for descriptor in available {
                        let option =
                            ui.selectable_label(editor == descriptor.editor, &descriptor.label);
                        result.options.push((descriptor.editor, option.rect));
                        if option.clicked() {
                            result.chosen = Some(descriptor.editor);
                        }
                    }
                });
            result.choice = choice.response.rect;
            selector_end = choice.response.rect.right();
        },
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt(("area-menu", id))
            .max_rect(menu_rect),
        |ui| {
            let (response, _) = egui::containers::menu::MenuButton::from_button(
                egui::Button::new("").min_size(Vec2::splat(COMPACT_CONTROL_SIZE)),
            )
            .ui(ui, menu);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Area")
            });
            response.clone().on_hover_text("Area");
            super::editor_ui::paint_icon(
                ui.painter(),
                response.rect.shrink(4.),
                super::editor_ui::Icon::More,
                super::editor_ui::TEXT,
            );
            result.menu = response.rect;
        },
    );
    let grab = Rect::from_min_max(
        Pos2::new(selector_end + 4., header.top() + 4.),
        Pos2::new(menu_rect.left() - 4., header.bottom() - 3.),
    );
    if grab.is_positive() {
        let drag = ui.interact(
            grab,
            ui.id().with(("area-move", id)),
            Sense::click_and_drag(),
        );
        drag.clone()
            .on_hover_cursor(CursorIcon::Grab)
            .on_hover_text("Drag to move this area into a window or dock it beside another area");
        result.drag = header_move_start(ui, grab, drag.id);
    }
    result
}

/// egui hit-tests a raw event batch at its final pointer position. A fast
/// header-to-another-area move therefore needs its actual press position.
pub(super) fn header_move_start(ui: &egui::Ui, rect: Rect, id: egui::Id) -> Option<Pos2> {
    let key = id.with(("header-press", ui.ctx().viewport_id()));
    let mut press = ui.ctx().data(|data| data.get_temp::<HeaderPress>(key));
    let frame = ui.ctx().cumulative_frame_nr();
    if press.is_some_and(|press| frame.saturating_sub(press.frame) > 1) {
        press = None;
    }
    let threshold = ui
        .ctx()
        .options(|options| options.input_options.max_click_dist);
    let mut started = None;
    let mut completed = false;
    let (events, held, focused) = ui.input(|input| {
        (
            input.events.clone(),
            input.pointer.primary_down(),
            input.focused,
        )
    });
    for event in &events {
        match event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } => {
                press = (ui.is_enabled()
                    && rect.contains(*pos)
                    && !egui::Popup::is_any_open(ui.ctx())
                    && ui.ctx().layer_id_at(*pos) == Some(ui.layer_id()))
                .then_some(HeaderPress {
                    start: *pos,
                    moved: false,
                    frame,
                });
            }
            egui::Event::PointerMoved(pos)
            | egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                ..
            } => {
                if let Some(candidate) = &mut press
                    && !candidate.moved
                    && candidate.start.distance(*pos) > threshold
                {
                    candidate.moved = true;
                    started = Some(candidate.start);
                }
                if matches!(event, egui::Event::PointerButton { pressed: false, .. }) {
                    completed = press.is_some();
                    press = None;
                }
            }
            egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                ..
            } => {
                press = None;
                started = None;
            }
            egui::Event::WindowFocused(false) => {
                press = None;
                started = None;
                completed = false;
            }
            egui::Event::PointerGone => {
                press = None;
            }
            _ => {}
        }
    }
    if !focused || (!held && !completed) {
        press = None;
        started = None;
    }
    if let Some(press) = &mut press {
        press.frame = frame;
    }
    ui.ctx().data_mut(|data| {
        if let Some(press) = press {
            data.insert_temp(key, press);
        } else {
            data.remove::<HeaderPress>(key);
        }
    });
    if started.is_some() {
        ui.ctx().set_dragged_id(id);
    }
    started
}
impl DockLayout {
    pub fn single(editor: EditorType) -> Self {
        Self {
            root: Area::Pane { id: 1, editor },
            next_id: 2,
            detached: vec![],
        }
    }
    pub fn default_for(workspace: WorkspaceKind) -> Self {
        if workspace == WorkspaceKind::Project {
            let mut layout = Self::single(EditorType::Scene);
            let hierarchy = layout.split(1, Axis::Horizontal).unwrap();
            layout.choose(hierarchy, EditorType::Hierarchy);
            let inspector = layout.split(hierarchy, Axis::Vertical).unwrap();
            layout.choose(inspector, EditorType::Inspector);
            let assets = layout.split(1, Axis::Vertical).unwrap();
            layout.choose(assets, EditorType::Assets);
            if let Area::Split {
                ratio,
                first,
                second,
                ..
            } = &mut layout.root
            {
                *ratio = 0.72;
                if let Area::Split { ratio, .. } = first.as_mut() {
                    *ratio = 0.60;
                }
                if let Area::Split { ratio, .. } = second.as_mut() {
                    *ratio = 0.36;
                }
            }
            return layout;
        }
        Self::single(EditorType::EngineOverview)
    }
    pub fn panes(&self) -> Vec<(u64, EditorType)> {
        let mut all = self.docked_panes();
        all.extend(self.detached.iter().flat_map(DetachedArea::panes));
        all
    }
    pub fn docked_panes(&self) -> Vec<(u64, EditorType)> {
        self.root.panes()
    }
    pub fn host_for(&self, pane: u64) -> Option<u64> {
        self.detached
            .iter()
            .find(|host| host.panes().iter().any(|(id, _)| *id == pane))
            .map(|host| host.id)
    }
    fn find_mut(&mut self, pane: u64) -> Option<&mut Area> {
        if self.root.panes().iter().any(|(id, _)| *id == pane) {
            return self.root.find_mut(pane);
        }
        self.detached
            .iter_mut()
            .find(|host| host.panes().iter().any(|(id, _)| *id == pane))?
            .tree_mut()
            .find_mut(pane)
    }
    pub fn close_host(&mut self, host: u64) -> Vec<u64> {
        if let Some(index) = self.detached.iter().position(|area| area.id == host) {
            self.detached
                .remove(index)
                .panes()
                .into_iter()
                .map(|(id, _)| id)
                .collect()
        } else {
            vec![]
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        fn walk(area: &Area, depth: usize, ids: &mut BTreeSet<u64>) -> Result<(), String> {
            if depth > 12 || ids.len() > 64 {
                return Err("Personal layout exceeds area/depth limits".into());
            }
            let id = match area {
                Area::Pane { id, .. } | Area::Split { id, .. } => *id,
            };
            if id == 0 || !ids.insert(id) {
                return Err("Personal layout area IDs must be unique and nonzero".into());
            }
            if let Area::Split {
                ratio,
                first,
                second,
                ..
            } = area
            {
                if !ratio.is_finite() || !(0.08..=0.92).contains(ratio) {
                    return Err("Personal split ratio is invalid".into());
                }
                walk(first, depth + 1, ids)?;
                walk(second, depth + 1, ids)?;
            }
            Ok(())
        }
        let mut ids = BTreeSet::new();
        walk(&self.root, 0, &mut ids)?;
        let mut hosts = BTreeSet::new();
        for pane in &self.detached {
            let mut host_ids = BTreeSet::new();
            if let Some(tree) = &pane.tree {
                walk(tree, 0, &mut host_ids)?;
            } else {
                host_ids.insert(pane.id);
            }
            // Native host IDs are a separate identity namespace: their original
            // leaf can move to another host while this HWND remains alive.
            if pane.id == 0
                || !hosts.insert(pane.id)
                || host_ids.iter().any(|id| !ids.insert(*id))
                || !pane
                    .size
                    .iter()
                    .all(|v| v.is_finite() && (160.0..=16384.0).contains(v))
                || pane
                    .position
                    .is_some_and(|pos| pos.iter().any(|v| !v.is_finite() || v.abs() > 100000.0))
            {
                return Err("Invalid detached area or placement".into());
            }
        }
        if self.panes().len() > 24 {
            return Err("At most 24 areas per workspace".into());
        }
        if self.next_id
            <= ids
                .last()
                .copied()
                .unwrap_or(0)
                .max(hosts.last().copied().unwrap_or(0))
            || self.next_id == u64::MAX
        {
            return Err("Personal layout ID sequence is invalid".into());
        }
        Ok(())
    }
    pub fn split(&mut self, pane: u64, axis: Axis) -> Result<u64, String> {
        if self.panes().len() >= 24 {
            return Err("At most 24 areas per workspace".into());
        }
        // Build first, then validate. A deep user layout must not be made
        // unsaveable by splitting its deepest leaf or exhausting its IDs.
        let mut candidate = self.clone();
        let split = candidate.next_id;
        let target = candidate.find_mut(pane).ok_or("Area not found")?;
        let editor = match target {
            Area::Pane { editor, .. } => *editor,
            _ => unreachable!(),
        };
        let added = split.checked_add(1).ok_or("Area ID exhausted")?;
        *target = Area::Split {
            id: split,
            axis,
            ratio: 0.5,
            first: Box::new(Area::Pane { id: pane, editor }),
            second: Box::new(Area::Pane { id: added, editor }),
        };
        candidate.next_id = added.checked_add(1).ok_or("Area ID exhausted")?;
        candidate.validate()?;
        *self = candidate;
        Ok(added)
    }
    pub fn choose(&mut self, pane: u64, editor: EditorType) -> bool {
        if let Some(Area::Pane {
            editor: current, ..
        }) = self.find_mut(pane)
        {
            let changed = *current != editor;
            *current = editor;
            changed
        } else {
            false
        }
    }
    pub fn close(&mut self, pane: u64) -> bool {
        fn walk(area: &mut Area, pane: u64) -> bool {
            let Area::Split { first, second, .. } = area else {
                return false;
            };
            if matches!(first.as_ref(),Area::Pane{id,..} if *id==pane) {
                *area = *second.clone();
                return true;
            }
            if matches!(second.as_ref(),Area::Pane{id,..} if *id==pane) {
                *area = *first.clone();
                return true;
            }
            walk(first, pane) || walk(second, pane)
        }
        if let Some(index) = self
            .detached
            .iter()
            .position(|host| host.panes().iter().any(|(id, _)| *id == pane))
        {
            if self.detached[index].panes().len() == 1 {
                self.detached.remove(index);
                true
            } else {
                walk(self.detached[index].tree_mut(), pane)
            }
        } else {
            walk(&mut self.root, pane)
        }
    }
    fn neighbor(&self, pane: u64) -> Option<(u64, Axis, bool)> {
        fn walk(area: &Area, pane: u64) -> Option<(u64, Axis, bool)> {
            match area {
                Area::Split {
                    axis,
                    first,
                    second,
                    ..
                } => {
                    if matches!(first.as_ref(),Area::Pane{id,..} if *id==pane) {
                        return Some((second.first_pane_id(), *axis, true));
                    }
                    if matches!(second.as_ref(),Area::Pane{id,..} if *id==pane) {
                        return Some((first.first_pane_id(), *axis, false));
                    }
                    walk(first, pane).or_else(|| walk(second, pane))
                }
                _ => None,
            }
        }
        walk(&self.root, pane).or_else(|| {
            self.detached
                .iter()
                .filter_map(|host| host.tree.as_ref())
                .find_map(|tree| walk(tree, pane))
        })
    }
    fn neighbor_groups(&self) -> BTreeMap<u64, Vec<(u64, EditorType)>> {
        fn leaves(area: &Area) -> Vec<(u64, EditorType)> {
            match area {
                Area::Pane { id, editor } => vec![(*id, *editor)],
                Area::Split { first, second, .. } => {
                    let mut a = leaves(first);
                    a.extend(leaves(second));
                    a
                }
            }
        }
        fn walk(area: &Area, result: &mut BTreeMap<u64, Vec<(u64, EditorType)>>) {
            if let Area::Split { first, second, .. } = area {
                if let Area::Pane { id, .. } = first.as_ref() {
                    result.insert(*id, leaves(second));
                }
                if let Area::Pane { id, .. } = second.as_ref() {
                    result.insert(*id, leaves(first));
                }
                walk(first, result);
                walk(second, result);
            }
        }
        let mut result = BTreeMap::new();
        walk(&self.root, &mut result);
        for tree in self.detached.iter().filter_map(|host| host.tree.as_ref()) {
            walk(tree, &mut result);
        }
        result
    }
    pub fn detach(&mut self, pane: u64) -> Result<(), String> {
        let editor = self
            .panes()
            .into_iter()
            .find(|(id, _)| *id == pane)
            .ok_or("Area is missing")?
            .1;
        let source_host = self
            .detached
            .iter()
            .find(|host| host.panes().iter().any(|(id, _)| *id == pane))
            .cloned();
        let mut candidate = self.clone();
        let (neighbor, axis, before, fallback) =
            if let Some((neighbor, axis, before)) = candidate.neighbor(pane) {
                candidate.close(pane);
                (neighbor, axis, before, None)
            } else if source_host.is_some() {
                candidate.close(pane);
                (
                    candidate.root.first_pane_id(),
                    Axis::Horizontal,
                    false,
                    None,
                )
            } else {
                let id = candidate.next_id;
                candidate.next_id = id.checked_add(1).ok_or("Area IDs exhausted")?;
                candidate.root = Area::Pane {
                    id,
                    editor: EditorType::Scene,
                };
                (id, Axis::Horizontal, false, Some(id))
            };
        let host = if source_host.is_some() || candidate.detached.iter().any(|host| host.id == pane)
        {
            let id = candidate.next_id;
            candidate.next_id = id.checked_add(1).ok_or("Area IDs exhausted")?;
            id
        } else {
            pane
        };
        candidate.detached.push(DetachedArea {
            id: host,
            editor,
            tree: (host != pane).then_some(Area::Pane { id: pane, editor }),
            neighbor,
            axis,
            before,
            fallback,
            position: source_host
                .as_ref()
                .filter(|host| host.panes().len() == 1)
                .and_then(|host| host.position),
            size: source_host
                .as_ref()
                .filter(|host| host.panes().len() == 1)
                .map_or([800., 600.], |host| host.size),
        });
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
    /// Insert an existing detached identity at the explicitly previewed side.
    /// A failed candidate leaves the original layout and detached window intact.
    pub fn dock_at(
        &mut self,
        pane: u64,
        target: u64,
        axis: Axis,
        before: bool,
    ) -> Result<(), String> {
        if pane == target {
            return Err("Cannot dock an area into itself".into());
        }
        let mut candidate = self.clone();
        let editor = candidate
            .panes()
            .into_iter()
            .find(|(id, _)| *id == pane)
            .ok_or("Moved area is missing")?
            .1;
        if !candidate.close(pane) {
            return Err("Moved area cannot be removed".into());
        }
        let split = candidate.next_id;
        candidate.next_id = split.checked_add(1).ok_or("Area IDs exhausted")?;
        let destination = candidate.find_mut(target).ok_or("Dock target is missing")?;
        let moved = Area::Pane { id: pane, editor };
        let old = destination.clone();
        let (first, second) = if before { (moved, old) } else { (old, moved) };
        *destination = Area::Split {
            id: split,
            axis,
            ratio: 0.5,
            first: Box::new(first),
            second: Box::new(second),
        };
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
    pub fn return_area(&mut self, pane: u64) -> Result<(), String> {
        let mut candidate = self.clone();
        let index = candidate
            .detached
            .iter()
            .position(|p| p.id == pane)
            .ok_or("Detached area is missing")?;
        let returning = candidate.detached.remove(index);
        let target = if candidate
            .docked_panes()
            .iter()
            .any(|(id, _)| *id == returning.neighbor)
        {
            returning.neighbor
        } else {
            candidate.root.first_pane_id()
        };
        fn find(area: &mut Area, id: u64) -> Option<&mut Area> {
            match area {
                Area::Pane { id: found, .. } if *found == id => Some(area),
                Area::Split { first, second, .. } => find(first, id).or_else(|| find(second, id)),
                _ => None,
            }
        }
        let split = candidate.next_id;
        candidate.next_id = split.checked_add(1).ok_or("Area IDs exhausted")?;
        let destination = find(&mut candidate.root, target).ok_or("Return area is missing")?;
        let moved = returning.tree.clone().unwrap_or(Area::Pane {
            id: returning.id,
            editor: returning.editor,
        });
        if returning.fallback == Some(target)
            && matches!(
                destination,
                Area::Pane {
                    editor: EditorType::Scene,
                    ..
                }
            )
        {
            *destination = moved;
        } else {
            let old = destination.clone();
            let (first, second) = if returning.before {
                (moved, old)
            } else {
                (old, moved)
            };
            *destination = Area::Split {
                id: split,
                axis: returning.axis,
                ratio: 0.5,
                first: Box::new(first),
                second: Box::new(second),
            };
        }
        // A changed/deep return neighborhood must never strand a native window.
        if candidate.validate().is_err() {
            candidate = self.clone();
            candidate.detached.remove(index);
            fn shallowest(area: &Area, depth: usize) -> (usize, u64) {
                match area {
                    Area::Pane { id, .. } => (depth, *id),
                    Area::Split { first, second, .. } => {
                        let a = shallowest(first, depth + 1);
                        let b = shallowest(second, depth + 1);
                        a.min(b)
                    }
                }
            }
            let (_, safe) = shallowest(&candidate.root, 0);
            let split = candidate.next_id;
            candidate.next_id = split.checked_add(1).ok_or("Area IDs exhausted")?;
            let destination = find(&mut candidate.root, safe).ok_or("No safe main area")?;
            let old = destination.clone();
            *destination = Area::Split {
                id: split,
                axis: Axis::Horizontal,
                ratio: 0.5,
                first: Box::new(old),
                second: Box::new(returning.tree.clone().unwrap_or(Area::Pane {
                    id: returning.id,
                    editor: returning.editor,
                })),
            };
            candidate.validate()?;
        }
        *self = candidate;
        Ok(())
    }
    fn area_menu(
        ui: &mut egui::Ui,
        pane: u64,
        options: &DrawOptions<'_>,
        response: &mut DockResponse,
        actions: &mut Vec<Action>,
    ) {
        for (label, key, axis) in [
            ("Split horizontally", "H", Axis::Horizontal),
            ("Split vertically", "V", Axis::Vertical),
        ] {
            let item = ui.button(label);
            response.split_buttons.insert((pane, key), item.rect);
            if item.clicked() {
                actions.push(Action::Split(pane, axis));
                ui.close();
            }
        }
        let item = ui
            .add_enabled(
                options.closable,
                egui::Button::new("Close area · keep neighboring area"),
            )
            .on_hover_text("Closes only this area; neighboring areas keep their identities.")
            .on_disabled_hover_text("The final main area is retained as a usable fallback.");
        response.split_buttons.insert((pane, "close"), item.rect);
        if let Some(kept) = options.neighbors.get(&pane) {
            ui.small(format!(
                "Keeps: {}",
                kept.iter()
                    .map(|(id, kind)| format!("{} · {id}", kind.label()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if item.hovered() {
                response.preview_keep.extend(kept.iter().map(|(id, _)| *id));
            }
        }
        if item.clicked() {
            actions.push(Action::Close(pane));
            ui.close();
        }
        ui.separator();
        if ui
            .add_enabled(
                !ui.ctx().embed_viewports(),
                egui::Button::new("Move to new window"),
            )
            .on_disabled_hover_text("Separate OS windows are unavailable in this host.")
            .clicked()
        {
            actions.push(Action::Detach(pane));
            ui.close();
        }
    }
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        available: &[EditorDescriptor],
        active_pane: Option<u64>,
    ) -> DockResponse {
        self.draw_host(ui, available, active_pane, None)
    }
    pub fn draw_host(
        &mut self,
        ui: &mut egui::Ui,
        available: &[EditorDescriptor],
        active_pane: Option<u64>,
        host: Option<u64>,
    ) -> DockResponse {
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, Sense::hover());
        let mut response = DockResponse::default();
        let mut actions = vec![];
        let panes = host.map_or_else(
            || self.docked_panes(),
            |id| {
                self.detached
                    .iter()
                    .find(|area| area.id == id)
                    .map_or_else(Vec::new, DetachedArea::panes)
            },
        );
        if panes.is_empty() {
            return response;
        }
        let options = DrawOptions {
            available,
            active_pane,
            closable: host.is_some() || panes.len() > 1,
            neighbors: self.neighbor_groups(),
            docked: panes,
        };
        fn draw(
            area: &mut Area,
            rect: Rect,
            ui: &mut egui::Ui,
            options: &DrawOptions<'_>,
            response: &mut DockResponse,
            actions: &mut Vec<Action>,
        ) {
            match area {
                Area::Pane { id, editor } => {
                    if let Some(pointer) = ui.input(|i| {
                        i.pointer
                            .any_pressed()
                            .then(|| i.pointer.interact_pos())
                            .flatten()
                    }) && rect.contains(pointer)
                        && ui.ctx().layer_id_at(pointer) == Some(ui.layer_id())
                    {
                        response.activated_pane = Some(*id);
                    }
                    let header = Rect::from_min_max(
                        rect.min,
                        Pos2::new(
                            rect.right(),
                            (rect.top() + AREA_HEADER_HEIGHT).min(rect.bottom()),
                        ),
                    );
                    ui.painter()
                        .rect_filled(rect, 0.0, Color32::from_rgb(18, 24, 34));
                    ui.painter().rect_filled(
                        header,
                        0.0,
                        if options.active_pane == Some(*id) {
                            Color32::from_rgb(36, 61, 69)
                        } else {
                            Color32::from_rgb(30, 38, 52)
                        },
                    );
                    if options.active_pane == Some(*id) {
                        ui.painter().rect_stroke(
                            rect.shrink(0.5),
                            0.0,
                            egui::Stroke::new(1.0, Color32::from_rgb(67, 168, 156)),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let header_response = area_header(
                        ui,
                        header,
                        *id,
                        *editor,
                        options.available,
                        options.active_pane == Some(*id),
                        |ui| {
                            DockLayout::area_menu(ui, *id, options, response, actions);
                            if options.docked.len() > 1 {
                                ui.separator();
                                ui.menu_button("Focus area", |ui| {
                                    for (target, kind) in &options.docked {
                                        if ui
                                            .selectable_label(
                                                *target == *id,
                                                format!("{} · {target}", kind.label()),
                                            )
                                            .clicked()
                                        {
                                            response.activated_pane = Some(*target);
                                            ui.close();
                                        }
                                    }
                                });
                            }
                        },
                    );
                    response.choices.insert(*id, header_response.choice);
                    response.area_menus.insert(*id, header_response.menu);
                    response
                        .divider_input
                        .header_controls
                        .extend([header_response.choice, header_response.menu]);
                    for (kind, rect) in header_response.options {
                        response.options.insert((*id, kind.label()), rect);
                    }
                    if let Some(chosen) = header_response.chosen {
                        actions.push(Action::Choose(*id, chosen));
                    }
                    if let Some(start) = header_response.drag {
                        response.drag_pane = Some((*id, rect, start));
                    }
                    let body = Rect::from_min_max(
                        Pos2::new(
                            rect.left() + CHROME_PAD as f32,
                            (header.bottom() + CHROME_PAD as f32).min(rect.bottom()),
                        ),
                        Pos2::new(
                            (rect.right() - CHROME_PAD as f32).max(rect.left() + CHROME_PAD as f32),
                            (rect.bottom() - CHROME_PAD as f32)
                                .max(header.bottom() + CHROME_PAD as f32),
                        ),
                    );
                    response.panes.push(PaneRect {
                        id: *id,
                        editor: *editor,
                        area_rect: rect,
                        rect: body.intersect(rect),
                    });
                }
                Area::Split {
                    id,
                    axis,
                    ratio,
                    first,
                    second,
                } => {
                    let span = if *axis == Axis::Horizontal {
                        rect.width()
                    } else {
                        rect.height()
                    };
                    let boundary = if *axis == Axis::Horizontal {
                        rect.left() + span * *ratio
                    } else {
                        rect.top() + span * *ratio
                    };
                    let divider = if *axis == Axis::Horizontal {
                        Rect::from_min_max(
                            Pos2::new(boundary - 4.0, rect.top()),
                            Pos2::new(boundary + 4.0, rect.bottom()),
                        )
                    } else {
                        Rect::from_min_max(
                            Pos2::new(rect.left(), boundary - 4.0),
                            Pos2::new(rect.right(), boundary + 4.0),
                        )
                    };
                    let divider_id = ui.make_persistent_id(("area-divider", *id));
                    // Read last pass's hit result without registering early:
                    // the actual widget is added once, after all pane content.
                    let gesture = ui
                        .is_enabled()
                        .then(|| DividerInput::read_response(ui.ctx(), divider_id))
                        .flatten()
                        .map(|response| {
                            response.on_hover_cursor(if *axis == Axis::Horizontal {
                                CursorIcon::ResizeHorizontal
                            } else {
                                CursorIcon::ResizeVertical
                            })
                        });
                    if gesture.as_ref().is_some_and(|g| g.dragged())
                        && let Some(pointer) =
                            gesture.as_ref().and_then(|g| g.interact_pointer_pos())
                    {
                        let offset = if *axis == Axis::Horizontal {
                            pointer.x - rect.left()
                        } else {
                            pointer.y - rect.top()
                        };
                        let minimum = (70.0 / span.max(1.0)).clamp(0.08, 0.45);
                        let candidate = (offset / span.max(1.0)).clamp(minimum, 1.0 - minimum);
                        if candidate != *ratio {
                            *ratio = candidate;
                            response.changed = true;
                        }
                    }
                    let updated_boundary = if *axis == Axis::Horizontal {
                        rect.left() + span * *ratio
                    } else {
                        rect.top() + span * *ratio
                    };
                    let divider = divider.translate(if *axis == Axis::Horizontal {
                        Vec2::new(updated_boundary - boundary, 0.)
                    } else {
                        Vec2::new(0., updated_boundary - boundary)
                    });
                    let boundary = updated_boundary;
                    response.dividers.insert(*id, divider);
                    response.divider_input.widgets.push((divider_id, divider));
                    let (a, b) = if *axis == Axis::Horizontal {
                        (
                            Rect::from_min_max(rect.min, Pos2::new(boundary - 1.0, rect.bottom())),
                            Rect::from_min_max(Pos2::new(boundary + 1.0, rect.top()), rect.max),
                        )
                    } else {
                        (
                            Rect::from_min_max(rect.min, Pos2::new(rect.right(), boundary - 1.0)),
                            Rect::from_min_max(Pos2::new(rect.left(), boundary + 1.0), rect.max),
                        )
                    };
                    draw(first, a, ui, options, response, actions);
                    draw(second, b, ui, options, response, actions);
                    let key = ui.make_persistent_id(("border-target", *id));
                    if gesture.as_ref().is_some_and(|g| g.secondary_clicked()) {
                        let point = gesture
                            .as_ref()
                            .and_then(|g| g.interact_pointer_pos())
                            .unwrap_or(divider.center());
                        let target = response
                            .panes
                            .iter()
                            .filter(|p| rect.contains(p.area_rect.center()))
                            .min_by(|a, b| {
                                a.area_rect
                                    .distance_sq_to_pos(point)
                                    .total_cmp(&b.area_rect.distance_sq_to_pos(point))
                                    .then_with(|| {
                                        (a.id != options.active_pane.unwrap_or(0))
                                            .cmp(&(b.id != options.active_pane.unwrap_or(0)))
                                    })
                            })
                            .map(|p| p.id)
                            .unwrap_or_else(|| first.first_pane_id());
                        ui.ctx().data_mut(|d| d.insert_temp(key, target));
                    }
                    let target = ui
                        .ctx()
                        .data(|d| d.get_temp::<u64>(key))
                        .unwrap_or_else(|| first.first_pane_id());
                    if let Some(gesture) = &gesture {
                        gesture.context_menu(|ui| {
                            ui.label(format!("Area {target}"));
                            DockLayout::area_menu(ui, target, options, response, actions);
                        });
                    }
                    let line = if *axis == Axis::Horizontal {
                        [
                            Pos2::new(boundary, rect.top()),
                            Pos2::new(boundary, rect.bottom()),
                        ]
                    } else {
                        [
                            Pos2::new(rect.left(), boundary),
                            Pos2::new(rect.right(), boundary),
                        ]
                    };
                    ui.painter().line_segment(
                        line,
                        egui::Stroke::new(
                            if gesture.as_ref().is_some_and(|g| g.hovered()) {
                                2.
                            } else {
                                1.
                            },
                            Color32::from_rgb(70, 100, 120),
                        ),
                    );
                }
            }
        }
        let tree = if let Some(id) = host {
            self.detached
                .iter_mut()
                .find(|area| area.id == id)
                .unwrap()
                .tree_mut()
        } else {
            &mut self.root
        };
        if rect.width() < 480. || rect.height() < 240. {
            let id = active_pane
                .filter(|id| options.docked.iter().any(|(p, _)| p == id))
                .unwrap_or_else(|| tree.first_pane_id());
            let editor = options.docked.iter().find(|(p, _)| *p == id).unwrap().1;
            draw(
                &mut Area::Pane { id, editor },
                rect,
                ui,
                &options,
                &mut response,
                &mut actions,
            );
        } else {
            draw(tree, rect, ui, &options, &mut response, &mut actions);
        }
        for pane in &response.panes {
            if response.preview_keep.contains(&pane.id) {
                ui.painter().rect_stroke(
                    pane.area_rect.shrink(2.),
                    0.,
                    egui::Stroke::new(2., Color32::from_rgb(111, 214, 198)),
                    egui::StrokeKind::Inside,
                );
            }
        }
        for action in actions {
            response.changed |= match action {
                Action::Split(id, axis) => match self.split(id, axis) {
                    Ok(added) => {
                        response.activated_pane = Some(added);
                        true
                    }
                    Err(_) => false,
                },
                Action::Close(id) => self.close(id),
                Action::Choose(id, editor) => {
                    response.activated_pane = Some(id);
                    self.choose(id, editor)
                }
                Action::Detach(id) => {
                    response.detach_panes.push(id);
                    false
                }
            };
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn divider_stops_immediately_when_parent_is_disabled_mid_drag() {
        for host in [None, Some(1)] {
            let ctx = egui::Context::default();
            let mut layout = DockLayout::single(EditorType::Scene);
            if host.is_some() {
                layout.detach(1).unwrap();
            }
            layout.split(1, Axis::Horizontal).unwrap();
            let mut step = 0;
            let mut draw = |events: Vec<egui::Event>, enabled: bool, layout: &mut DockLayout| {
                let mut result = None;
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 700.))),
                        time: Some(step as f64 * 0.03),
                        events,
                        focused: true,
                        ..Default::default()
                    },
                    |ui| {
                        if !enabled {
                            ui.disable();
                        }
                        let response =
                            layout.draw_host(ui, &EditorDescriptor::builtins(true), Some(1), host);
                        response.divider_input.finish(ui);
                        result = Some(response);
                    },
                )
                .drop_without_applying_deltas();
                step += 1;
                result.unwrap()
            };
            draw(vec![], true, &mut layout);
            let state = draw(vec![], true, &mut layout);
            let start = state.dividers.values().next().unwrap().center();
            draw(
                vec![
                    egui::Event::PointerMoved(start),
                    egui::Event::PointerButton {
                        pos: start,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
                true,
                &mut layout,
            );
            assert!(
                draw(
                    vec![egui::Event::PointerMoved(start + Vec2::new(15., 0.))],
                    true,
                    &mut layout
                )
                .changed
            );
            let before = layout.clone();
            let disabled = draw(
                vec![egui::Event::PointerMoved(start + Vec2::new(60., 0.))],
                false,
                &mut layout,
            );
            assert!(
                !disabled.changed && layout == before,
                "disabled parent resized: host={host:?}"
            );
            assert!(!egui::Popup::is_any_open(&ctx));
        }
    }
    #[test]
    fn thin_gutter_keeps_full_grab_band_beside_ready_scene_and_header_buttons() {
        use crate::scene_view_ui::SceneViewportState;
        use module_engine_core::{Frame, SceneSnapshot};
        for host in [None, Some(1)] {
            for axis in [Axis::Horizontal, Axis::Vertical] {
                for edge in [-3., 3.] {
                    let ctx = egui::Context::default();
                    crate::app::editor_ui::configure(&ctx);
                    let mut layout = DockLayout::single(EditorType::Scene);
                    if host.is_some() {
                        layout.detach(1).unwrap();
                    }
                    layout.split(1, axis).unwrap();
                    let mut scenes = [SceneViewportState::default(), SceneViewportState::default()];
                    let mut step = 0;
                    let mut draw = |events: Vec<egui::Event>| {
                        let mut result = None;
                        ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(Rect::from_min_size(
                                    Pos2::ZERO,
                                    Vec2::new(1000., 700.),
                                )),
                                time: Some(step as f64 * 0.03),
                                focused: true,
                                events,
                                ..Default::default()
                            },
                            |ui| {
                                let response = layout.draw_host(
                                    ui,
                                    &EditorDescriptor::builtins(true),
                                    Some(1),
                                    host,
                                );
                                for (index, pane) in response.panes.iter().enumerate() {
                                    ui.scope_builder(
                                        egui::UiBuilder::new().id_salt(pane.id).max_rect(pane.rect),
                                        |ui| {
                                            let snapshot = SceneSnapshot {
                                                items: vec![],
                                                time: 0.,
                                            };
                                            scenes[index].interact(ui, &[], &snapshot, None, false);
                                            scenes[index].paint(
                                                ui,
                                                &[],
                                                &snapshot,
                                                None,
                                                &mut |_, width, height| {
                                                    Ok(Frame {
                                                        width,
                                                        height,
                                                        rgba: vec![80; width * height * 4],
                                                        gpu: None,
                                                        stats: Default::default(),
                                                    })
                                                },
                                            );
                                        },
                                    );
                                }
                                response.divider_input.finish(ui);
                                result = Some(response);
                            },
                        )
                        .drop_without_applying_deltas();
                        step += 1;
                        result.unwrap()
                    };
                    draw(vec![]);
                    let initial = draw(vec![]);
                    let divider = initial.dividers.values().next().unwrap();
                    let mut start = divider.center();
                    let delta = if axis == Axis::Horizontal {
                        start.x += edge;
                        Vec2::new(60., 0.)
                    } else {
                        start.y += edge;
                        Vec2::new(0., 60.)
                    };
                    let pointer = |p, pressed| egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    };
                    draw(vec![egui::Event::PointerMoved(start)]);
                    draw(vec![pointer(start, true)]);
                    let moved = draw(vec![egui::Event::PointerMoved(start + delta)]);
                    assert!(moved.changed, "host={host:?} axis={axis:?} edge={edge}");
                    draw(vec![pointer(start + delta, false)]);
                    if axis == Axis::Vertical {
                        let settled = draw(vec![]);
                        let lower = settled
                            .panes
                            .iter()
                            .max_by(|a, b| a.area_rect.top().total_cmp(&b.area_rect.top()))
                            .unwrap();
                        let selector = settled.choices[&lower.id];
                        let menu = settled.area_menus[&lower.id];
                        let at = Pos2::new(
                            (selector.right() + menu.left()) * 0.5,
                            lower.area_rect.top() + 3.,
                        );
                        draw(vec![egui::Event::PointerMoved(at), pointer(at, true)]);
                        let moved = draw(vec![egui::Event::PointerMoved(at + Vec2::new(0., 25.))]);
                        assert!(
                            moved.changed && moved.drag_pane.is_none(),
                            "exact divider/header edge must only resize"
                        );
                        draw(vec![pointer(at + Vec2::new(0., 25.), false)]);
                    }
                    // The selector/More button's outer edge may overlap the
                    // grab band; these actual controls must still open.
                    let settled = draw(vec![]);
                    let menu = settled.area_menus[&1];
                    let at = Pos2::new(menu.right() - 1., menu.center().y);
                    draw(vec![
                        egui::Event::PointerMoved(at),
                        pointer(at, true),
                        pointer(at, false),
                    ]);
                    let opened = draw(vec![]);
                    assert!(
                        opened.split_buttons.contains_key(&(1, "H")),
                        "More edge lost: host={host:?} axis={axis:?} edge={edge}"
                    );
                    // Close the menu, then jump directly from canvas to the
                    // selector edge nearest the divider in a single batch.
                    draw(vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }]);
                    let settled = draw(vec![egui::Event::PointerMoved(start + delta)]);
                    let target = if axis == Axis::Horizontal {
                        settled
                            .panes
                            .iter()
                            .max_by(|a, b| a.area_rect.left().total_cmp(&b.area_rect.left()))
                            .unwrap()
                    } else {
                        settled
                            .panes
                            .iter()
                            .max_by(|a, b| a.area_rect.top().total_cmp(&b.area_rect.top()))
                            .unwrap()
                    };
                    let selector = settled.choices[&target.id];
                    let at = if axis == Axis::Horizontal {
                        Pos2::new(selector.left() + 0.5, selector.center().y)
                    } else {
                        Pos2::new(selector.center().x, selector.top() + 0.5)
                    };
                    draw(vec![
                        egui::Event::PointerMoved(at),
                        pointer(at, true),
                        pointer(at, false),
                    ]);
                    let opened = draw(vec![]);
                    assert!(
                        opened.options.keys().any(|(id, _)| *id == target.id),
                        "selector edge lost"
                    );
                }
            }
        }
    }
    #[test]
    fn move_to_new_window_from_singleton_floating_host_is_real_and_preserves_main() {
        let mut layout = DockLayout::single(EditorType::Scene);
        layout.detach(1).unwrap();
        layout.detached[0].position = Some([123., 234.]);
        layout.detached[0].size = [760., 510.];
        let main = layout.root.clone();
        layout.detach(1).unwrap();
        assert_eq!(layout.root, main);
        assert_eq!(layout.detached.len(), 1);
        assert_ne!(layout.host_for(1), Some(1));
        assert_eq!(layout.detached[0].panes(), vec![(1, EditorType::Scene)]);
        assert_eq!(layout.detached[0].position, Some([123., 234.]));
        assert_eq!(layout.detached[0].size, [760., 510.]);
        layout.validate().unwrap();
    }
    #[test]
    fn floating_host_splits_both_axes_and_closes_exact_leaves_across_roundtrip() {
        let mut layout = DockLayout::single(EditorType::Scene);
        layout.detach(1).unwrap();
        let main = layout.root.clone();
        let horizontal = layout.split(1, Axis::Horizontal).unwrap();
        let vertical = layout.split(horizontal, Axis::Vertical).unwrap();
        assert_eq!(layout.root, main, "floating split must not modify main");
        assert_eq!(layout.detached.len(), 1);
        assert_eq!(layout.host_for(vertical), Some(1));
        layout.choose(vertical, EditorType::Assets);
        assert!(layout.close(1));
        assert_eq!(
            layout.detached[0].id, 1,
            "native host identity survives its original leaf"
        );
        let mut restored: DockLayout =
            serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored, layout);
        assert_eq!(
            restored.detached[0].panes(),
            vec![
                (horizontal, EditorType::Scene),
                (vertical, EditorType::Assets)
            ]
        );
        assert!(restored.close(horizontal));
        assert_eq!(restored.host_for(vertical), Some(1));
        assert!(restored.close(vertical));
        assert!(restored.detached.is_empty());
        assert_eq!(restored.root, main);
        assert!(!restored.close(restored.root.first_pane_id()));
    }
    #[test]
    fn legacy_floating_host_moves_leaf_between_hosts_and_system_close_is_host_scoped() {
        let mut layout = DockLayout::single(EditorType::Scene);
        layout.detach(1).unwrap();
        assert!(!serde_json::to_string(&layout).unwrap().contains("tree"));
        let mut layout: DockLayout =
            serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
        let sibling = layout.split(1, Axis::Horizontal).unwrap();
        layout.detach(1).unwrap();
        let moved_host = layout.host_for(1).unwrap();
        assert_ne!(moved_host, 1);
        assert_eq!(layout.host_for(sibling), Some(1));
        layout.validate().unwrap();
        let main = layout.root.clone();
        layout.dock_at(1, sibling, Axis::Vertical, true).unwrap();
        assert_eq!(layout.detached.len(), 1);
        assert_eq!(layout.host_for(1), Some(1));
        assert_eq!(layout.root, main);
        assert_eq!(layout.close_host(moved_host), Vec::<u64>::new());
        assert_eq!(layout.close_host(1), vec![1, sibling]);
        assert_eq!(layout.root, main);
        layout.validate().unwrap();
    }
    #[test]
    fn invalid_floating_tree_is_rejected_atomically() {
        let mut layout = DockLayout::single(EditorType::Scene);
        layout.detach(1).unwrap();
        let main = layout.root.first_pane_id();
        let before = layout.clone();
        assert!(
            layout
                .dock_at(1, u64::MAX, Axis::Horizontal, false)
                .is_err()
        );
        assert_eq!(layout, before);
        layout.detached[0].tree = Some(Area::Pane {
            id: main,
            editor: EditorType::Scene,
        });
        assert!(layout.validate().is_err());
    }
    #[test]
    fn floating_menu_uses_common_commands_and_pointer_split_stays_in_its_host() {
        let ctx = egui::Context::default();
        let mut layout = DockLayout::single(EditorType::Scene);
        layout.detach(1).unwrap();
        let main = layout.root.clone();
        let first = frame_host(&ctx, &mut layout, 0., vec![], Some(1));
        let click_host = |layout: &mut DockLayout, point: Pos2, time| {
            let pressed = frame_host(
                &ctx,
                layout,
                time,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                Some(1),
            );
            let mut released = frame_host(
                &ctx,
                layout,
                time + 0.02,
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                Some(1),
            );
            released.changed |= pressed.changed;
            released
        };
        let initial_menu = click_host(&mut layout, first.area_menus[&1].center(), 0.1);
        let menu = frame_host(&ctx, &mut layout, 0.15, vec![], Some(1));
        eprintln!(
            "Floating popup H bounds: initial={:?}, settled={:?}",
            initial_menu.split_buttons.get(&(1, "H")),
            menu.split_buttons.get(&(1, "H"))
        );
        assert!(menu.split_buttons.contains_key(&(1, "H")));
        assert!(menu.split_buttons.contains_key(&(1, "V")));
        assert!(menu.split_buttons.contains_key(&(1, "close")));
        let split = click_host(&mut layout, menu.split_buttons[&(1, "H")].center(), 0.2);
        assert!(split.changed);
        assert_eq!(layout.root, main);
        assert_eq!(layout.detached.len(), 1);
        assert_eq!(layout.detached[0].panes().len(), 2);
        let drawn = frame_host(&ctx, &mut layout, 0.4, vec![], Some(1));
        assert_eq!(drawn.panes.len(), 2);
        assert_eq!(drawn.dividers.len(), 1);
    }
    #[test]
    fn area_icon_menu_is_right_aligned_and_its_press_cannot_move_the_pane() {
        let ctx = egui::Context::default();
        let mut layout = DockLayout::single(EditorType::Scene);
        let first = frame(&ctx, &mut layout, 0., vec![]);
        let menu = first.area_menus[&1];
        let area = first.panes[0].area_rect;
        assert!((area.right() - menu.right() - CHROME_PAD as f32).abs() < 0.1);
        assert_eq!(menu.size(), Vec2::splat(28.));
        assert!(menu.left() - first.choices[&1].right() > 200.);
        let next = frame(
            &ctx,
            &mut layout,
            0.1,
            vec![
                egui::Event::PointerButton {
                    pos: menu.center(),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerMoved(area.center()),
            ],
        );
        assert!(next.drag_pane.is_none());
    }
    fn frame(
        ctx: &egui::Context,
        layout: &mut DockLayout,
        time: f64,
        events: Vec<egui::Event>,
    ) -> DockResponse {
        frame_host(ctx, layout, time, events, None)
    }
    fn frame_host(
        ctx: &egui::Context,
        layout: &mut DockLayout,
        time: f64,
        events: Vec<egui::Event>,
        host: Option<u64>,
    ) -> DockResponse {
        let mut result = None;
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    if host.is_some() {
                        Vec2::new(760., 510.)
                    } else {
                        Vec2::new(900., 600.)
                    },
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| {
                let response =
                    layout.draw_host(ui, &EditorDescriptor::builtins(true), Some(1), host);
                response.divider_input.finish(ui);
                result = Some(response);
            },
        );
        output.drop_without_applying_deltas();
        result.unwrap()
    }
    fn click(ctx: &egui::Context, layout: &mut DockLayout, point: Pos2, time: f64) -> DockResponse {
        frame(
            ctx,
            layout,
            time,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            ctx,
            layout,
            time + 0.02,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        )
    }
    #[test]
    fn lost_or_hidden_header_press_cannot_turn_later_hover_into_a_move() {
        for hidden in [false, true] {
            let ctx = egui::Context::default();
            let mut layout = DockLayout::single(EditorType::ProjectSettings);
            let first = frame(&ctx, &mut layout, 0., vec![]);
            let point = Pos2::new(
                (first.choices[&1].right() + first.area_menus[&1].left()) * 0.5,
                first.area_menus[&1].center().y,
            );
            frame(
                &ctx,
                &mut layout,
                0.1,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            let release = egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            };
            if hidden {
                ctx.run_ui(
                    egui::RawInput {
                        time: Some(0.2),
                        events: vec![release],
                        ..Default::default()
                    },
                    |_ui| {},
                )
                .drop_without_applying_deltas();
            } else {
                frame(
                    &ctx,
                    &mut layout,
                    0.2,
                    vec![
                        egui::Event::WindowFocused(false),
                        egui::Event::PointerGone,
                        release,
                    ],
                );
            }
            let response = frame(
                &ctx,
                &mut layout,
                0.3,
                vec![
                    egui::Event::WindowFocused(true),
                    egui::Event::PointerMoved(point + Vec2::new(60., 80.)),
                ],
            );
            assert!(response.drag_pane.is_none(), "hidden={hidden}");
        }
    }
    #[test]
    fn header_drag_can_cross_the_area_between_ui_frames() {
        for scenario in [
            "held",
            "released",
            "out-and-back",
            "small",
            "menu",
            "escape",
        ] {
            let ctx = egui::Context::default();
            let mut layout = DockLayout::single(EditorType::ProjectSettings);
            let first = frame(&ctx, &mut layout, 0., vec![]);
            let start = Pos2::new(
                (first.choices[&1].right() + first.area_menus[&1].left()) * 0.5,
                first.area_menus[&1].center().y,
            );
            let end = start + Vec2::new(50., 120.);
            let origin = if scenario == "menu" {
                first.area_menus[&1].center()
            } else {
                start
            };
            let end = if scenario == "small" {
                origin + Vec2::new(1., 0.)
            } else {
                end
            };
            let mut events = vec![
                egui::Event::PointerMoved(origin),
                egui::Event::PointerButton {
                    pos: origin,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerMoved(end),
            ];
            if scenario == "out-and-back" {
                events.push(egui::Event::PointerMoved(start));
            }
            if scenario != "held" && scenario != "escape" {
                events.push(egui::Event::PointerButton {
                    pos: if scenario == "out-and-back" {
                        start
                    } else {
                        end
                    },
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            if scenario == "escape" {
                events.push(egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let response = frame(&ctx, &mut layout, 0.1, events);
            let expected = if ["small", "menu", "escape"].contains(&scenario) {
                None
            } else {
                Some((1, start))
            };
            assert_eq!(
                response.drag_pane.map(|(id, _, point)| (id, point)),
                expected,
                "{scenario}"
            );
        }
    }
    #[test]
    fn free_header_drag_starts_only_after_movement_threshold() {
        let ctx = egui::Context::default();
        let mut layout = DockLayout::single(EditorType::ProjectSettings);
        let first = frame(&ctx, &mut layout, 0., vec![]);
        let point = Pos2::new(
            (first.choices[&1].right() + first.area_menus[&1].left()) * 0.5,
            first.area_menus[&1].center().y,
        );
        let press = egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        };
        let pressed = frame(
            &ctx,
            &mut layout,
            0.1,
            vec![egui::Event::PointerMoved(point), press],
        );
        assert!(pressed.drag_pane.is_none());
        let small = frame(
            &ctx,
            &mut layout,
            0.2,
            vec![egui::Event::PointerMoved(point + Vec2::new(1., 0.))],
        );
        assert!(small.drag_pane.is_none());
        let moved = frame(
            &ctx,
            &mut layout,
            0.3,
            vec![egui::Event::PointerMoved(point + Vec2::new(90., 40.))],
        );
        assert_eq!(moved.drag_pane.map(|(id, _, _)| id), Some(1));
        assert!(!moved.changed);
        assert_eq!(layout, DockLayout::single(EditorType::ProjectSettings));
    }
    #[test]
    fn actual_pointer_split_resize_editor_choice_and_join_preserve_pane_ids() {
        let ctx = egui::Context::default();
        let mut layout = DockLayout::single(EditorType::Viewport);
        let first = frame(&ctx, &mut layout, 0.0, vec![]);
        click(&ctx, &mut layout, first.area_menus[&1].center(), 0.04);
        let menu = frame(&ctx, &mut layout, 0.09, vec![]);
        let button = menu.split_buttons[&(1, "H")].center();
        click(&ctx, &mut layout, button, 0.1);
        let split = frame(&ctx, &mut layout, 0.15, vec![]);
        assert_eq!(layout.panes().len(), 2);
        assert_eq!(layout.panes()[0].0, 1);
        let (divider, rect) = split
            .dividers
            .iter()
            .next()
            .map(|(id, rect)| (*id, *rect))
            .unwrap();
        let start = rect.center();
        frame(
            &ctx,
            &mut layout,
            0.2,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let moved = start + Vec2::new(100.0, 0.0);
        frame(
            &ctx,
            &mut layout,
            0.23,
            vec![egui::Event::PointerMoved(moved)],
        );
        frame(
            &ctx,
            &mut layout,
            0.26,
            vec![egui::Event::PointerButton {
                pos: moved,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(matches!(layout.root,Area::Split{id,ratio,..} if id==divider && ratio>0.6));
        let state = frame(&ctx, &mut layout, 0.3, vec![]);
        click(&ctx, &mut layout, state.choices[&1].center(), 0.32);
        let menu = frame(&ctx, &mut layout, 0.36, vec![]);
        let option = menu.options[&(1, EditorType::ObjectTree.label())].center();
        click(&ctx, &mut layout, option, 0.38);
        assert_eq!(layout.panes()[0], (1, EditorType::ObjectTree));
        let state = frame(&ctx, &mut layout, 0.42, vec![]);
        let other = layout.panes()[1].0;
        click(&ctx, &mut layout, state.area_menus[&other].center(), 0.43);
        let state = frame(&ctx, &mut layout, 0.46, vec![]);
        click(
            &ctx,
            &mut layout,
            state.split_buttons[&(other, "close")].center(),
            0.48,
        );
        assert_eq!(layout.panes(), vec![(1, EditorType::ObjectTree)]);
        let state = frame(&ctx, &mut layout, 0.5, vec![]);
        click(&ctx, &mut layout, state.area_menus[&1].center(), 0.50);
        let state = frame(&ctx, &mut layout, 0.55, vec![]);
        click(
            &ctx,
            &mut layout,
            state.split_buttons[&(1, "V")].center(),
            0.58,
        );
        assert!(matches!(
            layout.root,
            Area::Split {
                axis: Axis::Vertical,
                ..
            }
        ));
        layout.validate().unwrap();
        let restored: DockLayout =
            serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
        assert_eq!(restored, layout);
    }
    #[test]
    fn corrupt_layout_is_rejected_without_replacing_unavailable_area_types() {
        let layout = DockLayout::default_for(WorkspaceKind::Engine);
        assert_eq!(layout.panes()[0].1, EditorType::EngineOverview);
        let invalid = DockLayout {
            root: Area::Split {
                id: 2,
                axis: Axis::Horizontal,
                ratio: 0.5,
                first: Box::new(Area::Pane {
                    id: 1,
                    editor: EditorType::Viewport,
                }),
                second: Box::new(Area::Pane {
                    id: 1,
                    editor: EditorType::Viewport,
                }),
            },
            next_id: 3,
            detached: vec![],
        };
        assert!(invalid.validate().is_err());
    }
    #[test]
    fn area_activation_includes_header_and_content_and_split_selects_new_area() {
        let ctx = egui::Context::default();
        let mut layout = DockLayout::default_for(WorkspaceKind::Project);
        let state = frame(&ctx, &mut layout, 0.0, vec![]);
        let area = &state.panes[1];
        let id = area.id;
        // Stay inside header chrome, away from the splitter's expanded drag
        // hit area. The four-pane Project layout has a divider above Assets.
        let header = Pos2::new(area.area_rect.right() - 12.0, area.area_rect.top() + 15.0);
        assert!(area.area_rect.contains(header));
        assert!(!area.rect.contains(header));
        let pressed = frame(
            &ctx,
            &mut layout,
            0.1,
            vec![
                egui::Event::PointerMoved(header),
                egui::Event::PointerButton {
                    pos: header,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert_eq!(pressed.activated_pane, Some(id));
        frame(
            &ctx,
            &mut layout,
            0.12,
            vec![egui::Event::PointerButton {
                pos: header,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let body = state.panes[2].rect.center();
        let body_id = state.panes[2].id;
        let pressed = frame(
            &ctx,
            &mut layout,
            0.2,
            vec![
                egui::Event::PointerMoved(body),
                egui::Event::PointerButton {
                    pos: body,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert_eq!(pressed.activated_pane, Some(body_id));
        frame(
            &ctx,
            &mut layout,
            0.22,
            vec![egui::Event::PointerButton {
                pos: body,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        click(&ctx, &mut layout, state.area_menus[&body_id].center(), 0.26);
        let state = frame(&ctx, &mut layout, 0.29, vec![]);
        let next = layout.next_id + 1;
        let split = click(
            &ctx,
            &mut layout,
            state.split_buttons[&(body_id, "H")].center(),
            0.3,
        );
        assert_eq!(split.activated_pane, Some(next));
        assert!(layout.panes().iter().any(|(id, _)| *id == next));
    }
    #[test]
    fn explicit_return_retains_the_same_id_with_changed_neighbors_and_safe_last_area() {
        let mut layout = DockLayout::single(EditorType::RustCustom);
        layout.detach(1).unwrap();
        let fallback = layout.docked_panes()[0].0;
        assert_ne!(fallback, 1);
        assert_eq!(layout.detached[0].id, 1);
        let restored: DockLayout =
            serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
        assert_eq!(restored, layout);
        layout.return_area(1).unwrap();
        assert_eq!(layout.docked_panes(), vec![(1, EditorType::RustCustom)]);
        assert!(layout.detached.is_empty());
        let other = layout.split(1, Axis::Horizontal).unwrap();
        layout.choose(other, EditorType::Assets);
        let third = layout.split(other, Axis::Vertical).unwrap();
        layout.choose(third, EditorType::Scene);
        layout.detach(other).unwrap();
        layout.close(third);
        layout.return_area(other).unwrap();
        layout.validate().unwrap();
        assert!(layout.docked_panes().contains(&(other, EditorType::Assets)));
        assert!(layout.docked_panes().contains(&(1, EditorType::RustCustom)));
    }
    #[test]
    fn close_detached_removes_only_that_instance_and_survives_layout_roundtrip() {
        let mut layout = DockLayout::single(EditorType::Scene);
        let second = layout.split(1, Axis::Horizontal).unwrap();
        let third = layout.split(second, Axis::Vertical).unwrap();
        layout.detach(second).unwrap();
        layout.detach(third).unwrap();
        assert!(layout.close(second));
        assert!(!layout.close(second));
        assert_eq!(layout.docked_panes(), vec![(1, EditorType::Scene)]);
        assert_eq!(
            layout.detached.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![third]
        );
        let mut restored: DockLayout =
            serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
        restored.validate().unwrap();
        assert!(!restored.panes().iter().any(|(id, _)| *id == second));
        let reopened = restored.split(1, Axis::Horizontal).unwrap();
        assert_ne!(reopened, second);
        assert!(
            restored
                .panes()
                .iter()
                .any(|(id, kind)| *id == reopened && *kind == EditorType::Scene)
        );
    }
    #[test]
    fn return_uses_a_shallow_leaf_when_original_neighbor_reaches_depth_limit() {
        let mut layout = DockLayout::single(EditorType::Inspector);
        let deep = layout.split(1, Axis::Horizontal).unwrap();
        let leaf = deep;
        for _ in 0..10 {
            layout.split(leaf, Axis::Horizontal).unwrap();
        }
        layout.detach(1).unwrap();
        layout.split(leaf, Axis::Horizontal).unwrap();
        layout.split(leaf, Axis::Horizontal).unwrap();
        layout.validate().unwrap();
        layout.return_area(1).unwrap();
        layout.validate().unwrap();
        assert!(layout.detached.is_empty());
        assert!(layout.panes().contains(&(1, EditorType::Inspector)));
    }
}
