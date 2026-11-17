//! Read-only explanation of the connected renderer and its authored controls.
use super::*;
use module_engine_core::{RenderCommand, VariantGroup};

pub(super) fn variant_label<'a>(group: &'a VariantGroup, key: &'a str) -> &'a str {
    group
        .options
        .get(key)
        .map_or(key, |option| option.label.as_str())
}

fn targets_pass(command: &RenderCommand, id: &str) -> bool {
    matches!(command, RenderCommand::SetEnabled { pass, .. }
        | RenderCommand::SetParameter { pass, .. } if pass == id)
}

/// These are authored settings, never a claim about an accepted live renderer.
fn renderer_lines(document: &GraphDocument, time: f64) -> Vec<String> {
    let Some(renderer) = active_renderer_node(document) else {
        return vec!["No unique renderer is connected to the root viewport.".into()];
    };
    let mut lines = vec![format!(
        "Connected renderer: {} ({})",
        renderer.label, renderer.id
    )];
    let Some(graph) = renderer
        .subgraph
        .as_deref()
        .filter(|_| renderer.module == "render.vulkan")
    else {
        lines.push("This renderer has no built-in Vulkan pass graph to inspect.".into());
        return lines;
    };
    // GameHost resolves Project commands regardless of the legacy Runtime's
    // initialized guard. Full authoring can preserve an uninitialized policy.
    let commands = if document.game.is_some() || document.engine.initialized {
        module_engine_core::project_commands(document, time)
    } else {
        lines.push(
            "Legacy preview Project exposure is not initialized; base pass settings are used."
                .into(),
        );
        Ok(vec![])
    };
    for (module, name) in [
        ("gpu.fxaa", "AA · Anti-aliasing (FXAA)"),
        ("gpu.ssao", "AO · Ambient occlusion (SSAO)"),
    ] {
        let passes = graph
            .nodes
            .iter()
            .filter(|node| node.module == module)
            .collect::<Vec<_>>();
        if passes.is_empty() {
            lines.push(format!(
                "{name}: built-in pass absent from this renderer graph."
            ));
        }
        for pass in passes {
            let base = pass
                .params
                .get("enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            lines.push(format!(
                "{name} · {}: pass present; base setting {}.",
                pass.id,
                if base { "enabled" } else { "disabled" }
            ));
            if let Ok(commands) = &commands {
                let effective = commands
                    .iter()
                    .fold(base, |enabled, command| match command {
                        ControlCommand::Render(RenderCommand::SetEnabled { pass: id, enabled })
                            if id == &pass.id =>
                        {
                            *enabled
                        }
                        _ => enabled,
                    });
                lines.push(format!(
                    "Authored enable after Project conditions at reference time {time:.2} s: {}.",
                    if effective { "enabled" } else { "disabled" }
                ));
            }
            for (id, group) in &document.engine.exposure.variants {
                if !group.options.values().any(|option| {
                    option
                        .commands
                        .iter()
                        .any(|command| targets_pass(command, &pass.id))
                }) {
                    continue;
                }
                let selected = document.project.variants.get(id).unwrap_or(&group.default);
                let available = group
                    .options
                    .values()
                    .map(|option| option.label.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!(
                    "{}: selected fallback {}; available: {available}.",
                    group.label,
                    variant_label(group, selected)
                ));
                let count = document
                    .project
                    .rules
                    .iter()
                    .filter(|rule| rule.group == *id)
                    .count();
                if count > 0 {
                    lines.push(format!("{count} condition(s) can override this selection; the first matching condition wins."));
                }
            }
        }
    }
    if let Err(error) = commands {
        lines.push(format!(
            "Authored Project settings cannot be resolved: {error}"
        ));
    }
    lines
}

impl Editor {
    /// Project Settings keeps idle content free of operation explanations.
    /// Rejected commits are retained in this pane's temporary UI state.
    pub(super) fn project_settings_feedback(
        &mut self,
        ui: &mut egui::Ui,
        local_error: &mut Option<String>,
    ) {
        let activities = if self.document.game.is_some() {
            self.game_workbench.activity_snapshots()
        } else {
            self.preview_builder
                .activity_snapshot()
                .into_iter()
                .collect()
        };
        let pending = if self.document.game.is_some() {
            self.game_workbench.has_pending_preparation()
        } else {
            self.preview_builder.is_pending()
        };
        if self.has_unsaved_work() || pending {
            ui.horizontal_wrapped(|ui| {
                if self.has_unsaved_work() {
                    ui.label("Unsaved").on_hover_text("Save Project · Ctrl+S");
                }
                if pending {
                    ui.spinner();
                    ui.label("Preparing…");
                }
            });
        }
        let failure = activities
            .iter()
            .find(|activity| activity.state == crate::activity::ActivityState::Failed)
            .map(|activity| {
                activity
                    .error
                    .clone()
                    .unwrap_or_else(|| activity.detail.clone())
            });
        if let Some(error) = local_error.clone().or(failure) {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(error.lines().next().unwrap_or(&error))
                        .color(Color32::from_rgb(255, 159, 159)),
                )
                .wrap(),
            );
            ui.horizontal_wrapped(|ui| {
                ui.label("Correct the setting or undo the last edit.");
                if ui
                    .add_enabled(
                        self.history_dirty || self.history.can_undo(),
                        egui::Button::new("Undo edit"),
                    )
                    .clicked()
                {
                    let before = self.document.clone();
                    self.undo();
                    *local_error = if self.document != before {
                        None
                    } else {
                        Some(self.status.clone())
                    };
                }
                ui.menu_button("Details", |ui| {
                    ui.add(egui::Label::new(error).wrap().selectable(true));
                });
            });
        }
    }

    pub(super) fn render_settings_summary(&self, ui: &mut egui::Ui, expanded: bool) {
        egui::CollapsingHeader::new("Rendering · passes and Project choices")
            .id_salt("render-settings-summary")
            .default_open(expanded)
            .show(ui, |ui| {
                ui.label("AA smooths jagged edges. AO darkens occluded areas where surfaces are close.");
                // The legacy preview clock does not track Game execution.
                let reference_time = if self.document.game.is_some() { 0. } else { self.time };
                for line in renderer_lines(&self.document, reference_time) {
                    ui.add(egui::Label::new(line).wrap());
                }
                if let Some(graph) = active_vulkan_node(&self.document).and_then(|node| node.subgraph.as_deref()) {
                    egui::CollapsingHeader::new("Passes present in the renderer graph").show(ui, |ui| {
                        for node in &graph.nodes {
                            if !matches!(node.module.as_str(), "interface.input" | "interface.output") {
                                let label = self.registry.descriptor(&node.module).map_or(node.label.as_str(), |descriptor| descriptor.label.as_str());
                                ui.add(egui::Label::new(format!("{label} · {}", node.id)).wrap());
                            }
                        }
                    });
                }
                ui.label("These are authored settings. Graph presence and resolved choices do not establish live readiness.");
                ui.label("Game conditions use live Game time; permitted runtime render commands can also override these settings. The reference above is not the current rendered state.");
            });
    }

    pub(super) fn render_settings_feedback(&self, ui: &mut egui::Ui) {
        let activities = if self.document.game.is_some() {
            self.game_workbench.activity_snapshots()
        } else {
            self.preview_builder
                .activity_snapshot()
                .into_iter()
                .collect()
        };
        let pending = if self.document.game.is_some() {
            self.game_workbench.has_pending_preparation()
        } else {
            self.preview_builder.is_pending()
        };
        if let Some(failed) = activities
            .iter()
            .find(|activity| activity.state == crate::activity::ActivityState::Failed)
        {
            ui.colored_label(Color32::from_rgb(255, 159, 159), "Preparation failed; the candidate was not applied. The previous working version is retained when available.");
            if let Some(error) = &failed.error {
                ui.add(egui::Label::new(error).wrap().selectable(true));
            }
            ui.label(
                "Correct the rejected edit or use Undo, then inspect the result in Game/Scene.",
            );
        } else if pending {
            ui.label("Preparation is in progress; the new authored choice is not yet confirmed in the image.");
        } else {
            ui.label("No preparation is currently running. Inspect the rendered result to confirm the authored choice.");
        }
        ui.label(if self.document.game.is_some() {
            "Open Game or Scene to see the result. Graphics changes refresh automatically, including in paused Play. Execution graph changes require Stop."
        } else {
            "Open the preview to see the result. Renderer changes prepare in the background."
        });
        ui.label(if self.has_unsaved_work() {
            "Unsaved author edits. Save Project (Ctrl+S) before exporting this choice."
        } else {
            "The project is saved. Saving and successful graphics preparation are separate states."
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_engine_core::{Comparison, RuleCondition, VariantRule};
    use serde_json::json;

    // Construct the ordinary software Editor first: read-only UI assertions
    // must not start a Vulkan preparation job just to inspect authored data.
    fn inspection_editor(document: GraphDocument) -> Editor {
        let mut editor = Editor::new(
            module_engine_core::demo_document(WorldKind::Objects),
            project::source_root().join("unused-render-summary.json"),
        );
        editor.runtime = None;
        editor.policy_baseline = document.clone();
        editor.history = SnapshotHistory::new(document.clone(), 200);
        editor.document = document;
        editor
    }

    #[test]
    fn summary_uses_command_targets_custom_labels_and_conditions_without_editing() {
        let mut document = crate::host::gpu_document(WorldKind::Objects, RenderPath::Forward);
        crate::host::ensure_authoring_configuration(&mut document);
        let mut group = document
            .engine
            .exposure
            .variants
            .remove("antialiasing")
            .unwrap();
        group.label = "Edge treatment".into();
        group.options.get_mut("off").unwrap().label = "Keep sharp pixels".into();
        document
            .engine
            .exposure
            .variants
            .insert("custom.edge.mode".into(), group);
        document.project.variants.remove("antialiasing");
        document
            .project
            .variants
            .insert("custom.edge.mode".into(), "off".into());
        document.project.rules.push(VariantRule {
            group: "custom.edge.mode".into(),
            variant: "fxaa".into(),
            condition: RuleCondition::Time {
                op: Comparison::GreaterOrEqual,
                value: 2.,
            },
        });
        let before = document.clone();
        let early = renderer_lines(&document, 0.).join("\n");
        let later = renderer_lines(&document, 3.).join("\n");
        assert!(early.contains("base setting enabled"));
        assert!(early.contains("reference time 0.00 s: disabled"));
        assert!(later.contains("reference time 3.00 s: enabled"));
        assert!(early.contains("selected fallback Keep sharp pixels"));
        assert!(early.contains("1 condition(s)"));
        assert_eq!(document, before);
    }

    #[test]
    fn uninitialized_game_policy_resolves_project_commands_but_legacy_keeps_base_setting() {
        let mut document = crate::host::gpu_document(WorldKind::Objects, RenderPath::Forward);
        crate::host::ensure_authoring_configuration(&mut document);
        document.engine.initialized = false;
        document
            .project
            .variants
            .insert("antialiasing".into(), "off".into());
        document.game = Some(module_engine_core::GameProject::empty());
        let game = document.game.as_ref().unwrap();
        game.resolve_scene(&game.entry_scene).unwrap();
        module_engine_core::validate_settings(&document, &crate::host::registry().unwrap())
            .unwrap();
        let before = document.clone();
        let game_summary = renderer_lines(&document, 0.).join("\n");
        assert!(game_summary.contains("base setting enabled"));
        assert!(game_summary.contains("reference time 0.00 s: disabled"));
        assert!(!game_summary.contains("not initialized; base pass settings are used"));
        assert_eq!(document, before);

        document.game = None;
        let legacy_summary = renderer_lines(&document, 0.).join("\n");
        assert!(legacy_summary.contains("Legacy preview Project exposure is not initialized"));
        assert!(legacy_summary.contains("reference time 0.00 s: enabled"));
    }

    #[test]
    fn summary_distinguishes_absent_disabled_and_unused_alternative_renderer() {
        let mut document = crate::host::gpu_document(WorldKind::Objects, RenderPath::Forward);
        let mut alternative = active_vulkan_node(&document).unwrap().clone();
        alternative.id = "unused-renderer".into();
        document.root.nodes.push(alternative);
        let graph = document
            .root
            .nodes
            .iter_mut()
            .find(|node| node.id == "renderer")
            .unwrap()
            .subgraph
            .as_deref_mut()
            .unwrap();
        graph
            .nodes
            .iter_mut()
            .find(|node| node.module == "gpu.fxaa")
            .unwrap()
            .params = json!({"enabled":false});
        document.engine.initialized = false;
        let disabled = renderer_lines(&document, 0.).join("\n");
        assert!(disabled.contains("pass present; base setting disabled"));
        assert!(!disabled.contains("unused-renderer"));
        let graph = document
            .root
            .nodes
            .iter_mut()
            .find(|node| node.id == "renderer")
            .unwrap()
            .subgraph
            .as_deref_mut()
            .unwrap();
        graph
            .nodes
            .retain(|node| !matches!(node.module.as_str(), "gpu.fxaa" | "gpu.ssao"));
        let absent = renderer_lines(&document, 0.).join("\n");
        assert!(absent.contains("Anti-aliasing (FXAA): built-in pass absent"));
        assert!(absent.contains("Ambient occlusion (SSAO): built-in pass absent"));
        assert!(!absent.contains("pass present"));
    }

    #[test]
    fn narrow_summary_inspection_preserves_document_history_and_dirty_state() {
        let document = crate::host::gpu_document(WorldKind::Objects, RenderPath::Forward);
        let editor = inspection_editor(document);
        let before = editor.document.clone();
        let history = editor.history.entries().len();
        let dirty = editor.dirty;
        let ctx = egui::Context::default();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(480., 900.),
                )),
                ..Default::default()
            },
            |ui| editor.render_settings_summary(ui, true),
        );
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("base setting"))));
        output.drop_without_applying_deltas();
        assert_eq!(editor.document, before);
        assert_eq!(editor.history.entries().len(), history);
        assert_eq!(editor.dirty, dirty);
    }

    #[test]
    fn authored_label_is_used_by_the_project_variant_control_at_narrow_width() {
        let mut document = crate::host::gpu_document(WorldKind::Objects, RenderPath::Forward);
        crate::host::ensure_authoring_configuration(&mut document);
        document
            .engine
            .exposure
            .variants
            .get_mut("antialiasing")
            .unwrap()
            .options
            .get_mut("fxaa")
            .unwrap()
            .label = "Smooth edges".into();
        let mut editor = inspection_editor(document);
        let before = editor.document.clone();
        let history = editor.history.entries().len();
        let dirty = editor.dirty;
        let ctx = egui::Context::default();
        super::super::configure_style(&ctx);
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(480., 300.),
                )),
                ..Default::default()
            },
            |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| editor.project_settings_panel(ui));
            },
        );
        let combo = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Smooth edges" => Some((
                    shape.clip_rect,
                    egui::Rect::from_min_size(text.pos, text.galley.size()),
                )),
                _ => None,
            })
            .expect("AA choice must be rendered in the initial compact viewport");
        assert!(
            combo.0.contains_rect(combo.1),
            "AA choice text must be fully visible without scrolling: clip {:?}, text {:?}",
            combo.0,
            combo.1
        );
        output.drop_without_applying_deltas();
        assert_eq!(editor.document, before);
        assert_eq!(editor.history.entries().len(), history);
        assert_eq!(editor.dirty, dirty);
    }

    #[test]
    #[ignore = "Native GPU history restoration; run sequentially with render checks"]
    fn project_variant_combo_click_commits_and_undo_redo_restore_the_authored_choice() {
        let mut document = crate::host::gpu_document(WorldKind::Objects, RenderPath::Forward);
        crate::host::ensure_authoring_configuration(&mut document);
        assert!(
            document
                .engine
                .exposure
                .variants
                .contains_key("antialiasing")
        );
        let mut editor = Editor::new(
            document,
            project::source_root().join("unused-project-render-choice.json"),
        );
        let before = editor.document.clone();
        let ctx = egui::Context::default();
        ctx.all_styles_mut(|style| style.animation_time = 0.);
        let frame = |editor: &mut Editor, events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(480., 1100.),
                    )),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ui| editor.project_settings_panel(ui),
            )
        };
        let center = |output: &egui::FullOutput, label: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing control: {label}"))
        };
        let click = |pos| {
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
        };
        let output = frame(&mut editor, vec![]);
        let combo = center(&output, "FXAA");
        output.drop_without_applying_deltas();
        frame(&mut editor, click(combo)).drop_without_applying_deltas();
        let output = frame(&mut editor, vec![]);
        let off = center(&output, "Off");
        output.drop_without_applying_deltas();
        frame(&mut editor, click(off)).drop_without_applying_deltas();
        assert_eq!(
            editor.document.project.variants["antialiasing"], "off",
            "{}",
            editor.status
        );
        assert!(editor.dirty);
        let selected = editor.document.clone();
        editor.flush_history();
        editor.undo();
        assert_eq!(editor.document, before, "{}", editor.status);
        editor.redo();
        assert_eq!(editor.document, selected, "{}", editor.status);
    }
}
