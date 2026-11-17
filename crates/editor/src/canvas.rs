//! Shared visual canvas for module graphs and parsed Rust expression graphs.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::collections::{BTreeMap, BTreeSet};

pub const NODE_WIDTH: f32 = 220.0;
const HEADER: f32 = 28.0;
const ROW: f32 = 24.0;

#[derive(Clone)]
pub struct CanvasPort {
    pub name: String,
    pub data_type: String,
}

#[derive(Clone)]
pub struct CanvasNode {
    pub id: String,
    pub label: String,
    pub subtitle: String,
    pub position: [f32; 2],
    pub inputs: Vec<CanvasPort>,
    pub outputs: Vec<CanvasPort>,
    pub expandable: bool,
    pub opaque: bool,
}

#[derive(Clone)]
pub struct CanvasEdge {
    pub from_node: String,
    pub from_port: String,
    pub to_node: String,
    pub to_port: String,
}

#[derive(Debug)]
pub enum CanvasAction {
    Select(String),
    Move(String, [f32; 2]),
    Open(String),
    Connect {
        from_node: String,
        from_port: String,
        to_node: String,
        to_port: String,
    },
    DeleteEdge(usize),
    DeleteSelection {
        nodes: Vec<String>,
        edges: Vec<usize>,
    },
    Add(String, [f32; 2]),
}

#[derive(Default)]
pub struct CanvasLayout {
    pub rect: Option<Rect>,
    pub headers: BTreeMap<String, Rect>,
    pub inputs: BTreeMap<(String, String), (Pos2, String)>,
    pub outputs: BTreeMap<(String, String), (Pos2, String)>,
    pub menu: BTreeMap<String, Rect>,
    pub wires: BTreeMap<usize, Vec<Pos2>>,
}

pub struct CanvasState {
    pub pan: Vec2,
    pub zoom: f32,
    pub selected: Option<String>,
    pub pending: Option<(String, String)>,
    pub layout: CanvasLayout,
    pub message: Option<String>,
    pub selected_nodes: BTreeSet<String>,
    pub selected_edges: BTreeSet<usize>,
    edge_basis: Vec<(String, String, String, String)>,
    last_primary: Option<String>,
    pub palette: Vec<(String, String)>,
    pub allow_delete: bool,
    pub delete_reason: Option<String>,
    menu_search: String,
    menu_anchor: Option<(Pos2, Option<String>)>,
    pan_gesture: Option<(egui::PointerButton, Pos2, Pos2, bool)>,
    id: &'static str,
}

impl CanvasState {
    pub fn new(id: &'static str) -> Self {
        Self {
            pan: Vec2::new(28.0, 35.0),
            zoom: 1.0,
            selected: None,
            pending: None,
            layout: CanvasLayout::default(),
            message: None,
            selected_nodes: BTreeSet::new(),
            selected_edges: BTreeSet::new(),
            edge_basis: vec![],
            last_primary: None,
            palette: vec![],
            allow_delete: true,
            delete_reason: None,
            menu_search: String::new(),
            menu_anchor: None,
            pan_gesture: None,
            id,
        }
    }
    pub fn select_only(&mut self, id: Option<String>) {
        self.selected = id;
        self.selected_nodes.clear();
        self.selected_edges.clear();
        if let Some(id) = &self.selected {
            self.selected_nodes.insert(id.clone());
        }
        self.last_primary = self.selected.clone();
    }
    pub fn home(&mut self) {
        self.pan = Vec2::new(28.0, 35.0);
        self.zoom = 1.0;
        self.pending = None;
        self.message = None;
        self.selected_nodes.clear();
        self.selected_edges.clear();
        self.menu_anchor = None;
        self.pan_gesture = None;
    }
}

pub fn type_color(kind: &str) -> Color32 {
    match kind {
        "World" => Color32::from_rgb(176, 135, 249),
        "Scene3D" => Color32::from_rgb(61, 206, 183),
        "Frame3D" => Color32::from_rgb(245, 170, 88),
        "Scalar" | "f64" => Color32::from_rgb(99, 174, 252),
        "GpuColor" => Color32::from_rgb(246, 159, 79),
        "GpuNormals" => Color32::from_rgb(131, 203, 248),
        "GpuDepth" => Color32::from_rgb(196, 206, 220),
        "WorldCommands" => Color32::from_rgb(199, 144, 238),
        "RenderCommands" => Color32::from_rgb(237, 182, 96),
        "f32" | "u8" | "ObjectId" => Color32::from_rgb(99, 174, 252),
        "String" => Color32::from_rgb(127, 212, 155),
        "bool" => Color32::from_rgb(225, 130, 157),
        _ => Color32::from_rgb(163, 172, 193),
    }
}

fn compact(text: &str, len: usize) -> String {
    let text = text.lines().next().unwrap_or(text);
    if text.chars().count() > len {
        format!("{}…", text.chars().take(len).collect::<String>())
    } else {
        text.to_owned()
    }
}

fn curve(a: Pos2, b: Pos2) -> Vec<Pos2> {
    let bend = ((b.x - a.x).abs() * 0.48).clamp(40.0, 140.0);
    let c = a + Vec2::new(bend, 0.0);
    let d = b - Vec2::new(bend, 0.0);
    (0..=28)
        .map(|i| {
            let t = i as f32 / 28.0;
            let u = 1.0 - t;
            Pos2::new(
                u * u * u * a.x + 3.0 * u * u * t * c.x + 3.0 * u * t * t * d.x + t * t * t * b.x,
                u * u * u * a.y + 3.0 * u * u * t * c.y + 3.0 * u * t * t * d.y + t * t * t * b.y,
            )
        })
        .collect()
}

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let line = b - a;
    let t = if line.length_sq() > 0.0 {
        ((p - a).dot(line) / line.length_sq()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + line * t)
}

fn frame_nodes(state: &mut CanvasState, nodes: &[CanvasNode], canvas: Rect) {
    let selected = nodes
        .iter()
        .filter(|n| state.selected_nodes.is_empty() || state.selected_nodes.contains(&n.id))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return;
    }
    let bounds = selected.iter().fold(Rect::NOTHING, |rect, node| {
        let size = Vec2::new(
            NODE_WIDTH,
            HEADER + ROW * node.inputs.len().max(node.outputs.len()).max(1) as f32 + 28.,
        );
        rect.union(Rect::from_min_size(Pos2::from(node.position), size))
    });
    state.zoom = ((canvas.width() - 64.) / bounds.width())
        .min((canvas.height() - 64.) / bounds.height())
        .clamp(0.25, 2.5);
    state.pan = canvas.size() * 0.5 - bounds.center().to_vec2() * state.zoom;
}

/// Draw one canvas and return edits. Applying edits belongs to the model, so UI
/// connection gestures cannot bypass the graph's validation rules.
pub fn draw(
    ui: &mut egui::Ui,
    state: &mut CanvasState,
    nodes: &[CanvasNode],
    edges: &[CanvasEdge],
) -> Vec<CanvasAction> {
    let mut actions = Vec::new();
    let size = ui.available_size().max(Vec2::new(100.0, 100.0));
    let (canvas, background) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let painter = ui.painter_at(canvas);
    painter.rect_filled(canvas, 8.0, Color32::from_rgb(16, 21, 31));

    let pointer = ui.input(|i| i.pointer.hover_pos());
    let pointer_owned = ui.is_enabled()
        && pointer
            .is_some_and(|p| canvas.contains(p) && ui.ctx().layer_id_at(p) == Some(ui.layer_id()))
        && !egui::Popup::is_any_open(ui.ctx());
    if pointer_owned && ui.input(|i| i.pointer.any_pressed()) {
        background.request_focus();
    }
    let focused = ui.ctx().memory(|m| m.focused());
    let focused_node = focused.is_some_and(|focus| {
        nodes.iter().any(|node| {
            focus == ui.make_persistent_id((state.id, &node.id, "body"))
                || focus == ui.make_persistent_id((state.id, &node.id, "header"))
                || node
                    .inputs
                    .iter()
                    .any(|p| focus == ui.make_persistent_id((state.id, &node.id, &p.name, "input")))
                || node.outputs.iter().any(|p| {
                    focus == ui.make_persistent_id((state.id, &node.id, &p.name, "output"))
                })
        })
    });
    let keyboard = ui.is_enabled()
        && (background.has_focus() || focused_node)
        && !ui.ctx().text_edit_focused()
        && !egui::Popup::is_any_open(ui.ctx());
    if !ui.is_enabled() || !ui.input(|i| i.focused) {
        state.pan_gesture = None;
    }
    if pointer_owned {
        for button in [egui::PointerButton::Middle, egui::PointerButton::Secondary] {
            if ui.input(|i| i.pointer.button_pressed(button)) {
                let p = pointer.unwrap();
                state.pan_gesture = Some((button, p, p, false));
            }
        }
    }
    let mut right_click = None;
    let mut keyboard_menu = false;
    if let Some((button, start, last, dragged)) = state.pan_gesture
        && let Some(current) = pointer
    {
        let dragged = dragged || start.distance(current) >= 6.;
        if dragged {
            state.pan += current - last;
        }
        if ui.input(|i| i.pointer.button_released(button)) {
            background.request_focus();
            if button == egui::PointerButton::Secondary && !dragged {
                right_click = Some(current);
            }
            state.pan_gesture = None;
        } else if ui.input(|i| i.pointer.button_down(button)) {
            state.pan_gesture = Some((button, start, current, dragged));
        } else {
            state.pan_gesture = None;
        }
    }
    if pointer_owned {
        let factor = ui.input(|i| {
            let zoom = i.zoom_delta();
            if zoom != 1. {
                zoom
            } else {
                (i.events
                    .iter()
                    .filter_map(|event| {
                        if let egui::Event::MouseWheel { unit, delta, .. } = event {
                            Some(
                                delta.y
                                    * match unit {
                                        egui::MouseWheelUnit::Point => 1.,
                                        egui::MouseWheelUnit::Line => 40.,
                                        egui::MouseWheelUnit::Page => canvas.height(),
                                    },
                            )
                        } else {
                            None
                        }
                    })
                    .sum::<f32>()
                    * 0.002)
                    .exp()
            }
        });
        if factor.is_finite() && factor > 0. && factor != 1. {
            let zoom = (state.zoom * factor).clamp(0.25, 2.5);
            let anchor = pointer.unwrap() - canvas.min;
            state.pan = anchor - (anchor - state.pan) * (zoom / state.zoom);
            state.zoom = zoom;
        }
        ui.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
    }
    if keyboard && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        state.pending = None;
        state.menu_anchor = None;
    }
    if state.selected != state.last_primary {
        state.select_only(state.selected.clone());
    }
    state
        .selected_nodes
        .retain(|id| nodes.iter().any(|n| &n.id == id));
    let basis = edges
        .iter()
        .map(|e| {
            (
                e.from_node.clone(),
                e.from_port.clone(),
                e.to_node.clone(),
                e.to_port.clone(),
            )
        })
        .collect::<Vec<_>>();
    if state.edge_basis != basis {
        state.selected_edges = state
            .selected_edges
            .iter()
            .filter_map(|index| state.edge_basis.get(*index))
            .filter_map(|edge| basis.iter().position(|current| current == edge))
            .collect();
        state.edge_basis = basis;
    }
    state.selected_edges.retain(|i| *i < edges.len());
    if let Some(id) = &state.selected
        && nodes.iter().any(|n| &n.id == id)
    {
        state.selected_nodes.insert(id.clone());
    }
    if keyboard && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F)) {
        frame_nodes(state, nodes, canvas);
    }
    if keyboard && ui.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::F10)) {
        keyboard_menu = true;
        right_click = Some(
            pointer
                .filter(|p| canvas.contains(*p))
                .unwrap_or(canvas.center()),
        );
    }
    if keyboard
        && state.allow_delete
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete))
        && (!state.selected_nodes.is_empty() || !state.selected_edges.is_empty())
    {
        actions.push(CanvasAction::DeleteSelection {
            nodes: state.selected_nodes.iter().cloned().collect(),
            edges: state.selected_edges.iter().copied().collect(),
        });
        state.selected = None;
        state.selected_nodes.clear();
        state.selected_edges.clear();
        state.pending = None;
    }
    let grid = 24.0 * state.zoom;
    let first_x = canvas.left() + state.pan.x.rem_euclid(grid);
    let first_y = canvas.top() + state.pan.y.rem_euclid(grid);
    // Batch the tiny grid marks. Thousands of separate circle shapes used to
    // be rebuilt/tessellated for every pointer event when zoomed out.
    let mut grid_mesh = egui::Mesh::default();
    let mut x = first_x;
    while x < canvas.right() {
        let mut y = first_y;
        while y < canvas.bottom() {
            grid_mesh.add_colored_rect(
                Rect::from_center_size(Pos2::new(x, y), Vec2::splat(1.6)),
                Color32::from_rgb(35, 42, 57),
            );
            y += grid;
        }
        x += grid;
    }
    painter.add(egui::Shape::mesh(grid_mesh));

    let origin = canvas.min + state.pan;
    state.layout = CanvasLayout {
        rect: Some(canvas),
        ..CanvasLayout::default()
    };
    let mut rects = BTreeMap::new();
    for node in nodes {
        let p = origin + Vec2::new(node.position[0], node.position[1]) * state.zoom;
        let rows = node.inputs.len().max(node.outputs.len()).max(1) as f32;
        let rect = Rect::from_min_size(
            p,
            Vec2::new(NODE_WIDTH, HEADER + rows * ROW + 28.0) * state.zoom,
        );
        state.layout.headers.insert(
            node.id.clone(),
            Rect::from_min_size(p, Vec2::new(NODE_WIDTH, HEADER) * state.zoom),
        );
        for (i, port) in node.inputs.iter().enumerate() {
            state.layout.inputs.insert(
                (node.id.clone(), port.name.clone()),
                (
                    Pos2::new(
                        rect.left(),
                        p.y + (HEADER + 17.0 + i as f32 * ROW) * state.zoom,
                    ),
                    port.data_type.clone(),
                ),
            );
        }
        for (i, port) in node.outputs.iter().enumerate() {
            state.layout.outputs.insert(
                (node.id.clone(), port.name.clone()),
                (
                    Pos2::new(
                        rect.right(),
                        p.y + (HEADER + 17.0 + i as f32 * ROW) * state.zoom,
                    ),
                    port.data_type.clone(),
                ),
            );
        }
        rects.insert(node.id.clone(), rect);
    }

    // Hit ownership is geometric, not the registration order of overlapping
    // rectangles. At small zoom adjacent rows and neighboring nodes are close.
    let nearest_pin = |point: Pos2| {
        state
            .layout
            .inputs
            .iter()
            .map(|(key, (position, _))| (true, key, position))
            .chain(
                state
                    .layout
                    .outputs
                    .iter()
                    .map(|(key, (position, _))| (false, key, position)),
            )
            .filter(|(_, _, position)| canvas.contains(**position))
            .map(|(input, key, position)| (point.distance_sq(*position), input, key))
            .filter(|(distance, _, _)| *distance <= 8. * 8.)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, input, key)| (input, key.clone()))
    };
    let hot_pin = pointer.filter(|_| pointer_owned).and_then(nearest_pin);
    let pressed_pin = ui
        .input(|i| {
            (!i.modifiers.alt && i.pointer.button_pressed(egui::PointerButton::Primary))
                .then(|| i.pointer.press_origin())
                .flatten()
        })
        .filter(|point| {
            ui.is_enabled()
                && canvas.contains(*point)
                && ui.ctx().layer_id_at(*point) == Some(ui.layer_id())
                && !egui::Popup::is_any_open(ui.ctx())
        })
        .and_then(nearest_pin);
    if let Some((false, key)) = &pressed_pin {
        state.pending = Some(key.clone());
    }
    let additive = ui.input(|i| i.modifiers.shift || i.modifiers.ctrl || i.modifiers.command);
    let mut clicked_edge = false;
    for (index, edge) in edges.iter().enumerate() {
        if let (Some((a, kind)), Some((b, _))) = (
            state
                .layout
                .outputs
                .get(&(edge.from_node.clone(), edge.from_port.clone())),
            state
                .layout
                .inputs
                .get(&(edge.to_node.clone(), edge.to_port.clone())),
        ) {
            let points = curve(*a, *b);
            let hot = pointer_owned
                && pointer.is_some_and(|p| {
                    !rects.values().any(|r| r.contains(p))
                        && points
                            .windows(2)
                            .any(|w| distance_to_segment(p, w[0], w[1]) < 6.)
                });
            state.layout.wires.insert(index, points.clone());
            painter.add(egui::Shape::line(
                points,
                Stroke::new(
                    if hot || state.selected_edges.contains(&index) {
                        3.8
                    } else {
                        2.
                    },
                    if state.selected_edges.contains(&index) {
                        Color32::WHITE
                    } else {
                        type_color(kind)
                    },
                ),
            ));
            if hot && background.clicked() {
                clicked_edge = true;
                if ui.input(|i| i.modifiers.alt) && state.allow_delete {
                    actions.push(CanvasAction::DeleteEdge(index));
                    state.selected_edges.clear();
                } else {
                    if !additive {
                        state.selected_nodes.clear();
                        state.selected = None;
                        state.selected_edges.clear();
                    }
                    if !state.selected_edges.insert(index) {
                        state.selected_edges.remove(&index);
                    }
                }
            }
        }
    }
    let mut connect_target = None;
    let mut clicked_output = false;
    for node in nodes {
        let rect = rects[&node.id];
        if !canvas.intersects(rect) {
            continue;
        }
        let accent = if node.opaque {
            Color32::from_rgb(236, 171, 95)
        } else {
            node.outputs
                .first()
                .map(|p| type_color(&p.data_type))
                .unwrap_or(Color32::from_rgb(118, 137, 167))
        };
        painter.rect_filled(
            rect,
            7.0,
            if state.selected_nodes.contains(&node.id) {
                Color32::from_rgb(38, 49, 69)
            } else {
                Color32::from_rgb(27, 34, 48)
            },
        );
        painter.rect_stroke(
            rect,
            7.0,
            Stroke::new(
                if state.selected_nodes.contains(&node.id) {
                    1.7
                } else {
                    1.0
                },
                if state.selected_nodes.contains(&node.id) {
                    accent
                } else {
                    Color32::from_rgb(61, 73, 96)
                },
            ),
            egui::StrokeKind::Inside,
        );
        painter.line_segment(
            [
                rect.left_top() + Vec2::new(8.0, 2.0),
                rect.right_top() - Vec2::new(8.0, -2.0),
            ],
            Stroke::new(2.0, accent),
        );
        let body = ui.interact(
            rect,
            ui.make_persistent_id((state.id, &node.id, "body")),
            Sense::click(),
        );
        let header = state.layout.headers[&node.id];
        let response = ui.interact(
            header,
            ui.make_persistent_id((state.id, &node.id, "header")),
            Sense::click_and_drag(),
        );
        let clicked = (response.clicked() || body.clicked()) && hot_pin.is_none();
        if clicked || response.drag_started_by(egui::PointerButton::Primary) {
            if response.clicked() || response.drag_started_by(egui::PointerButton::Primary) {
                response.request_focus();
            } else {
                body.request_focus();
            }
            if !additive && (clicked || !state.selected_nodes.contains(&node.id)) {
                state.selected_nodes.clear();
                state.selected_edges.clear();
            }
            if additive && clicked && state.selected_nodes.contains(&node.id) {
                state.selected_nodes.remove(&node.id);
                state.selected = state.selected_nodes.iter().next().cloned();
            } else {
                state.selected_nodes.insert(node.id.clone());
                state.selected = Some(node.id.clone());
                actions.push(CanvasAction::Select(node.id.clone()));
            }
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = response.drag_delta() / state.zoom;
            for moved in nodes
                .iter()
                .filter(|n| state.selected_nodes.contains(&n.id))
            {
                actions.push(CanvasAction::Move(
                    moved.id.clone(),
                    [moved.position[0] + delta.x, moved.position[1] + delta.y],
                ));
            }
        }
        if response.double_clicked() && node.expandable {
            actions.push(CanvasAction::Open(node.id.clone()));
        }
        response.on_hover_text(if node.expandable {
            "Drag the header. Double-click to open the internal graph."
        } else {
            "Drag the header. The selected node's parameters are on the right."
        });
        painter.text(
            header.left_top() + Vec2::new(11.0, 10.0) * state.zoom,
            egui::Align2::LEFT_TOP,
            compact(&node.label, 24),
            FontId::proportional(13.0 * state.zoom),
            Color32::from_rgb(232, 237, 247),
        );
        painter.text(
            rect.left_bottom() + Vec2::new(10.0, -13.0) * state.zoom,
            egui::Align2::LEFT_CENTER,
            compact(&node.subtitle, 29),
            FontId::monospace(9.5 * state.zoom),
            Color32::from_rgb(127, 143, 167),
        );

        for port in &node.inputs {
            let (p, kind) = state.layout.inputs[&(node.id.clone(), port.name.clone())].clone();
            let response = ui.interact(
                Rect::from_center_size(p, Vec2::new(16., (ROW * state.zoom - 2.).clamp(4., 16.))),
                ui.make_persistent_id((state.id, &node.id, &port.name, "input")),
                Sense::click(),
            );
            let ready = state
                .pending
                .as_ref()
                .and_then(|from| state.layout.outputs.get(from))
                .is_some_and(|(_, out_kind)| out_kind == &kind);
            painter.circle_filled(p, if ready { 5.7 } else { 4.5 }, type_color(&kind));
            painter.text(
                p + Vec2::new(12.0, 0.0) * state.zoom,
                egui::Align2::LEFT_CENTER,
                compact(&port.name, 12),
                FontId::monospace(10.0 * state.zoom),
                Color32::from_rgb(179, 190, 207),
            );
            if hot_pin.as_ref() == Some(&(true, (node.id.clone(), port.name.clone())))
                && ui.input(|i| {
                    !i.modifiers.alt && i.pointer.button_released(egui::PointerButton::Primary)
                })
            {
                connect_target = Some((node.id.clone(), port.name.clone(), kind.clone()));
                response.request_focus();
            }
            response.on_hover_text(format!("{}: {}", port.name, kind));
        }
        for port in &node.outputs {
            let (p, kind) = state.layout.outputs[&(node.id.clone(), port.name.clone())].clone();
            let response = ui.interact(
                Rect::from_center_size(p, Vec2::new(16., (ROW * state.zoom - 2.).clamp(4., 16.))),
                ui.make_persistent_id((state.id, &node.id, &port.name, "output")),
                Sense::click_and_drag(),
            );
            painter.circle_filled(p, 4.5, type_color(&kind));
            painter.text(
                p - Vec2::new(12.0, 0.0) * state.zoom,
                egui::Align2::RIGHT_CENTER,
                compact(&port.name, 12),
                FontId::monospace(10.0 * state.zoom),
                Color32::from_rgb(179, 190, 207),
            );
            if hot_pin.as_ref() == Some(&(false, (node.id.clone(), port.name.clone()))) {
                clicked_output = true;
                if pressed_pin.as_ref() == hot_pin.as_ref() {
                    response.request_focus();
                }
            }
            response.on_hover_text(format!(
                "{}: {} — select an input of the same type",
                port.name, kind
            ));
        }
    }

    if let Some((to_node, to_port, kind)) = connect_target
        && let Some((from_node, from_port)) = state.pending.clone()
    {
        let from_kind = state
            .layout
            .outputs
            .get(&(from_node.clone(), from_port.clone()))
            .map(|(_, k)| k);
        if from_kind == Some(&kind) && from_node != to_node {
            actions.push(CanvasAction::Connect {
                from_node,
                from_port,
                to_node,
                to_port,
            });
            state.pending = None;
            state.message = None;
        } else {
            state.message =
                Some("Connection rejected: pin types must match and nodes must differ.".into());
        }
    }
    if let Some(from) = &state.pending
        && let (Some((a, kind)), Some(b)) = (
            state.layout.outputs.get(from),
            ui.input(|i| i.pointer.hover_pos()),
        )
    {
        painter.add(egui::Shape::line(
            curve(*a, b),
            Stroke::new(2.0, type_color(kind)),
        ));
    }
    if background.clicked()
        && !clicked_output
        && !clicked_edge
        && !pointer.is_some_and(|p| rects.values().any(|r| r.contains(p)))
        && state.pending.is_none()
        && !additive
    {
        state.selected = None;
        state.selected_nodes.clear();
        state.selected_edges.clear();
    }
    if let Some(point) = right_click {
        let node = if keyboard_menu {
            state.selected.clone()
        } else {
            nodes
                .iter()
                .rev()
                .find(|n| rects.get(&n.id).is_some_and(|r| r.contains(point)))
                .map(|n| n.id.clone())
        };
        if let Some(id) = &node {
            if !state.selected_nodes.contains(id) {
                state.selected_nodes.clear();
                state.selected_edges.clear();
                state.selected_nodes.insert(id.clone());
            }
            state.selected = Some(id.clone());
            actions.push(CanvasAction::Select(id.clone()));
        }
        state.menu_anchor = Some((point, node));
        state.menu_search.clear();
    }
    if let Some((point, node)) = state.menu_anchor.clone() {
        let mut open = true;
        egui::Popup::new(
            ui.make_persistent_id((state.id, "actions")),
            ui.ctx().clone(),
            egui::PopupAnchor::Position(point),
            ui.layer_id(),
        )
        .kind(egui::PopupKind::Menu)
        .open_bool(&mut open)
        .width(300.)
        .show(|ui| {
            ui.set_max_width(360.);
            if let Some(id) = node.as_ref() {
                ui.strong(
                    nodes
                        .iter()
                        .find(|n| &n.id == id)
                        .map(|n| n.label.as_str())
                        .unwrap_or(id),
                );
                if nodes.iter().any(|n| &n.id == id && n.expandable) {
                    let r = ui.button("Open · double-click");
                    state.layout.menu.insert("open".into(), r.rect);
                    if r.clicked() {
                        actions.push(CanvasAction::Open(id.clone()));
                        ui.close();
                    }
                }
                let r = ui.add_enabled(
                    state.allow_delete,
                    egui::Button::new("Delete selection · Delete"),
                );
                state.layout.menu.insert("delete".into(), r.rect);
                if r.clicked() {
                    actions.push(CanvasAction::DeleteSelection {
                        nodes: state.selected_nodes.iter().cloned().collect(),
                        edges: state.selected_edges.iter().copied().collect(),
                    });
                    state.selected = None;
                    state.selected_nodes.clear();
                    state.selected_edges.clear();
                    ui.close();
                }
                if let Some(reason) = &state.delete_reason {
                    ui.small(reason);
                }
            } else {
                ui.strong("Add node");
                if state.palette.is_empty() {
                    ui.label("No supported node creation in this context.");
                } else {
                    ui.add(
                        egui::TextEdit::singleline(&mut state.menu_search)
                            .hint_text("Search nodes"),
                    );
                    let search = state.menu_search.to_lowercase();
                    egui::ScrollArea::vertical()
                        .max_height(320.)
                        .show(ui, |ui| {
                            for (id, label) in &state.palette {
                                if !search.is_empty()
                                    && !label.to_lowercase().contains(&search)
                                    && !id.to_lowercase().contains(&search)
                                {
                                    continue;
                                }
                                let r = ui.button(label);
                                state.layout.menu.insert(id.clone(), r.rect);
                                if r.clicked() {
                                    let world = (point - canvas.min - state.pan) / state.zoom;
                                    actions.push(CanvasAction::Add(id.clone(), [world.x, world.y]));
                                    ui.close();
                                }
                            }
                        });
                }
            }
        });
        if !open {
            state.menu_anchor = None;
        }
    }
    painter.text(
        canvas.left_bottom() + Vec2::new(14.0, -14.0),
        egui::Align2::LEFT_BOTTOM,
        "MMB/RMB drag: pan · wheel: zoom · RMB / Shift+F10: actions · F: frame · Alt+click wire: remove",
        FontId::proportional(10.5),
        Color32::from_rgb(111, 126, 149),
    );
    state.last_primary = state.selected.clone();
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoomed_out_grid_is_batched_in_one_mesh_instead_of_thousands_of_shapes() {
        let ctx = egui::Context::default();
        let mut state = CanvasState::new("grid-batch-test");
        state.zoom = 0.45;
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600., 900.))),
                ..Default::default()
            },
            |ui| {
                assert!(draw(ui, &mut state, &[], &[]).is_empty());
            },
        );
        assert!(
            output.shapes.len() < 20,
            "Background dots must not create per-dot shapes"
        );
        let dense_meshes = output.shapes.iter().filter(|shape| {
            matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.vertices.len() > 1000)
        }).count();
        assert_eq!(dense_meshes, 1);
        output.drop_without_applying_deltas();
    }
    fn nodes() -> Vec<CanvasNode> {
        vec![
            CanvasNode {
                id: "a".into(),
                label: "Input".into(),
                subtitle: "".into(),
                position: [20., 40.],
                inputs: vec![],
                outputs: vec![CanvasPort {
                    name: "value".into(),
                    data_type: "Scalar".into(),
                }],
                expandable: false,
                opaque: false,
            },
            CanvasNode {
                id: "b".into(),
                label: "Output".into(),
                subtitle: "".into(),
                position: [290., 40.],
                inputs: vec![CanvasPort {
                    name: "base".into(),
                    data_type: "Scalar".into(),
                }],
                outputs: vec![],
                expandable: false,
                opaque: false,
            },
        ]
    }
    fn frame(
        ctx: &egui::Context,
        state: &mut CanvasState,
        nodes: &[CanvasNode],
        events: Vec<egui::Event>,
    ) -> Vec<CanvasAction> {
        frame_enabled(ctx, state, nodes, events, true)
    }
    fn frame_enabled(
        ctx: &egui::Context,
        state: &mut CanvasState,
        nodes: &[CanvasNode],
        events: Vec<egui::Event>,
        enabled: bool,
    ) -> Vec<CanvasAction> {
        let mut actions = vec![];
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 400.))),
                events,
                ..Default::default()
            },
            |ui| {
                if !enabled {
                    ui.disable();
                }
                actions = draw(ui, state, nodes, &[]);
            },
        );
        output.drop_without_applying_deltas();
        actions
    }
    fn click(
        ctx: &egui::Context,
        state: &mut CanvasState,
        nodes: &[CanvasNode],
        p: Pos2,
    ) -> Vec<CanvasAction> {
        frame(
            ctx,
            state,
            nodes,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        frame(
            ctx,
            state,
            nodes,
            vec![egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        )
    }
    #[test]
    fn ctrl_wheel_zooms_in_and_out_around_pointer_over_background_and_nodes() {
        for target in ["background", "header", "port"] {
            for direction in [1.0, -1.0] {
                let ctx = egui::Context::default();
                let mut state = CanvasState::new("wheel-zoom");
                let ns = nodes();
                frame(&ctx, &mut state, &ns, vec![]);
                let pointer = match target {
                    "header" => state.layout.headers["a"].center(),
                    "port" => state.layout.outputs[&("a".into(), "value".into())].0,
                    _ => Pos2::new(650.0, 300.0),
                };
                frame(
                    &ctx,
                    &mut state,
                    &ns,
                    vec![egui::Event::PointerMoved(pointer)],
                );
                let before_zoom = state.zoom;
                let before_anchor = (pointer - state.layout.headers["a"].min) / state.zoom;
                let modifiers = egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                };
                let actions = frame(
                    &ctx,
                    &mut state,
                    &ns,
                    vec![
                        egui::Event::ModifiersChanged(modifiers),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Line,
                            delta: Vec2::new(0.0, direction * 3.0),
                            phase: egui::TouchPhase::Move,
                            modifiers,
                        },
                    ],
                );
                assert_eq!(ctx.input(|input| input.smooth_scroll_delta()), Vec2::ZERO);
                assert!(
                    (state.zoom - before_zoom) * direction > 0.0,
                    "Ctrl+wheel must zoom {direction} over {target}"
                );
                let after_anchor = (pointer - state.layout.headers["a"].min) / state.zoom;
                assert!((before_anchor - after_anchor).length() < 0.001);
                assert!(actions.is_empty(), "zoom must not modify the graph");
                assert_eq!(ctx.zoom_factor(), 1.0, "editor UI scale stays unchanged");
            }
        }
    }
    #[test]
    fn zoom_outside_or_disabled_canvas_leaves_view_unchanged() {
        for (pointer, enabled, modifiers) in [
            (
                Pos2::new(900.0, 300.0),
                true,
                egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
            ),
            (
                Pos2::new(650.0, 300.0),
                false,
                egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
            ),
        ] {
            let ctx = egui::Context::default();
            let mut state = CanvasState::new("wheel-focus");
            let ns = nodes();
            frame_enabled(&ctx, &mut state, &ns, vec![], enabled);
            frame_enabled(
                &ctx,
                &mut state,
                &ns,
                vec![egui::Event::PointerMoved(pointer)],
                enabled,
            );
            let before = (state.zoom, state.pan);
            frame_enabled(
                &ctx,
                &mut state,
                &ns,
                vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Line,
                        delta: Vec2::new(0.0, 3.0),
                        phase: egui::TouchPhase::Move,
                        modifiers,
                    },
                ],
                enabled,
            );
            assert_eq!((state.zoom, state.pan), before);
        }
    }
    #[test]
    fn wheel_zooms_only_the_graph_under_the_pointer() {
        let ctx = egui::Context::default();
        let mut left = CanvasState::new("left");
        let mut right = CanvasState::new("right");
        let ns = nodes();
        for events in [
            vec![],
            vec![egui::Event::PointerMoved(Pos2::new(650.0, 300.0))],
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: Vec2::new(0.0, 3.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
            }],
        ] {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 400.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.columns(2, |columns| {
                        assert!(draw(&mut columns[0], &mut left, &ns, &[]).is_empty());
                        assert!(draw(&mut columns[1], &mut right, &ns, &[]).is_empty());
                    });
                },
            );
            output.drop_without_applying_deltas();
        }
        assert_eq!(left.zoom, 1.0);
        assert!(right.zoom > 1.0);
    }
    #[test]
    fn zoom_limits_keep_the_pointer_anchor_without_drift() {
        let ctx = egui::Context::default();
        let mut state = CanvasState::new("zoom-limits");
        let ns = nodes();
        let pointer = Pos2::new(650.0, 300.0);
        frame(
            &ctx,
            &mut state,
            &ns,
            vec![egui::Event::PointerMoved(pointer)],
        );
        let anchor = (pointer - state.layout.headers["a"].min) / state.zoom;
        for (factor, expected) in [(100.0, 2.5), (100.0, 2.5), (0.001, 0.25), (0.001, 0.25)] {
            frame(&ctx, &mut state, &ns, vec![egui::Event::Zoom(factor)]);
            assert_eq!(state.zoom, expected);
            let current = (pointer - state.layout.headers["a"].min) / state.zoom;
            assert!((anchor - current).length() < 0.001);
        }
    }
    #[test]
    fn pointer_gestures_connect_compatible_ports() {
        let ctx = egui::Context::default();
        let mut state = CanvasState::new("test");
        let ns = nodes();
        frame(&ctx, &mut state, &ns, vec![]);
        let out = state.layout.outputs[&("a".into(), "value".into())].0;
        let input = state.layout.inputs[&("b".into(), "base".into())].0;
        click(&ctx, &mut state, &ns, out);
        assert!(state.pending.is_some());
        let actions = click(&ctx, &mut state, &ns, input);
        assert!(actions.iter().any(|a|matches!(a,CanvasAction::Connect{from_node,to_node,..} if from_node=="a" && to_node=="b")));
    }
    #[test]
    fn dense_pins_choose_the_exact_resource_at_small_zoom_and_between_neighbors() {
        for zoom in [0.25, 0.5, 1., 2.5] {
            let ctx = egui::Context::default();
            let mut state = CanvasState::new("dense-pins");
            state.zoom = zoom;
            state.pan = Vec2::ZERO;
            let mut ns = nodes();
            ns[1].position = [245., 40.]; // authored 5-point gap, independent of zoom
            ns[0].outputs = ["color", "normals", "depth"]
                .into_iter()
                .map(|name| CanvasPort {
                    name: name.into(),
                    data_type: name.into(),
                })
                .collect();
            ns[1].inputs = ns[0].outputs.clone();
            frame(&ctx, &mut state, &ns, vec![]);
            for name in ["color", "normals", "depth"] {
                let from = state.layout.outputs[&("a".into(), name.into())].0;
                let to = state.layout.inputs[&("b".into(), name.into())].0;
                click(&ctx, &mut state, &ns, from);
                assert_eq!(
                    state.pending,
                    Some(("a".into(), name.into())),
                    "zoom {zoom}"
                );
                let actions = click(&ctx, &mut state, &ns, to);
                assert!(
                    actions.iter().any(
                        |a| matches!(a,CanvasAction::Connect{from_node,from_port,to_node,to_port}
                    if from_node=="a" && to_node=="b" && from_port==name && to_port==name)
                    ),
                    "zoom {zoom}: {actions:?}"
                );
                assert!(state.pending.is_none());
            }
        }
    }

    #[test]
    fn clipped_pin_centers_cannot_capture_a_visible_edge_connection() {
        for hidden_input in [false, true] {
            let ctx = egui::Context::default();
            let mut state = CanvasState::new("clipped-pins");
            state.pan = Vec2::ZERO;
            let mut ns = nodes();
            ns[0].position = [if hidden_input { -211. } else { -222. }, 40.];
            ns[1].position = [if hidden_input { -2. } else { 9. }, 40.];
            frame(&ctx, &mut state, &ns, vec![]);
            let rect = state.layout.rect.unwrap();
            let p = Pos2::new(
                rect.left() + 2.,
                state.layout.inputs[&("b".into(), "base".into())].0.y,
            );
            let actions = click(&ctx, &mut state, &ns, p);
            assert!(
                !actions
                    .iter()
                    .any(|a| matches!(a, CanvasAction::Connect { .. }))
            );
            assert_eq!(
                state.pending,
                hidden_input.then(|| ("a".into(), "value".into()))
            );
        }
    }

    #[test]
    fn pointer_gestures_reject_incompatible_types() {
        let ctx = egui::Context::default();
        let mut state = CanvasState::new("reject");
        let mut ns = nodes();
        ns[1].inputs[0].data_type = "Scene3D".into();
        frame(&ctx, &mut state, &ns, vec![]);
        let out = state.layout.outputs[&("a".into(), "value".into())].0;
        let input = state.layout.inputs[&("b".into(), "base".into())].0;
        click(&ctx, &mut state, &ns, out);
        let actions = click(&ctx, &mut state, &ns, input);
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, CanvasAction::Connect { .. }))
        );
        assert!(state.message.is_some());
    }

    #[test]
    fn header_drag_respects_zoom_and_double_click_opens_expandable_node() {
        let ctx = egui::Context::default();
        let mut state = CanvasState::new("header-gestures");
        state.zoom = 0.5;
        let mut ns = nodes();
        ns[0].expandable = true;
        frame(&ctx, &mut state, &ns, vec![]);
        let start = state.layout.headers["a"].center();
        frame(
            &ctx,
            &mut state,
            &ns,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );

        // Apply each returned model edit, as Editor does between frames. This
        // also catches accidentally accumulating the full drag distance twice.
        for (pointer, expected) in [
            (start + Vec2::new(40.0, 20.0), [100.0, 80.0]),
            (start + Vec2::new(55.0, 15.0), [130.0, 70.0]),
        ] {
            let actions = frame(
                &ctx,
                &mut state,
                &ns,
                vec![egui::Event::PointerMoved(pointer)],
            );
            let position = actions
                .iter()
                .find_map(|action| match action {
                    CanvasAction::Move(id, position) if id == "a" => Some(*position),
                    _ => None,
                })
                .expect("dragging the header must emit a Move action");
            assert!((position[0] - expected[0]).abs() < 0.001);
            assert!((position[1] - expected[1]).abs() < 0.001);
            assert!(
                !actions
                    .iter()
                    .any(|action| matches!(action, CanvasAction::Open(_)))
            );
            ns[0].position = position;
        }
        assert_eq!(state.selected.as_deref(), Some("a"));
        frame(
            &ctx,
            &mut state,
            &ns,
            vec![egui::Event::PointerButton {
                pos: start + Vec2::new(55.0, 15.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );

        let header = state.layout.headers["a"].center();
        let first = click(&ctx, &mut state, &ns, header);
        assert!(
            !first
                .iter()
                .any(|action| matches!(action, CanvasAction::Open(_)))
        );
        let second = click(&ctx, &mut state, &ns, header);
        assert_eq!(
            second
                .iter()
                .filter(|action| matches!(action, CanvasAction::Open(id) if id == "a"))
                .count(),
            1
        );

        // The same gesture must not expand an ordinary leaf module.
        let leaf = state.layout.headers["b"].center();
        click(&ctx, &mut state, &ns, leaf);
        let actions = click(&ctx, &mut state, &ns, leaf);
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, CanvasAction::Open(_)))
        );
    }
    fn frame_edges(
        ctx: &egui::Context,
        state: &mut CanvasState,
        nodes: &[CanvasNode],
        edges: &[CanvasEdge],
        events: Vec<egui::Event>,
    ) -> Vec<CanvasAction> {
        let mut actions = vec![];
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 400.))),
                events,
                ..Default::default()
            },
            |ui| {
                actions = draw(ui, state, nodes, edges);
            },
        );
        output.drop_without_applying_deltas();
        actions
    }
    fn pointer_button(
        point: Pos2,
        button: egui::PointerButton,
        pressed: bool,
        modifiers: egui::Modifiers,
    ) -> egui::Event {
        egui::Event::PointerButton {
            pos: point,
            button,
            pressed,
            modifiers,
        }
    }
    #[test]
    fn plain_wheel_zooms_without_changing_ui_scale_and_right_drag_over_node_does_not_open_menu() {
        let ctx = egui::Context::default();
        let mut state = CanvasState::new("ordinary");
        let ns = nodes();
        frame(&ctx, &mut state, &ns, vec![]);
        let at = state.layout.headers["a"].center();
        let actions = frame(
            &ctx,
            &mut state,
            &ns,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: Vec2::new(0., 3.),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(actions.is_empty());
        assert!(state.zoom > 1.);
        assert_eq!(ctx.zoom_factor(), 1.);
        let before = state.pan;
        frame(
            &ctx,
            &mut state,
            &ns,
            vec![
                egui::Event::PointerMoved(at),
                pointer_button(
                    at,
                    egui::PointerButton::Secondary,
                    true,
                    egui::Modifiers::NONE,
                ),
            ],
        );
        frame(
            &ctx,
            &mut state,
            &ns,
            vec![egui::Event::PointerMoved(at + Vec2::new(40., 10.))],
        );
        frame(
            &ctx,
            &mut state,
            &ns,
            vec![pointer_button(
                at + Vec2::new(40., 10.),
                egui::PointerButton::Secondary,
                false,
                egui::Modifiers::NONE,
            )],
        );
        assert_eq!(state.pan - before, Vec2::new(40., 10.));
        assert!(state.menu_anchor.is_none());
        assert!(state.layout.menu.is_empty());
    }
    #[test]
    fn alt_cut_near_a_small_pin_does_not_start_or_complete_a_replacement_wire() {
        let ctx = egui::Context::default();
        let mut state = CanvasState::new("small-alt-cut");
        state.zoom = 0.25;
        state.pan = Vec2::ZERO;
        let ns = nodes();
        let edges = vec![CanvasEdge {
            from_node: "a".into(),
            from_port: "value".into(),
            to_node: "b".into(),
            to_port: "base".into(),
        }];
        frame_edges(&ctx, &mut state, &ns, &edges, vec![]);
        let output = state.layout.outputs[&("a".into(), "value".into())].0;
        let wire = output + Vec2::new(3., 3.);
        for pressed in [true, false] {
            let actions = frame_edges(
                &ctx,
                &mut state,
                &ns,
                &edges,
                vec![
                    egui::Event::ModifiersChanged(egui::Modifiers::ALT),
                    egui::Event::PointerMoved(wire),
                    pointer_button(
                        wire,
                        egui::PointerButton::Primary,
                        pressed,
                        egui::Modifiers::ALT,
                    ),
                ],
            );
            if !pressed {
                assert!(
                    actions
                        .iter()
                        .any(|a| matches!(a, CanvasAction::DeleteEdge(0)))
                );
            }
        }
        assert!(
            state.pending.is_none(),
            "Cut must not leave a newly started wire: {:?}",
            state.pending
        );
        state.pending = Some(("a".into(), "value".into()));
        let input = state.layout.inputs[&("b".into(), "base".into())].0;
        for pressed in [true, false] {
            let actions = frame_edges(
                &ctx,
                &mut state,
                &ns,
                &edges,
                vec![
                    egui::Event::ModifiersChanged(egui::Modifiers::ALT),
                    egui::Event::PointerMoved(input),
                    pointer_button(
                        input,
                        egui::PointerButton::Primary,
                        pressed,
                        egui::Modifiers::ALT,
                    ),
                ],
            );
            assert!(
                !actions
                    .iter()
                    .any(|a| matches!(a, CanvasAction::Connect { .. })),
                "Alt is not a connection gesture"
            );
        }
    }

    #[test]
    fn mixed_selection_delete_is_one_action_and_alt_click_removes_only_the_wire() {
        let ctx = egui::Context::default();
        let mut state = CanvasState::new("mixed");
        let ns = nodes();
        let edges = vec![CanvasEdge {
            from_node: "a".into(),
            from_port: "value".into(),
            to_node: "b".into(),
            to_port: "base".into(),
        }];
        frame_edges(&ctx, &mut state, &ns, &edges, vec![]);
        let wire = state.layout.wires[&0][14];
        let a = state.layout.headers["a"].center();
        frame_edges(
            &ctx,
            &mut state,
            &ns,
            &edges,
            vec![
                egui::Event::PointerMoved(a),
                pointer_button(a, egui::PointerButton::Primary, true, egui::Modifiers::NONE),
            ],
        );
        frame_edges(
            &ctx,
            &mut state,
            &ns,
            &edges,
            vec![pointer_button(
                a,
                egui::PointerButton::Primary,
                false,
                egui::Modifiers::NONE,
            )],
        );
        frame_edges(
            &ctx,
            &mut state,
            &ns,
            &edges,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::SHIFT),
                egui::Event::PointerMoved(wire),
                pointer_button(
                    wire,
                    egui::PointerButton::Primary,
                    true,
                    egui::Modifiers::SHIFT,
                ),
            ],
        );
        frame_edges(
            &ctx,
            &mut state,
            &ns,
            &edges,
            vec![pointer_button(
                wire,
                egui::PointerButton::Primary,
                false,
                egui::Modifiers::SHIFT,
            )],
        );
        assert!(state.selected_nodes.contains("a"));
        assert!(state.selected_edges.contains(&0));
        let actions = frame_edges(
            &ctx,
            &mut state,
            &ns,
            &edges,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::NONE),
                egui::Event::Key {
                    key: egui::Key::Delete,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(
            matches!(actions.as_slice(),[CanvasAction::DeleteSelection{nodes,edges}] if nodes==&["a".to_owned()] && edges==&[0])
        );
        frame_edges(
            &ctx,
            &mut state,
            &ns,
            &edges,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::ALT),
                egui::Event::PointerMoved(wire),
                pointer_button(
                    wire,
                    egui::PointerButton::Primary,
                    true,
                    egui::Modifiers::ALT,
                ),
            ],
        );
        let actions = frame_edges(
            &ctx,
            &mut state,
            &ns,
            &edges,
            vec![pointer_button(
                wire,
                egui::PointerButton::Primary,
                false,
                egui::Modifiers::ALT,
            )],
        );
        assert!(matches!(actions.as_slice(), [CanvasAction::DeleteEdge(0)]));
    }
    #[test]
    fn context_menu_on_background_offers_actual_palette_and_node_menu_targets_selection() {
        let ctx = egui::Context::default();
        let mut state = CanvasState::new("context");
        state.palette = vec![("scalar.value".into(), "Scalar".into())];
        let ns = nodes();
        frame(&ctx, &mut state, &ns, vec![]);
        let at = Pos2::new(660., 250.);
        frame(
            &ctx,
            &mut state,
            &ns,
            vec![
                egui::Event::PointerMoved(at),
                pointer_button(
                    at,
                    egui::PointerButton::Secondary,
                    true,
                    egui::Modifiers::NONE,
                ),
            ],
        );
        frame(
            &ctx,
            &mut state,
            &ns,
            vec![pointer_button(
                at,
                egui::PointerButton::Secondary,
                false,
                egui::Modifiers::NONE,
            )],
        );
        frame(&ctx, &mut state, &ns, vec![]);
        let button = state.layout.menu["scalar.value"].center();
        let actions = click(&ctx, &mut state, &ns, button);
        assert!(
            actions
                .iter()
                .any(|a| matches!(a,CanvasAction::Add(id,_) if id=="scalar.value"))
        );
    }
    #[test]
    fn node_body_selects_and_keyboard_menu_keeps_that_node_when_pointer_leaves() {
        let ctx = egui::Context::default();
        let mut state = CanvasState::new("body-menu");
        let ns = nodes();
        frame(&ctx, &mut state, &ns, vec![]);
        let body = state.layout.headers["a"].left_bottom() + Vec2::new(80., 30.);
        click(&ctx, &mut state, &ns, body);
        assert_eq!(state.selected.as_deref(), Some("a"));
        frame(
            &ctx,
            &mut state,
            &ns,
            vec![
                egui::Event::PointerMoved(Pos2::new(700., 300.)),
                egui::Event::ModifiersChanged(egui::Modifiers::SHIFT),
                egui::Event::Key {
                    key: egui::Key::F10,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::SHIFT,
                },
            ],
        );
        assert_eq!(
            state.menu_anchor.as_ref().and_then(|(_, id)| id.as_deref()),
            Some("a")
        );
        frame(&ctx, &mut state, &ns, vec![]);
        assert!(state.layout.menu.contains_key("delete"));
        assert!(!state.layout.menu.contains_key("scalar.value"));
    }
}
