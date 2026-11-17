//! Shared native editor geometry and vector controls. No runtime services.
use eframe::egui::{self, Color32, FontId, Pos2, Response, Stroke, TextStyle, Vec2};

pub const CANVAS: Color32 = Color32::from_rgb(17, 24, 35);
pub const PANEL: Color32 = Color32::from_rgb(25, 34, 48);
pub const RAISED: Color32 = Color32::from_rgb(32, 45, 62);
pub const ACCENT: Color32 = Color32::from_rgb(102, 214, 196);
pub const SECONDARY: Color32 = Color32::from_rgb(182, 196, 213);
pub const TEXT: Color32 = Color32::from_rgb(230, 237, 245);

// Shared dense chrome metrics. Keep these local to shell, Area chrome and
// Scene overlays; ordinary forms and popup/menu rows retain their own spacing.
pub const CHROME_GAP: f32 = 2.;
pub const CHROME_PAD: i8 = 1;
pub const COMPACT_CONTROL_SIZE: f32 = 28.;
pub const AREA_HEADER_HEIGHT: f32 = COMPACT_CONTROL_SIZE + 2. * CHROME_PAD as f32;
pub const SCENE_TOOL_SIZE: f32 = 32.;
pub const SCENE_GROUP_HEIGHT: f32 = SCENE_TOOL_SIZE + 2. * CHROME_PAD as f32;
pub const SCENE_SPEED_WIDTH: f32 = 96.;

pub fn compact_spacing(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = Vec2::splat(CHROME_GAP);
}

pub fn status_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .inner_margin(egui::Margin::symmetric(CHROME_GAP as i8, CHROME_PAD))
}

/// Allocate a render surface with both edges on the physical pixel grid.
/// The same rectangle owns image placement and pointer coordinates.
pub fn image_area(ui: &mut egui::Ui, size: Vec2, sense: egui::Sense) -> (egui::Rect, Response) {
    use egui::emath::GuiRounding as _;
    let (id, raw_rect) = ui.allocate_space(size);
    let rect = raw_rect.round_to_pixels(ui.ctx().pixels_per_point());
    (rect, ui.interact(rect, id, sense))
}

pub fn configure(ctx: &egui::Context) {
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = RAISED;
    style.visuals.extreme_bg_color = CANVAS;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.selection.bg_fill = Color32::from_rgb(35, 77, 80);
    style.visuals.selection.stroke = Stroke::new(2., ACCENT);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.bg_fill = Color32::from_rgb(40, 57, 77);
        widget.weak_bg_fill = widget.bg_fill;
        widget.bg_stroke = Stroke::new(1., Color32::from_rgb(145, 161, 184));
        widget.fg_stroke = Stroke::new(1., TEXT);
        widget.corner_radius = 4.into();
    }
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(52, 74, 99);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(35, 77, 80);
    style.visuals.widgets.active.bg_stroke = Stroke::new(2., ACCENT);
    style.visuals.widgets.noninteractive.fg_stroke.color = SECONDARY;
    style.spacing.item_spacing = Vec2::new(8., 8.);
    style.spacing.button_padding = Vec2::new(8., 4.);
    style.spacing.interact_size.y = 28.;
    style.spacing.menu_spacing = 4.;
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(20.));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(13.));
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(egui::ThemePreference::Dark);
}

#[derive(Clone, Copy)]
pub enum Icon {
    Play,
    Pause,
    Stop,
    Restart,
    Save,
    Move,
    Rotate,
    Scale,
    Frame,
    Snap,
    World,
    Local,
    Settings,
    Folder,
    Plus,
    Back,
    Copy,
    More,
}

pub fn paint_icon(painter: &egui::Painter, rect: egui::Rect, icon: Icon, color: Color32) {
    let size = rect.width().min(rect.height()).min(48.);
    let origin = rect.center() - Vec2::splat(size * 0.5);
    let point = |x: f32, y: f32| origin + Vec2::new(x, y) * (size / 24.);
    let stroke = Stroke::new(2., color);
    let line = |coords: &[[f32; 2]]| {
        painter.add(egui::Shape::line(
            coords.iter().map(|p| point(p[0], p[1])).collect(),
            stroke,
        ));
    };
    match icon {
        Icon::Play => line(&[[7., 4.], [20., 12.], [7., 20.], [7., 4.]]),
        Icon::Pause => {
            line(&[[8., 4.], [8., 20.]]);
            line(&[[16., 4.], [16., 20.]]);
        }
        Icon::Stop => line(&[[5., 5.], [19., 5.], [19., 19.], [5., 19.], [5., 5.]]),
        Icon::Save => {
            line(&[
                [4., 3.],
                [17., 3.],
                [21., 7.],
                [21., 21.],
                [3., 21.],
                [3., 3.],
                [4., 3.],
            ]);
            line(&[[7., 3.], [7., 10.], [16., 10.], [16., 3.]]);
            line(&[[7., 21.], [7., 14.], [17., 14.], [17., 21.]]);
        }
        Icon::Restart | Icon::Rotate => {
            line(&[
                [19., 7.],
                [15., 3.],
                [8., 3.],
                [3., 8.],
                [3., 16.],
                [8., 21.],
                [16., 21.],
                [21., 16.],
            ]);
            line(&[[13., 7.], [19., 7.], [19., 1.]]);
        }
        Icon::Move => {
            line(&[[12., 2.], [12., 22.]]);
            line(&[[2., 12.], [22., 12.]]);
            line(&[[8., 6.], [12., 2.], [16., 6.]]);
            line(&[[8., 18.], [12., 22.], [16., 18.]]);
            line(&[[6., 8.], [2., 12.], [6., 16.]]);
            line(&[[18., 8.], [22., 12.], [18., 16.]]);
        }
        Icon::Scale => {
            line(&[[3., 15.], [3., 21.], [9., 21.], [9., 15.], [3., 15.]]);
            line(&[[9., 15.], [20., 4.], [14., 4.]]);
            line(&[[20., 4.], [20., 10.]]);
        }
        Icon::Frame => {
            for (x, y, dx, dy) in [
                (3., 3., 1., 1.),
                (21., 3., -1., 1.),
                (3., 21., 1., -1.),
                (21., 21., -1., -1.),
            ] {
                line(&[[x + 6. * dx, y], [x, y], [x, y + 6. * dy]]);
            }
        }
        Icon::Snap => {
            line(&[
                [4., 3.],
                [4., 14.],
                [8., 20.],
                [16., 20.],
                [20., 14.],
                [20., 3.],
                [15., 3.],
                [15., 13.],
                [12., 15.],
                [9., 13.],
                [9., 3.],
                [4., 3.],
            ]);
        }
        Icon::World => {
            painter.circle_stroke(point(12., 12.), size * 0.4, stroke);
            line(&[[3., 12.], [21., 12.]]);
            line(&[[12., 2.5], [8., 7.], [7., 12.], [8., 17.], [12., 21.5]]);
            line(&[[12., 2.5], [16., 7.], [17., 12.], [16., 17.], [12., 21.5]]);
        }
        Icon::Local => {
            line(&[
                [12., 2.],
                [21., 7.],
                [21., 17.],
                [12., 22.],
                [3., 17.],
                [3., 7.],
                [12., 2.],
            ]);
            line(&[[3., 7.], [12., 12.], [21., 7.]]);
            line(&[[12., 12.], [12., 22.]]);
        }
        Icon::Settings => {
            for (y, knob) in [(5., 8.), (12., 16.), (19., 8.)] {
                line(&[[3., y], [knob - 3., y]]);
                line(&[[knob + 3., y], [21., y]]);
                line(&[
                    [knob - 2., y - 2.],
                    [knob + 2., y - 2.],
                    [knob + 2., y + 2.],
                    [knob - 2., y + 2.],
                    [knob - 2., y - 2.],
                ]);
            }
        }
        Icon::Folder => line(&[
            [2., 6.],
            [2., 20.],
            [22., 20.],
            [22., 7.],
            [11., 7.],
            [9., 4.],
            [2., 4.],
            [2., 6.],
        ]),
        Icon::Plus => {
            line(&[[12., 4.], [12., 20.]]);
            line(&[[4., 12.], [20., 12.]]);
        }
        Icon::Back => line(&[[15., 4.], [7., 12.], [15., 20.]]),
        Icon::Copy => {
            line(&[[7., 7.], [21., 7.], [21., 21.], [7., 21.], [7., 7.]]);
            line(&[[17., 3.], [3., 3.], [3., 17.]]);
        }
        Icon::More => {
            for y in [5., 12., 19.] {
                painter.circle_filled(point(12., y), size / 12., color);
            }
        }
    }
}

#[derive(Clone, Default)]
struct IconHelpState {
    dismissed: bool,
    rect: Option<egui::Rect>,
}

pub fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    selected: bool,
) -> Response {
    icon_button_sized(ui, icon, label, enabled, selected, 32.)
}
pub fn icon_button_sized(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    selected: bool,
    size: f32,
) -> Response {
    let enabled = enabled && ui.is_enabled();
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .min_size(Vec2::splat(size))
            .selected(selected),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label)
    });
    paint_icon(
        ui.painter(),
        response.rect.shrink(6.),
        icon,
        if enabled {
            TEXT
        } else {
            Color32::from_rgb(123, 135, 152)
        },
    );
    // Disabled commands still have a keyboard-reachable explanation. The
    // wrapper owns help/focus only and can never activate the command.
    let help = if enabled {
        response.clone()
    } else {
        ui.interact(
            response.rect,
            response.id.with("help"),
            egui::Sense::focusable_noninteractive(),
        )
    };
    if !enabled {
        help.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Label,
                ui.is_enabled(),
                format!("Help: {label}"),
            )
        });
    }
    // Availability changes keep the same help identity and dismissed state.
    let state_id = response.id.with("help-state");
    let mut state = ui
        .ctx()
        .data(|data| data.get_temp::<IconHelpState>(state_id))
        .unwrap_or_default();
    let focused = help.has_focus();
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let in_tooltip = state
        .rect
        .is_some_and(|r| pointer.is_some_and(|p| r.expand(8.).contains(p)));
    let active = focused || response.contains_pointer() || in_tooltip;
    if !active {
        state = IconHelpState::default();
    }
    if active && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.dismissed = true;
    }
    if focused {
        ui.painter().rect_stroke(
            response.rect.shrink(1.),
            4,
            Stroke::new(2., ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    if !state.dismissed {
        let mut tooltip = if focused {
            egui::Tooltip::for_widget(&response)
        } else if enabled {
            egui::Tooltip::for_enabled(&response)
        } else {
            egui::Tooltip::for_disabled(&response)
        }
        .width(320.)
        .gap(8.);
        tooltip.popup = tooltip
            .popup
            .frame(egui::Frame::popup(ui.style()).inner_margin(8));
        if let Some(shown) = tooltip.show(|ui| {
            ui.style_mut().interaction.selectable_labels = true;
            ui.label(label);
        }) {
            state.rect = Some(shown.response.rect);
        }
    }
    ui.ctx().data_mut(|data| {
        if active {
            data.insert_temp(state_id, state);
        } else {
            data.remove::<IconHelpState>(state_id);
        }
    });
    response
}

pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(16.);
    ui.label(egui::RichText::new(title).size(16.).strong());
    ui.add_space(4.);
}

pub fn two_line_row(ui: &mut egui::Ui, title: &str, subtitle: &str, selected: bool) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            title,
        )
    });
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            4,
            if selected {
                Color32::from_rgb(35, 77, 80)
            } else {
                RAISED
            },
        );
    }
    for (text, size, y, color) in [(title, 14., 9., TEXT), (subtitle, 12., 31., SECONDARY)] {
        let mut job = egui::text::LayoutJob::simple_singleline(
            text.into(),
            FontId::proportional(size),
            color,
        );
        job.wrap.max_width = (rect.width() - 24.).max(1.);
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = ui.painter().layout_job(job);
        ui.painter()
            .galley(rect.left_top() + Vec2::new(12., y), galley, color);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.),
            4,
            Stroke::new(2., ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    response
}

pub fn type_label(ui: &mut egui::Ui, icon: Icon, text: &str, selected: bool) -> Response {
    let (rect, response) =
        ui.allocate_at_least(Vec2::new(ui.available_width(), 28.), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            text,
        )
    });
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            4,
            if selected {
                Color32::from_rgb(35, 77, 80)
            } else {
                RAISED
            },
        );
    }
    if selected {
        ui.painter().line_segment(
            [rect.left_top(), rect.left_bottom()],
            Stroke::new(3., ACCENT),
        );
    }
    paint_icon(
        ui.painter(),
        egui::Rect::from_center_size(
            Pos2::new(rect.left() + 16., rect.center().y),
            Vec2::splat(24.),
        ),
        icon,
        SECONDARY,
    );
    let mut job =
        egui::text::LayoutJob::simple_singleline(text.into(), FontId::proportional(14.), TEXT);
    job.wrap.max_width = (rect.width() - 44.).max(1.);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        Pos2::new(rect.left() + 36., rect.center().y - galley.size().y * 0.5),
        galley,
        TEXT,
    );
    if response.has_focus() {
        ui.painter()
            .rect_stroke(rect, 4, Stroke::new(2., ACCENT), egui::StrokeKind::Inside);
    }
    response.on_hover_text(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escape_dismisses_hover_and_focus_help_until_both_leave() {
        for enabled in [true, false] {
            let ctx = egui::Context::default();
            configure(&ctx);
            ctx.all_styles_mut(|s| {
                s.animation_time = 0.;
                s.interaction.tooltip_delay = 0.;
            });
            let mut time = 0.;
            let mut frame = |events| {
                time += 0.2;
                let mut response = None;
                let output = ctx.run_ui(
                    egui::RawInput {
                        time: Some(time),
                        screen_rect: Some(egui::Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(480., 240.),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        response = Some(icon_button(ui, Icon::Play, "Action help", enabled, false));
                    },
                );
                let shown = output.shapes.iter().any(
                    |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Action help"),
                );
                output.drop_without_applying_deltas();
                (shown, response.unwrap())
            };
            let (_, response) = frame(vec![]);
            let point = response.rect.center();
            frame(vec![egui::Event::PointerMoved(point)]);
            frame(vec![]);
            let (shown, _) = frame(vec![]);
            assert!(shown, "Hover help must be reachable");
            let focus = if enabled {
                response.id
            } else {
                response.id.with("help")
            };
            ctx.memory_mut(|m| m.request_focus(focus));
            let (shown, _) = frame(vec![]);
            assert!(shown, "Focused help must remain visible, enabled={enabled}");
            let event = |pressed| egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            };
            assert!(!frame(vec![event(true)]).0);
            assert!(
                !frame(vec![event(false)]).0,
                "Hover must not reopen dismissed focus help"
            );
            assert!(!frame(vec![]).0);
            ctx.memory_mut(|m| m.surrender_focus(focus));
            assert!(!frame(vec![]).0, "Hover-only channel must stay dismissed");
            frame(vec![egui::Event::PointerMoved(point + Vec2::splat(180.))]);
            frame(vec![egui::Event::PointerMoved(point)]);
            frame(vec![]);
            assert!(frame(vec![]).0, "Leaving both channels re-enables help");
        }
    }
    #[test]
    fn focused_icon_help_and_disabled_help_wrapper_do_not_reenter_context_lock() {
        let ctx = egui::Context::default();
        configure(&ctx);
        let mut enabled_id = None;
        let mut disabled_id = None;
        for pass in 0..4 {
            if pass == 1 {
                ctx.memory_mut(|m| m.request_focus(enabled_id.unwrap()));
            }
            if pass == 2 {
                ctx.memory_mut(|m| m.request_focus(disabled_id.unwrap()));
            }
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(480., 240.))),
                    ..Default::default()
                },
                |ui| {
                    ui.horizontal(|ui| {
                        let response =
                            icon_button(ui, Icon::Play, "Play · run a copy", true, false);
                        assert_eq!(response.rect.size(), Vec2::splat(32.));
                        enabled_id = Some(response.id);
                        let response =
                            icon_button(ui, Icon::Stop, "Stop · no Play session", false, false);
                        assert!(!response.clicked());
                        disabled_id = Some(response.id.with("help"));
                    });
                },
            )
            .drop_without_applying_deltas();
        }
    }
}
