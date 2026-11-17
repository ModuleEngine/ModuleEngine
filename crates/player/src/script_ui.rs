//! Native presentation of validated, author-owned Custom menu descriptions.
//! This emits portable input actions; the host remains the setting authority.
use eframe::egui::{self, Rect};
use module_engine_core::{
    GameInput, ScriptSettingsSnapshot, ScriptUiItem, ScriptUiSnapshot, script_ui_action_key,
};
use serde_json::Value;

#[derive(Default)]
pub struct ScriptUiResponse {
    pub input: GameInput,
    pub blocks_world: bool,
}

/// Limit only this menu's paint layers. A ComboBox's standard popup otherwise
/// uses the whole native window as its paint boundary, including Editor panes.
fn clip_menu_layer(ctx: &egui::Context, layer: egui::LayerId, rect: Rect) {
    ctx.graphics_mut(|graphics| {
        if let Some(paint) = graphics.get_mut(layer) {
            let count = paint.all_entries().len();
            for index in 0..count {
                paint.mutate_shape(egui::layers::ShapeIdx(index), |shape| {
                    shape.clip_rect = shape.clip_rect.intersect(rect);
                });
            }
        }
    });
}

/// Standard ComboBox popups use the entire OS window for placement. Translate
/// only their own layer into the Game viewport; egui applies this transform to
/// both graphics and hit testing. No neighboring Editor layer is changed.
fn bound_choice_popup(ctx: &egui::Context, response: &egui::Response, rect: Rect) {
    let popup_id = egui::Popup::default_response_id(response);
    let layer = egui::LayerId::new(egui::Order::Foreground, popup_id);
    if let Some(popup) = ctx.memory(|memory| memory.area_rect(popup_id)) {
        let left = popup
            .left()
            .clamp(rect.left(), (rect.right() - popup.width()).max(rect.left()));
        let top = popup
            .top()
            .clamp(rect.top(), (rect.bottom() - popup.height()).max(rect.top()));
        let offset = egui::pos2(left, top) - popup.min;
        let transform = egui::emath::TSTransform::from_translation(offset);
        ctx.set_transform_layer(layer, transform);
        clip_menu_layer(ctx, layer, transform.inverse().mul_rect(rect));
    }
}

/// Panel positions and widths are logical points relative to the Game viewport.
/// `settings` is accepted host state; `displayed` belongs to the actual frame.
pub fn draw_script_ui(
    ui: &mut egui::Ui,
    rect: Rect,
    menus: &ScriptUiSnapshot,
    settings: &ScriptSettingsSnapshot,
    displayed: &ScriptSettingsSnapshot,
    error: Option<&str>,
) -> ScriptUiResponse {
    let mut result = ScriptUiResponse::default();
    if !rect.is_positive() {
        return result;
    }
    let mut drawn = false;
    for (instance, panels) in menus {
        for panel in panels {
            drawn = true;
            let width = panel.width.min((rect.width() - 16.).max(1.));
            let position = rect.min + egui::vec2(panel.position[0], panel.position[1]);
            let window = egui::Window::new(&panel.title)
                .auto_sized()
                .id(egui::Id::new(("script-menu", ui.id(), instance, &panel.id)))
                .fixed_pos(position)
                .constrain_to(rect)
                .min_width(0.)
                .default_width(width)
                .max_width(width)
                .max_height((rect.height() - 16.).max(1.))
                .resizable(false)
                .movable(false)
                .collapsible(false)
                .title_bar(false)
                .vscroll(true)
                .show(ui.ctx(), |ui| {
                    ui.set_clip_rect(ui.clip_rect().intersect(rect));
                    ui.set_max_width(width);
                    if !panel.title.is_empty() {
                        ui.add(
                            egui::Label::new(egui::RichText::new(&panel.title).heading()).wrap(),
                        );
                    }
                    if let Some(error) = error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    for item in &panel.items {
                        match item {
                            ScriptUiItem::Text { text } => {
                                ui.add(egui::Label::new(text).wrap());
                            }
                            ScriptUiItem::Choice { id, label, setting } => {
                                let Some(current) = settings
                                    .get(setting)
                                    .filter(|setting| !setting.options.is_empty())
                                else {
                                    continue;
                                };
                                ui.push_id((instance, &panel.id, id), |ui| {
                                    if !label.is_empty() {
                                        ui.add(egui::Label::new(label).wrap());
                                    }
                                    let selected_label = current
                                        .selected
                                        .as_ref()
                                        .and_then(|selected| {
                                            current
                                                .options
                                                .iter()
                                                .find(|option| &option.id == selected)
                                        })
                                        .map_or("Runtime controlled", |option| {
                                            option.label.as_str()
                                        });
                                    let mut was_popup_open = false;
                                    let response = ui
                                        .add_enabled_ui(error.is_none(), |ui| {
                                            was_popup_open = egui::ComboBox::is_open(
                                                ui.ctx(),
                                                ui.make_persistent_id(egui::IdSalt::new("choice")),
                                            );
                                            egui::ComboBox::from_id_salt("choice")
                                                .width(ui.available_width())
                                                .height((rect.height() - 32.).clamp(24., 200.))
                                                .wrap()
                                                .selected_text(selected_label)
                                                .show_ui(ui, |ui| {
                                                    // Keep the standard ScrollArea hit/paint clip
                                                    // in this layer's local coordinates. Its final
                                                    // measured area is bounded below, before egui
                                                    // publishes graphics and interaction geometry.
                                                    ui.set_max_width((width - 16.).max(1.));
                                                    ui.style_mut().wrap_mode =
                                                        Some(egui::TextWrapMode::Wrap);
                                                    for option in &current.options {
                                                        let response = ui.selectable_label(
                                                            current.selected.as_ref()
                                                                == Some(&option.id),
                                                            &option.label,
                                                        );
                                                        let pointer_allowed =
                                                            !response.clicked_by(
                                                                egui::PointerButton::Primary,
                                                            ) || ui.ctx().input(|input| {
                                                                input
                                                                    .pointer
                                                                    .interact_pos()
                                                                    .is_some_and(|position| {
                                                                        rect.contains(position)
                                                                    })
                                                            });
                                                        if response.clicked() && pointer_allowed {
                                                            // Clicking the current option is meaningful: the host
                                                            // can retry a previously failed persistence write.
                                                            let key =
                                                                script_ui_action_key(instance, id);
                                                            result
                                                                .input
                                                                .actions
                                                                .insert(key.clone());
                                                            result.input.action_values.insert(
                                                                key,
                                                                Value::String(option.id.clone()),
                                                            );
                                                            result.blocks_world = true;
                                                        }
                                                    }
                                                })
                                                .response
                                        })
                                        .inner;
                                    let popup = egui::ComboBox::is_open(ui.ctx(), response.id);
                                    if !popup
                                        && ui.input(|input| input.pointer.any_pressed())
                                        && !response.contains_pointer()
                                    {
                                        ui.memory_mut(|memory| memory.surrender_focus(response.id));
                                    }
                                    result.blocks_world |= was_popup_open
                                        || popup
                                        || response.has_focus()
                                        || response.contains_pointer();
                                    bound_choice_popup(ui.ctx(), &response, rect);
                                    if displayed
                                        .get(setting)
                                        .is_none_or(|shown| shown.selected != current.selected)
                                    {
                                        ui.small("Applying…");
                                    }
                                    if let Some(reason) = &current.rejection {
                                        ui.colored_label(
                                            ui.visuals().error_fg_color,
                                            format!("Refused: {reason}"),
                                        );
                                    }
                                    if current.persistence.error.is_some()
                                        || (current.persistence.enabled
                                            && !current.persistence.saved)
                                    {
                                        ui.colored_label(
                                            ui.visuals().error_fg_color,
                                            "Not saved · works until exit",
                                        );
                                        if let Some(reason) = &current.persistence.error {
                                            ui.add(egui::Label::new(reason).wrap());
                                        }
                                    } else if !current.persistence.enabled
                                        && current.source == "local"
                                    {
                                        ui.small("Works until exit");
                                    }
                                });
                            }
                        }
                    }
                });
            if let Some(window) = window {
                result.blocks_world |= window.response.contains_pointer();
                clip_menu_layer(ui.ctx(), window.response.layer_id, rect);
            }
        }
    }
    if !drawn && let Some(error) = error {
        let area = egui::Area::new(egui::Id::new(("script-menu-error", ui.id())))
            .fixed_pos(rect.min + egui::vec2(8., 8.))
            .constrain_to(rect)
            .show(ui.ctx(), |ui| {
                ui.set_clip_rect(rect);
                ui.set_max_width((rect.width() - 16.).max(1.));
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                });
            });
        result.blocks_world |= area.response.contains_pointer();
        clip_menu_layer(ui.ctx(), area.response.layer_id, rect);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_engine_core::{
        ScriptSettingOption, ScriptSettingPersistence, ScriptSettingSnapshot, ScriptUiPanel,
        script_ui_actions,
    };
    use serde_json::json;

    fn fixture() -> (ScriptUiSnapshot, ScriptSettingsSnapshot) {
        let menus = [(
            "actor.one:[2]".into(),
            vec![ScriptUiPanel {
                id: "custom/panel".into(),
                title: "Author menu".into(),
                position: [16., 16.],
                width: 260.,
                items: vec![
                    ScriptUiItem::Text {
                        text: "A game-owned explanation".into(),
                    },
                    ScriptUiItem::Choice {
                        id: "edge:choice/[0]".into(),
                        label: "Edge treatment".into(),
                        setting: "custom/setting:[1]".into(),
                    },
                ],
            }],
        )]
        .into();
        let settings = [(
            "custom/setting:[1]".into(),
            ScriptSettingSnapshot {
                label: "Arbitrary setting".into(),
                options: vec![
                    ScriptSettingOption {
                        id: "option/one".into(),
                        label: "Soft edges".into(),
                    },
                    ScriptSettingOption {
                        id: "option:two".into(),
                        label: "Sharp pixels".into(),
                    },
                ],
                selected: Some("option/one".into()),
                source: "project".into(),
                persistence: ScriptSettingPersistence {
                    enabled: true,
                    saved: true,
                    error: None,
                },
                rejection: None,
            },
        )]
        .into();
        (menus, settings)
    }

    fn frame(
        ctx: &egui::Context,
        menus: &ScriptUiSnapshot,
        settings: &ScriptSettingsSnapshot,
        displayed: &ScriptSettingsSnapshot,
        error: Option<&str>,
        events: Vec<egui::Event>,
    ) -> (egui::FullOutput, ScriptUiResponse) {
        // egui Area::begin marks a previously unseen Window as an invisible,
        // non-interactable sizing pass. Settle that native container before
        // asserting visibility or sending a click, as the running UI does.
        if ctx.cumulative_frame_nr() == 0 {
            for _ in 0..2 {
                raw_frame(ctx, menus, settings, displayed, error, vec![])
                    .0
                    .drop_without_applying_deltas();
            }
        }
        raw_frame(ctx, menus, settings, displayed, error, events)
    }

    fn raw_frame(
        ctx: &egui::Context,
        menus: &ScriptUiSnapshot,
        settings: &ScriptSettingsSnapshot,
        displayed: &ScriptSettingsSnapshot,
        error: Option<&str>,
        events: Vec<egui::Event>,
    ) -> (egui::FullOutput, ScriptUiResponse) {
        raw_frame_at(
            ctx,
            menus,
            settings,
            displayed,
            error,
            Rect::from_min_size(egui::pos2(24., 24.), egui::vec2(320., 300.)),
            events,
        )
    }

    fn raw_frame_at(
        ctx: &egui::Context,
        menus: &ScriptUiSnapshot,
        settings: &ScriptSettingsSnapshot,
        displayed: &ScriptSettingsSnapshot,
        error: Option<&str>,
        rect: Rect,
        events: Vec<egui::Event>,
    ) -> (egui::FullOutput, ScriptUiResponse) {
        let mut result = ScriptUiResponse::default();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640., 480.),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| {
                result = draw_script_ui(ui, rect, menus, settings, displayed, error);
            },
        );
        (output, result)
    }

    #[test]
    fn popup_near_short_game_bottom_keeps_options_reachable_and_outside_clicks_inert() {
        let (mut menus, settings) = fixture();
        menus.values_mut().next().unwrap()[0].position = [8., 112.];
        let ctx = context();
        let game = Rect::from_min_size(egui::pos2(24., 24.), egui::vec2(320., 160.));
        let draw = |events| raw_frame_at(&ctx, &menus, &settings, &settings, None, game, events);
        for _ in 0..2 {
            draw(vec![]).0.drop_without_applying_deltas();
        }
        let (output, _) = draw(vec![]);
        let button = text_rect(&output, "Soft edges").center();
        output.drop_without_applying_deltas();
        draw(click(button)).0.drop_without_applying_deltas();
        let popup_layer = ctx
            .memory(|memory| {
                memory
                    .areas()
                    .visible_layer_ids()
                    .into_iter()
                    .find(|layer| layer.order == egui::Order::Foreground)
            })
            .expect("The actual ComboBox popup has its own foreground layer");
        let (output, _) = draw(vec![]);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Sharp pixels")), "First popup must contain the second option; area={:?}; transform={:?}; texts={:?}", ctx.memory(|memory| memory.area_rect(popup_layer.id)), ctx.layer_transform_to_global(popup_layer), output.shapes.iter().filter_map(|shape| match &shape.shape { egui::Shape::Text(text) => Some((text.galley.text(), text.pos, shape.clip_rect)), _ => None }).collect::<Vec<_>>());
        let visible = text_rect(&output, "Sharp pixels");
        assert!(
            game.contains_rect(visible),
            "The last option must be fully visible in the short Game viewport: {visible:?}"
        );
        let transform = ctx
            .layer_transform_to_global(popup_layer)
            .expect("The popup is translated into Game bounds");
        let outside = transform.inverse().mul_pos(visible.center());
        assert!(
            !game.contains(outside),
            "The unbounded native popup would place this option outside Game"
        );
        output.drop_without_applying_deltas();
        let (output, result) = draw(click(outside));
        assert!(
            result.input.actions.is_empty(),
            "An invisible original popup position must never emit input"
        );
        output.drop_without_applying_deltas();
        let (output, _) = draw(vec![]);
        let button = text_rect(&output, "Soft edges").center();
        output.drop_without_applying_deltas();
        draw(click(button)).0.drop_without_applying_deltas();
        let (output, _) = draw(vec![]);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Sharp pixels")), "Reopened popup must contain the second option; popup open={}", egui::Popup::is_id_open(&ctx, popup_layer.id));
        let visible = text_rect(&output, "Sharp pixels").center();
        output.drop_without_applying_deltas();
        let (output, result) = draw(click(visible));
        assert_eq!(
            script_ui_actions(&result.input, "actor.one:[2]")["edge:choice/[0]"],
            json!("option:two")
        );
        assert!(result.blocks_world);
        output.drop_without_applying_deltas();
    }

    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        ctx.all_styles_mut(|style| style.animation_time = 0.);
        ctx
    }

    #[test]
    fn already_open_popup_resized_to_tiny_game_rejects_outside_transformed_option_click() {
        let (menus, settings) = fixture();
        let ctx = context();
        let original = Rect::from_min_size(egui::pos2(24., 24.), egui::vec2(320., 160.));
        let tiny = Rect::from_min_size(original.min, egui::vec2(320., 20.));
        let draw =
            |rect, events| raw_frame_at(&ctx, &menus, &settings, &settings, None, rect, events);
        for _ in 0..2 {
            draw(original, vec![]).0.drop_without_applying_deltas();
        }
        let (output, _) = draw(original, vec![]);
        let choice = text_rect(&output, "Soft edges").center();
        output.drop_without_applying_deltas();
        draw(original, click(choice))
            .0
            .drop_without_applying_deltas();
        draw(original, vec![]).0.drop_without_applying_deltas();
        let (output, _) = draw(tiny, vec![]);
        let option = text_rect(&output, "Soft edges");
        let outside = egui::pos2(option.center().x, option.bottom() - 1.);
        assert!(
            !tiny.contains(outside),
            "The fixture must target the actual transformed option outside the resized Game: {option:?}"
        );
        output.drop_without_applying_deltas();
        let (output, result) = draw(tiny, click(outside));
        assert!(
            result.input.actions.is_empty(),
            "An option outside the current Game cannot be clicked after resize"
        );
        assert!(result.blocks_world);
        output.drop_without_applying_deltas();
    }

    #[test]
    fn outside_click_dismisses_the_menu_without_becoming_a_world_click() {
        let (menus, settings) = fixture();
        let ctx = context();
        let (output, _) = frame(&ctx, &menus, &settings, &settings, None, vec![]);
        let choice = text_rect(&output, "Soft edges").center();
        output.drop_without_applying_deltas();
        frame(&ctx, &menus, &settings, &settings, None, click(choice))
            .0
            .drop_without_applying_deltas();
        frame(&ctx, &menus, &settings, &settings, None, vec![])
            .0
            .drop_without_applying_deltas();
        let outside = egui::pos2(340., 316.);
        let press = vec![
            egui::Event::PointerMoved(outside),
            egui::Event::PointerButton {
                pos: outside,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ];
        let (output, result) = frame(&ctx, &menus, &settings, &settings, None, press);
        assert!(result.blocks_world && result.input.actions.is_empty());
        output.drop_without_applying_deltas();
        let release = vec![egui::Event::PointerButton {
            pos: outside,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }];
        let (output, result) = frame(&ctx, &menus, &settings, &settings, None, release);
        assert!(
            result.blocks_world && result.input.actions.is_empty(),
            "The dismissal release belongs to the menu"
        );
        output.drop_without_applying_deltas();
        frame(&ctx, &menus, &settings, &settings, None, vec![])
            .0
            .drop_without_applying_deltas();
        let (output, _) = frame(&ctx, &menus, &settings, &settings, None, vec![]);
        let choice = text_rect(&output, "Soft edges").center();
        output.drop_without_applying_deltas();
        frame(&ctx, &menus, &settings, &settings, None, click(choice))
            .0
            .drop_without_applying_deltas();
        frame(&ctx, &menus, &settings, &settings, None, vec![])
            .0
            .drop_without_applying_deltas();
        let (output, result) = frame(&ctx, &menus, &settings, &settings, None, click(outside));
        assert!(
            result.blocks_world && result.input.actions.is_empty(),
            "A batched press/release dismissal belongs entirely to the menu"
        );
        output.drop_without_applying_deltas();
        frame(&ctx, &menus, &settings, &settings, None, vec![])
            .0
            .drop_without_applying_deltas();
        let (output, result) = frame(&ctx, &menus, &settings, &settings, None, click(outside));
        assert!(
            !result.blocks_world,
            "A fresh background gesture may return to the world"
        );
        output.drop_without_applying_deltas();
    }

    fn text_rect(output: &egui::FullOutput, text: &str) -> Rect {
        output
            .shapes
            .iter()
            .rev()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(galley) if galley.galley.text() == text => {
                    Some(Rect::from_min_size(galley.pos, galley.galley.size()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing menu text: {text}"))
    }

    fn click(pos: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    }

    #[test]
    fn author_choice_emits_scoped_action_including_retry_of_the_current_option() {
        let (menus, settings) = fixture();
        for (label, option) in [("Sharp pixels", "option:two"), ("Soft edges", "option/one")] {
            let ctx = context();
            let (output, _) = frame(&ctx, &menus, &settings, &settings, None, vec![]);
            let button = text_rect(&output, "Soft edges").center();
            output.drop_without_applying_deltas();
            frame(&ctx, &menus, &settings, &settings, None, click(button))
                .0
                .drop_without_applying_deltas();
            let (output, result) = frame(&ctx, &menus, &settings, &settings, None, vec![]);
            assert!(
                result.blocks_world,
                "An open menu owns input outside its panel too"
            );
            let selected = text_rect(&output, label).center();
            output.drop_without_applying_deltas();
            let (output, result) = frame(&ctx, &menus, &settings, &settings, None, click(selected));
            assert_eq!(
                script_ui_actions(&result.input, "actor.one:[2]")["edge:choice/[0]"],
                json!(option)
            );
            assert!(result.input.values.is_empty() && !result.input.restart);
            assert_eq!(result.input.actions.len(), 1);
            assert!(result.blocks_world);
            output.drop_without_applying_deltas();
        }
    }

    #[test]
    fn latest_host_selection_reports_applying_refused_and_unsaved_without_faking_displayed_state() {
        let (menus, mut settings) = fixture();
        let displayed = settings.clone();
        let current = settings.get_mut("custom/setting:[1]").unwrap();
        current.selected = Some("option:two".into());
        current.source = "local".into();
        current.rejection = Some("A different request was rejected".into());
        current.persistence.saved = false;
        current.persistence.error = Some("Storage is read-only".into());
        let ctx = context();
        let (output, result) = frame(&ctx, &menus, &settings, &displayed, None, vec![]);
        for text in [
            "Sharp pixels",
            "Applying…",
            "Refused: A different request was rejected",
            "Not saved · works until exit",
            "Storage is read-only",
        ] {
            text_rect(&output, text);
        }
        assert!(result.input.actions.is_empty());
        output.drop_without_applying_deltas();
        let current = settings.get_mut("custom/setting:[1]").unwrap();
        current.selected = None;
        current.source = "mixed".into();
        let (output, _) = frame(&ctx, &menus, &settings, &settings, None, vec![]);
        text_rect(&output, "Runtime controlled");
        output.drop_without_applying_deltas();
    }

    #[test]
    fn removed_or_unbindable_settings_hide_choices_and_malformed_packet_disables_previous_menu() {
        let (menus, settings) = fixture();
        let ctx = context();
        let (output, result) = frame(&ctx, &menus, &Default::default(), &settings, None, vec![]);
        assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Edge treatment")));
        assert!(result.input.actions.is_empty());
        output.drop_without_applying_deltas();
        let (output, _) = frame(
            &ctx,
            &menus,
            &settings,
            &settings,
            Some("Menu update rejected"),
            vec![],
        );
        let button = text_rect(&output, "Soft edges").center();
        text_rect(&output, "Menu update rejected");
        output.drop_without_applying_deltas();
        let (output, result) = frame(
            &ctx,
            &menus,
            &settings,
            &settings,
            Some("Menu update rejected"),
            click(button),
        );
        assert!(result.input.actions.is_empty());
        assert!(result.blocks_world);
        output.drop_without_applying_deltas();
    }

    #[test]
    fn panel_geometry_is_viewport_relative_and_choice_keyboard_focus_blocks_world() {
        let (menus, settings) = fixture();
        let ctx = context();
        let (output, _) = frame(&ctx, &menus, &settings, &settings, None, vec![]);
        let game = Rect::from_min_size(egui::pos2(24., 24.), egui::vec2(320., 300.));
        for label in [
            "Author menu",
            "A game-owned explanation",
            "Edge treatment",
            "Soft edges",
        ] {
            let rect = text_rect(&output, label);
            assert!(game.contains_rect(rect), "{label}: {rect:?}");
        }
        assert!(text_rect(&output, "Author menu").left() >= game.left() + 16.);
        let button = text_rect(&output, "Soft edges").center();
        output.drop_without_applying_deltas();
        frame(&ctx, &menus, &settings, &settings, None, click(button))
            .0
            .drop_without_applying_deltas();
        let (output, _) = frame(
            &ctx,
            &menus,
            &settings,
            &settings,
            None,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        output.drop_without_applying_deltas();
        // Pointer activation need not transfer keyboard focus, and Escape
        // deliberately clears focus. Reach the actual control with Tab.
        let (output, result) = frame(
            &ctx,
            &menus,
            &settings,
            &settings,
            None,
            vec![egui::Event::Key {
                key: egui::Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        let choice_id = ctx
            .memory(|memory| memory.focused())
            .expect("Tab reaches the choice");
        assert!(
            ctx.read_response(choice_id)
                .expect("The focused widget exists")
                .rect
                .contains_rect(text_rect(&output, "Soft edges")),
            "Keyboard focus must belong to the actual choice control"
        );
        assert!(result.blocks_world);
        output.drop_without_applying_deltas();
        let (output, result) = frame(
            &ctx,
            &menus,
            &settings,
            &settings,
            None,
            vec![egui::Event::Key {
                key: egui::Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(result.blocks_world);
        output.drop_without_applying_deltas();
        let (output, result) = frame(&ctx, &menus, &settings, &settings, None, vec![]);
        text_rect(&output, "Sharp pixels");
        assert!(result.blocks_world);
        output.drop_without_applying_deltas();
        // A keyboard choice is allowed even when the last pointer position is
        // outside Game. Tab may visit the popup Area before its option widgets;
        // keep traversing until the actual option row owns focus.
        let mut focused_option = None;
        for _ in 0..8 {
            let (output, _) = frame(
                &ctx,
                &menus,
                &settings,
                &settings,
                None,
                vec![
                    egui::Event::PointerMoved(egui::pos2(600., 450.)),
                    egui::Event::Key {
                        key: egui::Key::Space,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers: Default::default(),
                    },
                    egui::Event::Key {
                        key: egui::Key::Tab,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers: Default::default(),
                    },
                    egui::Event::Key {
                        key: egui::Key::Tab,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    },
                ],
            );
            if let Some(response) = ctx
                .memory(|memory| memory.focused())
                .and_then(|id| ctx.read_response(id))
                && response.layer_id.order == egui::Order::Foreground
                && response.rect.height() <= 30.
            {
                let soft = response
                    .rect
                    .contains_rect(text_rect(&output, "Soft edges"));
                let sharp = response
                    .rect
                    .contains_rect(text_rect(&output, "Sharp pixels"));
                if soft != sharp {
                    focused_option = Some((
                        if soft { "option/one" } else { "option:two" },
                        response.id,
                        response.rect,
                    ));
                }
            }
            output.drop_without_applying_deltas();
            if focused_option.is_some() {
                break;
            }
        }
        let (option, focused, focused_rect) = focused_option
            .expect("Tab must reach an actual option row rather than its enclosing popup Area");
        let (output, result) = frame(
            &ctx,
            &menus,
            &settings,
            &settings,
            None,
            vec![egui::Event::Key {
                key: egui::Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(
            script_ui_actions(&result.input, "actor.one:[2]").get("edge:choice/[0]"),
            Some(&json!(option)),
            "before={focused:?}/{focused_rect:?}; parent={choice_id:?}; after={:?}; space={}; open={}; input={:?}",
            ctx.memory(|memory| memory.focused()),
            ctx.input(|input| input.key_pressed(egui::Key::Space)),
            egui::Popup::is_any_open(&ctx),
            result.input
        );
        assert!(result.blocks_world);
        output.drop_without_applying_deltas();
    }

    #[test]
    fn instance_identifiers_do_not_alias_and_saved_idle_has_no_status_wall() {
        let (mut menus, settings) = fixture();
        let mut other = menus.values().next().unwrap().clone();
        other[0].position = [16., 140.];
        menus.insert("actor:one.[2]".into(), other);
        let ctx = context();
        let (output, _) = frame(&ctx, &menus, &settings, &settings, None, vec![]);
        assert_eq!(output.shapes.iter().filter(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Soft edges")).count(), 2);
        assert_ne!(
            script_ui_action_key("actor.one:[2]", "edge:choice/[0]"),
            script_ui_action_key("actor:one.[2]", "edge:choice/[0]")
        );
        assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("Applying") || text.galley.text().contains("Not saved") || text.galley.text().contains("Saved"))));
        let second_choice = text_rect(&output, "Soft edges").center();
        output.drop_without_applying_deltas();
        frame(
            &ctx,
            &menus,
            &settings,
            &settings,
            None,
            click(second_choice),
        )
        .0
        .drop_without_applying_deltas();
        let (output, _) = frame(&ctx, &menus, &settings, &settings, None, vec![]);
        let option = text_rect(&output, "Sharp pixels").center();
        output.drop_without_applying_deltas();
        let (output, result) = frame(&ctx, &menus, &settings, &settings, None, click(option));
        assert_eq!(
            script_ui_actions(&result.input, "actor:one.[2]")["edge:choice/[0]"],
            json!("option:two")
        );
        assert!(script_ui_actions(&result.input, "actor.one:[2]").is_empty());
        output.drop_without_applying_deltas();
    }
}
