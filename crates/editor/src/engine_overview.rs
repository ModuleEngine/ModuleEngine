//! Discovery of source-owned capabilities and preview of ARPG audio changes.
use super::{Editor, EditorType, Graph, WorkspaceKind, egui, panes::PaneMemory};
use module_engine_core::{
    CapabilityConfiguration as Route, CapabilityDependencyKind, CapabilityFact,
    CapabilityInventory, CapabilityKind, CapabilityOwner, CapabilityRecord,
};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct OverviewCache {
    inventory: Option<Arc<CapabilityInventory>>,
    basis: Option<Result<String, String>>,
    audio_feature: Option<Result<bool, String>>,
    invalidated_at: Option<f64>,
}

impl OverviewCache {
    pub(super) fn invalidate(&mut self, now: f64) {
        self.invalidated_at = Some(now);
    }
    fn needs_refresh(&self, now: f64) -> bool {
        self.inventory.is_none() || self.invalidated_at.is_some_and(|at| now - at >= 0.15)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct CapabilityKey {
    id: String,
    kind: CapabilityKind,
}
impl From<&CapabilityRecord> for CapabilityKey {
    fn from(record: &CapabilityRecord) -> Self {
        Self {
            id: record.id.clone(),
            kind: record.metadata.kind,
        }
    }
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct OverviewPane {
    selected: Option<CapabilityKey>,
    search: String,
    owner: CapabilityOwner,
    detail_open: bool,
}
impl Default for OverviewPane {
    fn default() -> Self {
        Self {
            selected: None,
            search: String::new(),
            owner: CapabilityOwner::Engine,
            detail_open: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum OverviewTarget {
    Graph {
        scope: Vec<String>,
        node: Option<String>,
    },
    Rust(String),
    Hlsl(String),
    AudioSystem(String),
    Window(EditorType),
}
impl OverviewTarget {
    fn editor(&self) -> EditorType {
        match self {
            Self::Graph { .. } => EditorType::ModuleGraph,
            Self::Rust(_) => EditorType::RustCustom,
            Self::Hlsl(_) => EditorType::Hlsl,
            Self::AudioSystem(_) => EditorType::GameSystems,
            Self::Window(editor) => *editor,
        }
    }
}

fn owner_label(owner: CapabilityOwner) -> &'static str {
    match owner {
        CapabilityOwner::Engine => "Technical engine subsystems",
        CapabilityOwner::Gameplay => "Reusable gameplay packages",
        CapabilityOwner::Project => "Game-specific rules and tools",
        CapabilityOwner::Unknown => "Ownership not described",
    }
}
fn kind_label(kind: CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::SourceContract => "Source contract",
        CapabilityKind::WorldStorage => "World storage",
        CapabilityKind::Renderer => "Renderer",
        CapabilityKind::RenderPass => "Render pass",
        CapabilityKind::ExecutableSystem => "Executable system",
        CapabilityKind::ComponentData => "Component data",
        CapabilityKind::HostContribution => "Host contribution",
        CapabilityKind::Library => "Helper library",
        CapabilityKind::Service => "Host service",
        CapabilityKind::EditorTool => "Editor tool",
    }
}
fn fact_label(fact: &CapabilityFact) -> &'static str {
    match fact.value {
        Some(true) => "Yes",
        Some(false) => "No",
        None => "Unknown",
    }
}
fn participation_label(record: &CapabilityRecord) -> &'static str {
    match record.participation.value {
        Some(true) => "Participating",
        Some(false) => "Not selected",
        None => "Unknown participation",
    }
}

/// Stable scope IDs, not indices: equal local IDs in alternate renderers are distinct.
fn graph_at<'a>(root: &'a Graph, scope: &[String]) -> Option<&'a Graph> {
    let mut graph = root;
    for id in scope {
        graph = graph
            .nodes
            .iter()
            .find(|node| node.id == *id)?
            .subgraph
            .as_deref()?;
    }
    Some(graph)
}
fn occurrences(root: &Graph, module: &str) -> Vec<(Vec<String>, String)> {
    fn visit(
        graph: &Graph,
        module: &str,
        scope: &mut Vec<String>,
        out: &mut Vec<(Vec<String>, String)>,
    ) {
        for node in &graph.nodes {
            if node.module == module {
                out.push((scope.clone(), node.id.clone()));
            }
            if let Some(inner) = node.subgraph.as_deref() {
                scope.push(node.id.clone());
                visit(inner, module, scope, out);
                scope.pop();
            }
        }
    }
    let mut out = vec![];
    visit(root, module, &mut vec![], &mut out);
    out
}
fn scope_label(scope: &[String]) -> String {
    if scope.is_empty() {
        "Root".into()
    } else {
        format!("Root / {}", scope.join(" / "))
    }
}

impl Editor {
    fn refresh_overview(&mut self) {
        self.overview.basis = Some(
            module_engine_project::EngineProfile::from_document(&self.document)
                .map(|profile| format!("{} @ {}", profile.identity.id, profile.identity.version)),
        );
        let root = self
            .distribution
            .as_ref()
            .map(|p| p.package_root.clone())
            .unwrap_or_else(crate::project::source_root);
        let result = crate::host::composition_at(&root).and_then(|composition| {
            let services = module_engine_player::service_registry_at(
                self.project_asset_root().join(".user/editor-play-state"),
            )?;
            Ok(module_engine_player::capabilities::capability_inventory(
                &self.document,
                &composition,
                &services,
            ))
        });
        let mut inventory = result.unwrap_or_else(|error| CapabilityInventory {
            records: vec![],
            diagnostics: vec![format!("Capability inspection failed: {error}")],
        });
        self.overview.audio_feature = inventory
            .records
            .iter()
            .any(|record| {
                record.id == "arpg.presentation" && record.participation.value == Some(true)
            })
            .then(|| {
                crate::game_package::selected_features(&self.document, &root)
                    .map(|features| features.contains("audio"))
            });
        inventory.records.extend(self.module_panel_capabilities());
        inventory.records.sort_by(|a, b| {
            (
                a.participation.value != Some(true),
                a.metadata.kind != CapabilityKind::Renderer,
                a.metadata.kind == CapabilityKind::RenderPass,
                &a.metadata.label,
                &a.id,
            )
                .cmp(&(
                    b.participation.value != Some(true),
                    b.metadata.kind != CapabilityKind::Renderer,
                    b.metadata.kind == CapabilityKind::RenderPass,
                    &b.metadata.label,
                    &b.id,
                ))
        });
        self.overview.inventory = Some(Arc::new(inventory));
        self.overview.invalidated_at = None;
    }

    pub(super) fn engine_overview_panel(&mut self, ui: &mut egui::Ui, pane: &mut OverviewPane) {
        if self.overview.needs_refresh(self.last_tick) {
            self.refresh_overview();
        }
        let inventory = self.overview.inventory.as_ref().unwrap().clone();
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("Working configuration")
                    .size(20.)
                    .strong(),
            );
            ui.label(if self.can_author_engine() {
                "Full · checked sources"
            } else {
                "Project only · Engine locked"
            });
        });
        ui.add(egui::Label::new(format!("{} · {}", self.document.name, self.path)).truncate())
            .on_hover_text(&self.path);
        if let Some(basis) = &self.overview.basis {
            match basis {
                Ok(basis) => {
                    ui.small(format!("Configuration basis: {basis}"));
                }
                Err(error) => {
                    ui.colored_label(
                        super::editor_ui::SECONDARY,
                        format!("Configuration basis unknown: {error}"),
                    );
                }
            }
        }
        ui.add_space(16.);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut pane.search)
                    .hint_text("Find a subsystem or module")
                    .desired_width((ui.available_width() - 100.).max(80.)),
            );
            if ui.button("Refresh").clicked() {
                self.refresh_overview();
            }
        });
        let inventory = self
            .overview
            .inventory
            .as_ref()
            .unwrap_or(&inventory)
            .clone();
        ui.add_sized(
            [ui.available_width(), 20.],
            egui::Label::new(
                egui::RichText::new(if self.overview.invalidated_at.is_some() {
                    "Updating authored state… previous evidence shown"
                } else {
                    "Authored selection · runtime readiness is separate"
                })
                .small(),
            )
            .truncate(),
        );
        if pane.selected.as_ref().is_none_or(|key| {
            !inventory
                .records
                .iter()
                .any(|r| CapabilityKey::from(r) == *key && r.metadata.owner == pane.owner)
        }) {
            let active = super::active_renderer_node(&self.document);
            pane.selected = inventory
                .records
                .iter()
                .find(|r| {
                    r.metadata.owner == pane.owner
                        && r.metadata.kind == CapabilityKind::Renderer
                        && active.is_some_and(|n| n.module == r.id)
                })
                .or_else(|| {
                    inventory
                        .records
                        .iter()
                        .find(|r| r.metadata.owner == pane.owner)
                })
                .map(CapabilityKey::from);
        }
        let groups = [
            (CapabilityOwner::Engine, "Technical systems"),
            (CapabilityOwner::Gameplay, "Gameplay packages"),
            (CapabilityOwner::Project, "Game project"),
            (CapabilityOwner::Unknown, "Unknown owner"),
        ];
        let width = ui.available_width();
        let wide = width >= 1000.;
        let compact = width < 700.;
        if !wide {
            ui.horizontal_wrapped(|ui| {
                for (owner, label) in groups {
                    let count = inventory
                        .records
                        .iter()
                        .filter(|r| r.metadata.owner == owner)
                        .count();
                    if owner == CapabilityOwner::Unknown && count == 0 {
                        continue;
                    }
                    if ui
                        .selectable_value(&mut pane.owner, owner, format!("{label} ({count})"))
                        .clicked()
                    {
                        pane.selected = None;
                        pane.detail_open = false;
                    }
                }
            });
        }
        let body = ui.available_rect_before_wrap();
        let nav = if wide { 184. } else { 0. };
        let gap = 12.;
        let list_width = if compact { body.width() } else { 280. };
        let mut x = body.left();
        if wide {
            let rect = egui::Rect::from_min_size(body.min, egui::vec2(nav, body.height()));
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("overview-groups")
                    .max_rect(rect),
                |ui| {
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    for (owner, label) in groups {
                        let count = inventory
                            .records
                            .iter()
                            .filter(|r| r.metadata.owner == owner)
                            .count();
                        if owner == CapabilityOwner::Unknown && count == 0 {
                            continue;
                        }
                        if ui
                            .add_sized(
                                [nav, 40.],
                                egui::Button::new(format!("{label} ({count})"))
                                    .selected(pane.owner == owner),
                            )
                            .clicked()
                        {
                            pane.owner = owner;
                            pane.selected = None;
                        }
                    }
                },
            );
            x += nav + gap;
        }
        if !compact || !pane.detail_open {
            let rect = egui::Rect::from_min_size(
                egui::pos2(x, body.top()),
                egui::vec2(list_width, body.height()),
            );
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("overview-list")
                    .max_rect(rect),
                |ui| {
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    egui::ScrollArea::vertical()
                        .id_salt("capabilities")
                        .show(ui, |ui| {
                            let search = pane.search.to_lowercase();
                            let records: Vec<_> = inventory
                                .records
                                .iter()
                                .filter(|r| {
                                    r.metadata.owner == pane.owner
                                        && (search.is_empty()
                                            || r.metadata.label.to_lowercase().contains(&search)
                                            || r.id.to_lowercase().contains(&search)
                                            || r.metadata.purpose.to_lowercase().contains(&search))
                                })
                                .collect();
                            if records.is_empty() {
                                ui.label("No matching capabilities in this group.");
                            }
                            for record in records {
                                let key = CapabilityKey::from(record);
                                let selected = pane.selected.as_ref() == Some(&key);
                                ui.push_id((&record.id, kind_label(record.metadata.kind)), |ui| {
                                    let response = super::editor_ui::two_line_row(
                                        ui,
                                        &record.metadata.label,
                                        &format!(
                                            "{} · {}",
                                            kind_label(record.metadata.kind),
                                            participation_label(record)
                                        ),
                                        selected,
                                    );
                                    if selected {
                                        ui.painter().line_segment(
                                            [response.rect.left_top(), response.rect.left_bottom()],
                                            egui::Stroke::new(3., super::editor_ui::ACCENT),
                                        );
                                    }
                                    if response.on_hover_text(&record.metadata.purpose).clicked() {
                                        pane.selected = Some(key);
                                        pane.detail_open = true;
                                    }
                                });
                            }
                        });
                },
            );
            x += list_width + gap;
        }
        if !compact || pane.detail_open {
            let rect = egui::Rect::from_min_max(
                egui::pos2(if compact { body.left() } else { x }, body.top()),
                body.max,
            );
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("overview-detail")
                    .max_rect(rect),
                |ui| {
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    if compact && ui.button("Back to systems").clicked() {
                        pane.detail_open = false;
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("capability-detail")
                        .show(ui, |ui| {
                            if let Some(record) = inventory
                                .records
                                .iter()
                                .find(|r| pane.selected.as_ref() == Some(&CapabilityKey::from(*r)))
                            {
                                self.overview_detail(ui, record);
                            } else {
                                ui.label(
                                    "Select a subsystem to inspect its role and available actions.",
                                );
                            }
                            if !inventory.diagnostics.is_empty() {
                                super::editor_ui::section(ui, "Inventory diagnostics");
                                for diagnostic in &inventory.diagnostics {
                                    ui.colored_label(egui::Color32::YELLOW, diagnostic);
                                }
                            }
                        });
                },
            );
        }
    }

    fn overview_detail(&mut self, ui: &mut egui::Ui, record: &CapabilityRecord) {
        ui.heading(&record.metadata.label);
        ui.monospace(&record.id);
        ui.label(&record.metadata.purpose);
        ui.add_space(12.);
        if matches!(
            record.id.as_str(),
            module_engine_arpg::AUDIO_SYSTEM | module_engine_arpg::AUDIO_SERVICE
        ) {
            self.audio_overview(ui);
        }
        let (mut actions, notes) = self.overview_actions(record);
        if record.metadata.kind == CapabilityKind::Renderer
            && let Some(index) = actions
                .iter()
                .position(|(label, _)| label.starts_with("Open render graph:"))
        {
            actions.swap(0, index);
        }
        for (index, (label, target)) in actions.into_iter().enumerate() {
            let validation = self.overview_target_valid(&target);
            let button = if index == 0 {
                egui::Button::new(
                    egui::RichText::new(label).color(egui::Color32::from_rgb(14, 34, 36)),
                )
                .fill(super::editor_ui::ACCENT)
            } else {
                egui::Button::new(label)
            };
            let response = ui.add_enabled(validation.is_ok(), button);
            if index == 0 && response.has_focus() {
                ui.painter().rect_stroke(
                    response.rect.shrink(2.),
                    4,
                    egui::Stroke::new(2., egui::Color32::from_rgb(14, 34, 36)),
                    egui::StrokeKind::Inside,
                );
            }
            if response.clicked() {
                self.overview_navigation = Some(target.clone());
            }
            if let Err(reason) = validation {
                response.on_disabled_hover_text(reason);
            }
        }
        for note in notes {
            ui.small(note);
        }
        if record.metadata.kind == CapabilityKind::Renderer {
            self.render_settings_summary(ui, true);
            ui.label("Full with checked matching sources can edit the render graph and Engine Exposure. Project Settings selects the prepared variants; changing layouts never changes these rights.");
            self.render_settings_feedback(ui);
        }

        ui.label(format!(
            "{} · {}",
            owner_label(record.metadata.owner),
            kind_label(record.metadata.kind)
        ));
        ui.label(format!(
            "Implementation: {}",
            record
                .metadata
                .source_package
                .as_deref()
                .unwrap_or("Source did not declare a package")
        ));
        ui.label(format!(
            "Contract version: {}",
            record
                .version
                .as_deref()
                .unwrap_or("Not separately versioned")
        ));
        super::editor_ui::section(ui, "Evidence (not runtime readiness)");
        ui.label(format!(
            "Installed: {}",
            if record.installed { "Yes" } else { "No" }
        ));
        for (label, fact) in [
            ("Available", &record.available),
            ("Participating", &record.participation),
        ] {
            ui.label(format!("{label}: {}", fact_label(fact)));
            ui.small(&fact.evidence);
        }
        if !record.dependencies.is_empty() {
            super::editor_ui::section(ui, "Dependencies");
            for dependency in &record.dependencies {
                let relation = match dependency.kind {
                    CapabilityDependencyKind::ActiveModule => "Requires active module",
                    CapabilityDependencyKind::PrecedingSystem => "Runs after system",
                    CapabilityDependencyKind::Library => "Uses helper library",
                    CapabilityDependencyKind::ActivatedByAny => "Activated by any of",
                };
                ui.label(format!("{relation}: {}", dependency.id));
            }
        }
        if !record.provided_components.is_empty() || !record.consumed_components.is_empty() {
            egui::CollapsingHeader::new("Component contracts").show(ui, |ui| {
                for id in &record.provided_components {
                    ui.label(format!("Provides: {id}"));
                }
                for id in &record.consumed_components {
                    ui.label(format!("Consumes (static): {id}"));
                }
            });
        }
        if record.metadata.configuration.contains(&Route::NativeSource) {
            ui.small("Native source/build configuration: edit the implementation package and rebuild. This is not a live module toggle.");
        }
    }

    fn audio_overview(&mut self, ui: &mut egui::Ui) {
        use crate::game_ui::audio_wav_label;
        use module_engine_arpg::{
            AUDIO_SERVICE, AUDIO_SYSTEM, audio_cue_event, audio_nodes, has_audio_request,
        };

        let Some(feature) = &self.overview.audio_feature else {
            return;
        };
        let Some(game) = &self.document.game else {
            return;
        };
        let nodes = audio_nodes(game)
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        let requested = has_audio_request(game);
        let manifest = requested
            .then(|| {
                module_engine_project::AssetManifest::from_value(&self.document.asset_manifest).ok()
            })
            .flatten();
        super::editor_ui::section(ui, "ARPG audio dependencies");
        if requested {
            for (cue, wav) in &game.audio_cues {
                let wav = audio_wav_label(manifest.as_ref(), wav);
                let event = audio_cue_event(cue);
                ui.label(if event == module_engine_arpg::SOUND_EVENT {
                    format!("Sound ({cue}) → {wav}")
                } else {
                    format!("{event} → Sound ({cue}) → {wav}")
                });
            }
            ui.label(format!("WAV playback requested from {AUDIO_SERVICE}."));
        } else {
            ui.label("No ARPG WAV playback request.");
            if !game.audio_cues.is_empty() {
                ui.label("Cue bindings are retained in the project.");
            }
        }
        if self.overview.invalidated_at.is_some() {
            ui.label("Player build feature: refresh pending");
        } else {
            match feature {
                Ok(true) => {
                    ui.label("Player build feature: audio selected");
                }
                Ok(false) => {
                    ui.label("Player build feature: audio omitted");
                }
                Err(error) => {
                    ui.label(format!("Player build feature unknown: {error}"));
                }
            }
        }
        for id in &nodes {
            let target = OverviewTarget::AudioSystem(id.clone());
            let valid = self.overview_target_valid(&target);
            let response = ui.add_enabled(
                valid.is_ok(),
                egui::Button::new(format!("Show {AUDIO_SYSTEM} · {id}")),
            );
            if response.clicked() {
                self.overview_navigation = Some(target);
            }
            if let Err(error) = valid {
                response.on_disabled_hover_text(error);
            }
        }
        if nodes.is_empty() {
            return;
        }

        let permission = self.audio_edit_allowed(true);
        let mut disable = false;
        ui.horizontal_wrapped(|ui| {
            if requested {
                disable = ui
                    .add_enabled(
                        permission.is_ok(),
                        egui::Button::new("Disable WAV playback request"),
                    )
                    .clicked();
            }
            if ui
                .add_enabled(
                    permission.is_ok(),
                    egui::Button::new("Remove audio system…"),
                )
                .clicked()
            {
                self.workflow.audio_removal = true;
                ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
            }
        });
        if let Err(error) = permission {
            ui.small(error);
        }

        if disable && let Err(error) = self.edit_audio_composition(false) {
            self.status = error;
        }
    }

    pub(super) fn draw_audio_removal_dialog(&mut self, ctx: &egui::Context) {
        use crate::game_ui::audio_wav_label;
        use module_engine_arpg::{audio_cue_event, has_audio_request};

        if !self.workflow.audio_removal {
            return;
        }
        let Some(game) = &self.document.game else {
            self.workflow.audio_removal = false;
            return;
        };
        let requested = has_audio_request(game);
        let mut remove = false;
        let modal = egui::Modal::new(egui::Id::new("remove-audio-system")).show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 32.).clamp(160., 400.));
                ui.heading("Remove ARPG audio system");
                if requested {
                    let manifest = module_engine_project::AssetManifest::from_value(&self.document.asset_manifest).ok();
                    ui.label("These events still request WAV playback. Removing their system would stop the sounds.");
                    egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 160.).clamp(40., 220.)).show(ui, |ui| {
                        for (cue, wav) in &game.audio_cues {
                            ui.label(format!("{} ({cue}) → {}", audio_cue_event(cue), audio_wav_label(manifest.as_ref(), wav)));
                        }
                    });
                    ui.label("Disable the WAV playback request before removing the system.");
                } else {
                    ui.label("This removes the ARPG sound event system and its ordering links. Cue bindings and WAV files stay in the project. Undo restores the system.");
                }
                ui.horizontal_wrapped(|ui| {
                    if !requested {
                        remove = ui.button("Remove audio system").clicked();
                    }
                    ui.button("Cancel").clicked()
                }).inner
            });
        if remove || modal.inner || modal.should_close() {
            self.workflow.audio_removal = false;
        }
        if remove && let Err(error) = self.edit_audio_composition(true) {
            self.status = error;
        }
    }

    pub(super) fn audio_edit_allowed(&self, engine: bool) -> Result<(), String> {
        if engine && !self.can_author_engine() {
            return Err("Full profile and checked matching sources are required".into());
        }
        if self.playing || self.game_workbench.is_play_session() {
            return Err("Stop Play before changing audio".into());
        }
        if self.workflow.blocks_document_edits() {
            return Err("Finish the current project operation before changing audio".into());
        }
        if self.json_dirty
            || self.game_workbench.has_graph_draft()
            || self.scene_gesture_before.is_some()
        {
            return Err("Finish or discard authoring drafts before changing audio".into());
        }
        self.require_resolved_module_tools("changing audio")
    }

    fn edit_audio_composition(&mut self, remove: bool) -> Result<(), String> {
        use module_engine_arpg::{AUDIO_SYSTEM, PLAYBACK_ENABLED, audio_nodes};

        self.audio_edit_allowed(true)?;
        let mut candidate = self.document.clone();
        let game = candidate.game.as_mut().ok_or("No authored game project")?;
        let ids = audio_nodes(game)
            .map(|node| node.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        for stage in &mut game.execution.stages {
            if remove {
                stage.nodes.retain(|node| node.module != AUDIO_SYSTEM);
                for node in &mut stage.nodes {
                    node.after.retain(|id| !ids.contains(id));
                }
            } else {
                for node in stage
                    .nodes
                    .iter_mut()
                    .filter(|node| node.module == AUDIO_SYSTEM)
                {
                    if node.params.is_null() {
                        node.params = serde_json::json!({});
                    }
                    node.params
                        .as_object_mut()
                        .ok_or("The audio system needs object parameters to disable playback")?
                        .insert(PLAYBACK_ENABLED.into(), serde_json::Value::Bool(false));
                }
            }
        }
        crate::game_ui::validate_audio_removal(&self.document, &candidate)?;
        self.validate_candidate(&candidate, true)?;
        if candidate != self.document {
            self.flush_history();
            self.document = candidate;
            self.policy_baseline = self.document.clone();
            self.changed(false);
            self.flush_history();
            self.status = if remove {
                "ARPG audio system removed. Undo restores it."
            } else {
                "WAV playback request disabled. Undo restores playback."
            }
            .into();
        }
        Ok(())
    }

    fn overview_actions(
        &self,
        record: &CapabilityRecord,
    ) -> (Vec<(String, OverviewTarget)>, Vec<String>) {
        let mut actions = vec![];
        let mut notes = vec![];
        let mut add = |label: String, target: OverviewTarget| {
            if !actions.iter().any(|(_, existing)| existing == &target) {
                actions.push((label, target));
            }
        };
        let instances = occurrences(&self.document.root, &record.id);
        for route in &record.metadata.configuration {
            match route {
                Route::RootGraph => {
                    for (scope, node) in &instances {
                        add(
                            format!("Show {node} in {}", scope_label(scope)),
                            OverviewTarget::Graph {
                                scope: scope.clone(),
                                node: Some(node.clone()),
                            },
                        );
                    }
                    if instances.is_empty() && record.installed {
                        add(
                            "Open root graph (module not placed)".into(),
                            OverviewTarget::Graph {
                                scope: vec![],
                                node: None,
                            },
                        );
                    }
                }
                Route::RenderGraph => {
                    for (scope, id) in &instances {
                        let node = graph_at(&self.document.root, scope)
                            .and_then(|g| g.nodes.iter().find(|n| n.id == *id))
                            .unwrap();
                        if record.metadata.kind == CapabilityKind::Renderer {
                            if node.subgraph.is_some() {
                                let mut inner = scope.clone();
                                inner.push(id.clone());
                                add(
                                    format!("Open render graph: {}", scope_label(&inner)),
                                    OverviewTarget::Graph {
                                        scope: inner,
                                        node: None,
                                    },
                                );
                            }
                        } else {
                            add(
                                format!("Show {id} in {}", scope_label(scope)),
                                OverviewTarget::Graph {
                                    scope: scope.clone(),
                                    node: Some(id.clone()),
                                },
                            );
                        }
                    }
                    if instances.is_empty() || record.metadata.kind != CapabilityKind::Renderer {
                        if let Some(renderer) = super::active_vulkan_node(&self.document)
                            .filter(|n| n.subgraph.is_some())
                        {
                            add(
                                "Open active render graph".into(),
                                OverviewTarget::Graph {
                                    scope: vec![renderer.id.clone()],
                                    node: None,
                                },
                            );
                        } else {
                            notes.push(
                                "No connected Vulkan render graph exists in this project.".into(),
                            );
                        }
                    }
                }
                Route::ExecutionGraph => {
                    if self.document.game.is_some() {
                        add(
                            "Open Gameplay Systems".into(),
                            OverviewTarget::Window(EditorType::GameSystems),
                        );
                    } else {
                        notes
                            .push("This graph-only project has no game execution schedule.".into());
                    }
                }
                Route::RustSource | Route::HlslSource => {
                    let mut sources = std::collections::BTreeSet::new();
                    for (scope, id) in &instances {
                        if let Some(source) = graph_at(&self.document.root, scope)
                            .and_then(|g| g.nodes.iter().find(|n| n.id == *id))
                            .and_then(|n| n.params.get("source_id"))
                            .and_then(|v| v.as_str())
                        {
                            sources.insert(source.to_owned());
                        }
                    }
                    if let Some(game) = &self.document.game {
                        for node in game
                            .execution
                            .stages
                            .iter()
                            .flat_map(|s| &s.nodes)
                            .filter(|n| n.module == record.id)
                        {
                            if let Some(source) =
                                node.params.get("source_id").and_then(|v| v.as_str())
                            {
                                sources.insert(source.to_owned());
                            }
                        }
                    }
                    if sources.is_empty() {
                        notes.push("No source instance is assigned to this capability in the current project.".into());
                    }
                    for source in sources {
                        let (label, target) = if *route == Route::RustSource {
                            (format!("Edit Rust: {source}"), OverviewTarget::Rust(source))
                        } else {
                            (format!("Edit HLSL: {source}"), OverviewTarget::Hlsl(source))
                        };
                        add(label, target);
                    }
                }
                Route::ProjectSettings => add(
                    "Open permitted Project Settings".into(),
                    OverviewTarget::Window(EditorType::ProjectSettings),
                ),
                Route::ComponentFields => {
                    if self.document.game.is_some() {
                        add(
                            "Open Inspector (select a scene object)".into(),
                            OverviewTarget::Window(EditorType::Inspector),
                        );
                    } else {
                        notes.push("Component data is inspected in the owning graph/object tool; no authored game scene is present.".into());
                    }
                }
                Route::ModuleTool => {
                    for panel in &self.panel_registry {
                        if panel.module == record.id
                            || record.id == format!("editor.panel.{}", panel.id)
                        {
                            add(
                                format!("Open {}", panel.label),
                                OverviewTarget::Window(EditorType::Extension(panel.id)),
                            );
                        }
                    }
                }
                Route::NativeSource => {}
            }
        }
        if actions.is_empty() {
            notes.push(
                "No live editor target is provided for this capability in this project.".into(),
            );
        }
        (actions, notes)
    }

    fn overview_target_valid(&self, target: &OverviewTarget) -> Result<(), String> {
        if self.json_dirty {
            return Err("Apply or discard the Project File draft first".into());
        }
        let editor = target.editor();
        if editor.requires_engine() && !self.can_author_engine() {
            return Err("Full profile and checked matching sources are required".into());
        }
        if matches!(editor, EditorType::Extension(_))
            && !self
                .available_editors()
                .iter()
                .any(|item| item.editor == editor)
        {
            return Err(
                "This module tool is unavailable for the current project and editing rights".into(),
            );
        }
        match target {
            OverviewTarget::Graph { scope, node } => {
                let graph = graph_at(&self.document.root, scope)
                    .ok_or("The graph scope no longer exists")?;
                if node
                    .as_ref()
                    .is_some_and(|id| !graph.nodes.iter().any(|node| node.id == *id))
                {
                    return Err("The node no longer exists in that graph scope".into());
                }
            }
            OverviewTarget::Rust(source) => {
                if !self.document.custom_sources.contains_key(source)
                    || !self.can_edit_source(source)
                {
                    return Err("This Rust source is missing or read-only".into());
                }
            }
            OverviewTarget::Hlsl(source) => {
                if !self.document.shader_sources.contains_key(source) {
                    return Err("This HLSL source is missing".into());
                }
            }
            OverviewTarget::Window(EditorType::GameSystems) if self.document.game.is_none() => {
                return Err("This project has no game execution schedule".into());
            }
            OverviewTarget::AudioSystem(id) => {
                if !self.document.game.as_ref().is_some_and(|game| {
                    module_engine_arpg::audio_nodes(game).any(|node| &node.id == id)
                }) {
                    return Err("The audio system is no longer scheduled".into());
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Deferred until pane swaps finish: navigation must not be overwritten by a
    /// target pane's old memory later in the same dock frame.
    pub(super) fn apply_overview_navigation(&mut self) {
        let Some(target) = self.overview_navigation.take() else {
            return;
        };
        if let Err(error) = self.overview_target_valid(&target) {
            self.status = error;
            return;
        }
        let kind = target.editor();
        self.reveal_editor(kind);
        let Some(id) = self.active_window() else {
            return;
        };
        let (layout, panes) = match self.workspace {
            WorkspaceKind::Engine => (&self.engine_layout, &mut self.engine_panes),
            WorkspaceKind::Project => (&self.project_layout, &mut self.project_panes),
        };
        if !layout.panes().contains(&(id, kind)) {
            return;
        } // e.g. area limit; keep the existing navigation error.
        let memory = panes
            .entry(id)
            .or_insert_with(|| PaneMemory::new(Default::default(), &self.document));
        match target {
            OverviewTarget::Graph { scope, node } => {
                memory.snapshot.graph_scope = scope;
                memory.rebind_scope(&self.document);
                memory.graph_canvas.select_only(node.clone());
                memory.graph_canvas.pending = None;
                memory.graph_canvas.zoom = 0.65;
                memory.graph_canvas.pan = node
                    .as_ref()
                    .and_then(|id| {
                        graph_at(&self.document.root, &memory.snapshot.graph_scope)?
                            .nodes
                            .iter()
                            .find(|n| n.id == *id)
                    })
                    .map(|node| {
                        egui::vec2(32. - node.position[0] * 0.65, 48. - node.position[1] * 0.65)
                    })
                    .unwrap_or(egui::vec2(28., 35.));
                memory.snapshot(&self.document);
            }
            OverviewTarget::Rust(source) => {
                if memory.current_source != source {
                    memory.current_source = source;
                    memory.program = None;
                    memory.command_program = None;
                    memory.source_error = None;
                    memory.code_canvas.select_only(None);
                    memory.code_canvas.pending = None;
                    memory.code_positions.clear();
                }
                memory.snapshot(&self.document);
            }
            OverviewTarget::Hlsl(source) => {
                memory.current_shader = source;
                memory.snapshot(&self.document);
            }
            OverviewTarget::AudioSystem(id) => self.game_workbench.show_execution_node(id),
            OverviewTarget::Window(_) => {}
        }
        self.personal_dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        dock::{Axis, DockLayout},
        panes,
    };
    use super::*;
    use module_engine_core::{WorldKind, demo_document, make_node};
    use module_engine_script::TestArtifactGuard;

    fn fixture(document: &module_engine_core::GraphDocument) -> TestArtifactGuard {
        let guard = TestArtifactGuard::new(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.work/test-temp"),
            "overview",
        )
        .unwrap();
        crate::distribution::copy_native_fixture(guard.path());
        crate::distribution::write_full_manifest(guard.path(), document).unwrap();
        guard
    }
    fn inventory(editor: &mut Editor) -> Arc<CapabilityInventory> {
        editor.refresh_overview();
        editor.overview.inventory.as_ref().unwrap().clone()
    }
    fn record<'a>(inventory: &'a CapabilityInventory, id: &str) -> &'a CapabilityRecord {
        inventory.records.iter().find(|r| r.id == id).unwrap()
    }
    fn frame(
        editor: &mut Editor,
        pane: &mut OverviewPane,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> Vec<(String, egui::Pos2)> {
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440., 1080.),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| {
                editor.workflow_shortcuts(ui.ctx());
                editor.engine_overview_panel(ui, pane);
                editor.draw_audio_removal_dialog(ui.ctx());
            },
        );
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some((
                    text.galley.text().to_owned(),
                    text.pos + text.galley.size() * 0.5,
                )),
                _ => None,
            })
            .collect();
        output.drop_without_applying_deltas();
        text
    }

    #[test]
    fn audio_overview_blocks_removal_then_saves_and_undoes_the_exact_audio_configuration() {
        let mut document = crate::game_templates::demo_document();
        let game = document.game.as_mut().unwrap();
        game.audio_cues.retain(|cue, _| cue == "attack");
        game.execution
            .stages
            .retain(|stage| stage.name != "Custom game logic");
        document.custom_sources.clear();
        let node = game
            .execution
            .stages
            .iter_mut()
            .flat_map(|stage| &mut stage.nodes)
            .find(|node| node.module == module_engine_arpg::AUDIO_SYSTEM)
            .unwrap();
        node.params = serde_json::json!({"legacy": null});
        game.audio_cues
            .insert("attack".into(), "asset://practice-attack".into());
        let mut document = module_engine_project::externalize_scenes(&document, false).unwrap();
        let fixture = fixture(&document);
        for relative in ["assets/audio/attack.wav", "assets/models/demo_crystal.glb"] {
            let target = fixture.path().join(relative);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(crate::project::source_root().join(relative), target).unwrap();
        }
        let mut manifest = module_engine_project::AssetManifest::default();
        manifest
            .register("practice-attack", "assets/audio/attack.wav", fixture.path())
            .unwrap();
        document.asset_manifest = serde_json::to_value(manifest).unwrap();
        let path = fixture.path().join("audio-project.json");
        crate::project::save(&path, &document).unwrap();
        let document = crate::project::read(&path).unwrap();
        let mut editor = Editor::new(document.clone(), path.clone());
        editor.workspace = WorkspaceKind::Engine;
        let inv = inventory(&mut editor);
        let mut pane = OverviewPane {
            selected: Some(CapabilityKey::from(record(
                &inv,
                module_engine_arpg::AUDIO_SYSTEM,
            ))),
            owner: CapabilityOwner::Gameplay,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let click = |editor: &mut Editor, pane: &mut OverviewPane, text: &str| {
            let labels = frame(editor, pane, &ctx, vec![]);
            let pos = labels
                .iter()
                .find(|(label, _)| label == text)
                .unwrap_or_else(|| panic!("Missing {text}: {labels:?}"))
                .1;
            for pressed in [true, false] {
                frame(
                    editor,
                    pane,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
        };

        let original_file = std::fs::read(&path).unwrap();
        click(&mut editor, &mut pane, "Remove audio system…");
        let blocked = frame(&mut editor, &mut pane, &ctx, vec![]);
        assert!(
            blocked
                .iter()
                .any(|(label, _)| label.contains("Attack (attack) → assets/audio/attack.wav"))
        );
        assert!(
            !blocked
                .iter()
                .any(|(label, _)| label == "Remove audio system")
        );
        click(&mut editor, &mut pane, "Cancel");
        assert!(!editor.workflow.audio_removal);
        assert_eq!(editor.document, document);
        assert_eq!(editor.history.cursor(), 0);
        assert_eq!(std::fs::read(&path).unwrap(), original_file);
        assert!(
            editor
                .edit_audio_composition(true)
                .unwrap_err()
                .contains("Attack (attack)")
        );

        click(&mut editor, &mut pane, "Disable WAV playback request");
        let muted = editor.document.clone();
        let game = muted.game.as_ref().unwrap();
        assert!(!module_engine_arpg::has_audio_request(game));
        assert_eq!(
            module_engine_arpg::audio_nodes(game).next().unwrap().params,
            serde_json::json!({"legacy": null, "playback_enabled": false})
        );
        click(&mut editor, &mut pane, "Remove audio system…");
        for key in [egui::Key::S, egui::Key::Z] {
            frame(
                &mut editor,
                &mut pane,
                &ctx,
                vec![egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
            );
        }
        assert!(editor.workflow.audio_removal);
        assert_eq!(std::fs::read(&path).unwrap(), original_file);
        click(&mut editor, &mut pane, "Cancel");
        assert!(editor.document == muted, "{}", editor.status);
        click(&mut editor, &mut pane, "Remove audio system…");
        click(&mut editor, &mut pane, "Remove audio system");
        let silent = editor.document.game.clone().unwrap();
        assert!(module_engine_arpg::audio_nodes(&silent).next().is_none());
        assert_eq!(silent.audio_cues, muted.game.as_ref().unwrap().audio_cues);
        assert!(editor.save(), "{}", editor.status);
        let reopened = crate::project::read(&path).unwrap();
        assert_eq!(reopened.game, Some(silent));

        editor.undo();
        assert!(editor.document == muted, "{}", editor.status);
        editor.undo();
        assert!(editor.document == document, "{}", editor.status);
        assert!(module_engine_arpg::has_audio_request(
            editor.document.game.as_ref().unwrap()
        ));
    }

    #[test]
    fn fresh_engine_starts_at_overview_without_rewriting_saved_layouts() {
        let document = demo_document(WorldKind::Objects);
        let fixture = fixture(&document);
        let path = fixture.path().join("layout.json");
        let (fresh, _) = panes::load(&path, WorkspaceKind::Engine, &document).unwrap();
        assert_eq!(fresh.panes(), vec![(1, EditorType::EngineOverview)]);
        let mut saved = DockLayout::single(EditorType::ModuleGraph);
        let second = saved.split(1, Axis::Horizontal).unwrap();
        saved.choose(second, EditorType::Diagnostics);
        let mut memories =
            std::collections::BTreeMap::from([(1, PaneMemory::new(Default::default(), &document))]);
        memories.get_mut(&1).unwrap().graph_canvas.zoom = 1.2;
        panes::save(&path, &saved, &mut memories, &document).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let (loaded, panes) = panes::load(&path, WorkspaceKind::Engine, &document).unwrap();
        assert_eq!(loaded, saved);
        assert_eq!(panes[&1].graph_canvas.zoom, 1.2);
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert!(
            super::super::EditorDescriptor::builtins(true)
                .iter()
                .any(|e| e.editor == EditorType::EngineOverview)
        );
        assert!(
            !super::super::EditorDescriptor::builtins(false)
                .iter()
                .any(|e| e.editor == EditorType::EngineOverview)
        );
    }

    #[test]
    fn debounced_inventory_explicitly_marks_previous_authored_evidence_until_refresh() {
        let document = module_engine_core::demo_document(module_engine_core::WorldKind::Objects);
        let fixture = fixture(&document);
        let mut editor = Editor::new(document.clone(), fixture.path().join("project.json"));
        editor.last_tick = 5.;
        inventory(&mut editor);
        editor.overview.invalidate(5.);
        let ctx = egui::Context::default();
        let mut pane = OverviewPane::default();
        let text = frame(&mut editor, &mut pane, &ctx, vec![]);
        assert!(
            text.iter()
                .any(|(t, _)| t.starts_with("Updating authored state…"))
        );
        editor.last_tick = 5.2;
        let text = frame(&mut editor, &mut pane, &ctx, vec![]);
        assert!(
            !text
                .iter()
                .any(|(t, _)| t.starts_with("Updating authored state…"))
        );
        assert!(
            text.iter()
                .any(|(t, _)| t == "Authored selection · runtime readiness is separate")
        );
        assert_eq!(editor.document, document);
    }

    #[test]
    fn renderer_is_explained_and_real_egui_click_opens_and_reuses_its_inner_graph() {
        let document = crate::host::gpu_document(
            WorldKind::Objects,
            module_engine_vulkan::RenderPath::Forward,
        );
        let fixture = fixture(&document);
        let mut editor = Editor::new(document.clone(), fixture.path().join("project.json"));
        editor.workspace = WorkspaceKind::Engine;
        let ctx = egui::Context::default();
        let mut pane = OverviewPane::default();
        let text = frame(&mut editor, &mut pane, &ctx, vec![]);
        assert_eq!(pane.selected.as_ref().unwrap().id, "render.vulkan");
        assert!(text.iter().any(|(text, _)| text == "Working configuration"));
        assert!(
            text.iter()
                .any(|(text, _)| text.contains("Full · checked sources"))
        );
        assert!(
            text.iter()
                .any(|(text, _)| text.contains("module-engine-vulkan"))
        );
        assert!(
            text.iter()
                .any(|(text, _)| text.contains(owner_label(CapabilityOwner::Engine)))
        );
        assert!(
            text.iter()
                .any(|(text, _)| text == "Evidence (not runtime readiness)")
        );
        let label = "Open render graph: Root / renderer";
        let pos = text
            .iter()
            .find(|(text, _)| text == label)
            .expect("Renderer's real graph action is visible")
            .1;
        for pressed in [true, false] {
            frame(
                &mut editor,
                &mut pane,
                &ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert!(editor.overview_navigation.is_some());
        editor.apply_overview_navigation();
        let target = editor.active_window().unwrap();
        assert_eq!(
            editor.engine_panes[&target].snapshot.graph_scope,
            vec!["renderer"]
        );
        let areas = editor.engine_layout.clone();
        editor.overview_navigation = Some(OverviewTarget::Graph {
            scope: vec!["renderer".into()],
            node: None,
        });
        editor.apply_overview_navigation();
        assert_eq!(editor.active_window(), Some(target));
        assert_eq!(editor.engine_layout, areas);
        assert_eq!(editor.document, document);
        assert!(!editor.dirty);
    }

    #[test]
    fn nested_instances_and_source_targets_keep_exact_scope_and_reject_missing_targets() {
        let mut document = crate::host::gpu_document(
            WorldKind::Objects,
            module_engine_vulkan::RenderPath::Forward,
        );
        let mut other = document
            .root
            .nodes
            .iter()
            .find(|n| n.id == "renderer")
            .unwrap()
            .clone();
        other.id = "alternate".into();
        document.root.nodes.push(other);
        let fixture = fixture(&document);
        let mut editor = Editor::new(document.clone(), fixture.path().join("project.json"));
        editor.workspace = WorkspaceKind::Engine;
        let inv = inventory(&mut editor);
        let (actions, _) = editor.overview_actions(record(&inv, "gpu.fxaa"));
        let (scope, node) = actions
            .iter()
            .find_map(|(_, t)| match t {
                OverviewTarget::Graph {
                    scope,
                    node: Some(node),
                } if scope == &["alternate"] => Some((scope.clone(), node.clone())),
                _ => None,
            })
            .unwrap();
        editor.overview_navigation = Some(OverviewTarget::Graph {
            scope: scope.clone(),
            node: Some(node.clone()),
        });
        editor.apply_overview_navigation();
        let id = editor.active_window().unwrap();
        assert_eq!(editor.engine_panes[&id].snapshot.graph_scope, scope);
        assert_eq!(editor.engine_panes[&id].graph_canvas.selected, Some(node));
        let layout = editor.engine_layout.clone();
        editor.overview_navigation = Some(OverviewTarget::Graph {
            scope: vec!["deleted".into()],
            node: None,
        });
        editor.apply_overview_navigation();
        assert!(editor.status.contains("no longer exists"));
        assert_eq!(editor.engine_layout, layout);
        assert_eq!(editor.document, document);

        editor.reveal_editor(EditorType::RustCustom);
        let rust_id = editor.active_window().unwrap();
        let memory = editor
            .engine_panes
            .entry(rust_id)
            .or_insert_with(|| PaneMemory::new(Default::default(), &editor.document));
        memory.current_source = "previous-source".into();
        memory.code_canvas.selected = Some("previous-node".into());
        memory.code_canvas.pending = Some(("previous-node".into(), "output".into()));
        memory
            .code_positions
            .insert("previous-node".into(), [7., 8.]);
        editor.overview_navigation = Some(OverviewTarget::Rust("speed".into()));
        editor.apply_overview_navigation();
        assert_eq!(editor.engine_panes[&rust_id].code_canvas.selected, None);
        assert_eq!(editor.engine_panes[&rust_id].code_canvas.pending, None);
        assert!(editor.engine_panes[&rust_id].code_positions.is_empty());
        editor
            .engine_panes
            .get_mut(&rust_id)
            .unwrap()
            .code_canvas
            .selected = Some("keep-current-selection".into());
        editor.overview_navigation = Some(OverviewTarget::Rust("speed".into()));
        editor.apply_overview_navigation();
        assert_eq!(
            editor.engine_panes[&rust_id]
                .code_canvas
                .selected
                .as_deref(),
            Some("keep-current-selection")
        );
        assert_eq!(
            editor.engine_panes[&editor.active_window().unwrap()].current_source,
            "speed"
        );
        assert!(
            editor
                .overview_target_valid(&OverviewTarget::Hlsl("missing".into()))
                .is_err()
        );
        editor
            .document
            .shader_sources
            .insert("owned".into(), "// authored text".into());
        editor.overview_navigation = Some(OverviewTarget::Hlsl("owned".into()));
        editor.apply_overview_navigation();
        assert_eq!(
            editor.engine_panes[&editor.active_window().unwrap()].current_shader,
            "owned"
        );
    }

    #[test]
    fn inspection_is_cached_and_edits_history_and_project_adoption_refresh_participation() {
        let document = demo_document(WorldKind::Objects);
        let fixture = fixture(&document);
        let path = fixture.path().join("project.json");
        let mut editor = Editor::new(document.clone(), path.clone());
        let initial = inventory(&mut editor);
        assert_eq!(
            record(&initial, "world.objects").participation.value,
            Some(true)
        );
        assert_eq!(
            record(&initial, "world.ecs").participation.value,
            Some(false)
        );
        assert_eq!(editor.document, document);
        assert!(!path.exists());
        assert!(!fixture.path().join(".user/editor-play-state").exists());
        assert!(!editor.overview.needs_refresh(100.));
        editor
            .document
            .root
            .nodes
            .iter_mut()
            .find(|n| n.id == "world")
            .unwrap()
            .module = "world.ecs".into();
        editor.changed(false);
        assert!(!editor.overview.needs_refresh(0.05));
        assert!(editor.overview.needs_refresh(0.2));
        let edited = inventory(&mut editor);
        assert!(!Arc::ptr_eq(&initial, &edited));
        assert_eq!(record(&edited, "world.ecs").participation.value, Some(true));
        editor.restore_document(document.clone());
        assert!(editor.overview.invalidated_at.is_some());
        assert_eq!(
            record(&inventory(&mut editor), "world.objects")
                .participation
                .value,
            Some(true)
        );

        let g1 = crate::game_templates::g1_document();
        crate::distribution::write_full_manifest(fixture.path(), &g1).unwrap();
        std::fs::write(&path, serde_json::to_vec(&g1).unwrap()).unwrap();
        editor.overview_navigation = Some(OverviewTarget::Rust("speed".into()));
        assert!(editor.load(), "{}", editor.status);
        assert!(editor.overview_navigation.is_none());
        let inv = inventory(&mut editor);
        assert_eq!(
            record(&inv, "g1.rules").metadata.owner,
            CapabilityOwner::Project
        );
        assert_eq!(record(&inv, "g1.rules").participation.value, Some(true));
        assert_eq!(
            record(&inv, "game.combat").metadata.owner,
            CapabilityOwner::Gameplay
        );
        assert_eq!(record(&inv, "game.combat").participation.value, Some(false));
        assert_eq!(record(&inv, "editor.panel.110").available.value, Some(true));
        assert_eq!(editor.document, g1);
    }

    #[test]
    fn team_missing_sources_json_drafts_and_absent_optional_targets_do_not_gain_permissions() {
        let document = demo_document(WorldKind::Objects);
        let fixture = fixture(&document);
        let mut editor = Editor::new(document.clone(), fixture.path().join("project.json"));
        let inv = inventory(&mut editor);
        let (_, notes) = editor.overview_actions(record(&inv, "gpu.ssao"));
        assert!(notes.iter().any(|s| s.contains("No connected Vulkan")));
        let graph = OverviewTarget::Graph {
            scope: vec![],
            node: None,
        };
        editor.json_dirty = true;
        assert!(
            editor
                .overview_target_valid(&graph)
                .unwrap_err()
                .contains("draft")
        );
        editor.json_dirty = false;
        editor.distribution.as_mut().unwrap().profile =
            crate::distribution::DistributionProfile::Team;
        editor.overview_navigation = Some(graph.clone());
        let before = editor.engine_layout.clone();
        editor.apply_overview_navigation();
        assert_eq!(editor.engine_layout, before);
        assert!(
            editor
                .overview_target_valid(&OverviewTarget::Window(EditorType::EngineOverview))
                .is_err()
        );
        editor.distribution.as_mut().unwrap().profile =
            crate::distribution::DistributionProfile::Full;
        editor
            .distribution
            .as_mut()
            .unwrap()
            .source_status
            .available = false;
        assert!(editor.overview_target_valid(&graph).is_err());
        assert_eq!(editor.document, document);
    }

    #[test]
    fn unknown_evidence_and_metadata_are_shown_without_a_fake_active_or_ready_state() {
        let mut document = demo_document(WorldKind::Objects);
        document.engine_profile = None;
        document
            .engine
            .enabled_modules
            .insert("external.no-description".into());
        document.root.nodes.push(make_node(
            "unknown",
            "external.no-description",
            "Opaque",
            [0., 0.],
            serde_json::Value::Null,
        ));
        let fixture = fixture(&demo_document(WorldKind::Objects));
        // Begin with a normally validated installation; inspect a subsequent
        // invalid author candidate without pretending it passed source checks.
        let mut editor = Editor::new(
            demo_document(WorldKind::Objects),
            fixture.path().join("project.json"),
        );
        editor.document = document.clone();
        let inv = inventory(&mut editor);
        let unknown = record(&inv, "external.no-description");
        assert_eq!(unknown.metadata.owner, CapabilityOwner::Unknown);
        assert_eq!(unknown.available.value, Some(false));
        let unproven = record(&inv, "world.objects");
        assert_eq!(unproven.available.value, None);
        assert_eq!(fact_label(&unproven.available), "Unknown");
        assert_eq!(editor.document, document);
        assert!(!inv.diagnostics.is_empty());
    }

    #[test]
    fn actual_dock_frame_keeps_graph_and_rust_navigation_after_later_pane_swaps() {
        let mut document = crate::host::gpu_document(
            WorldKind::Objects,
            module_engine_vulkan::RenderPath::Forward,
        );
        let mut other = document
            .root
            .nodes
            .iter()
            .find(|n| n.id == "renderer")
            .unwrap()
            .clone();
        other.id = "alternate".into();
        document.root.nodes.push(other);
        document
            .custom_sources
            .insert("other".into(), "fn other() -> f64 { 1.0 }".into());
        let fixture = fixture(&document);
        let mut editor = Editor::new(document.clone(), fixture.path().join("project.json"));
        editor.playing = false;
        editor.workspace = WorkspaceKind::Engine;
        editor.engine_layout = DockLayout::single(EditorType::EngineOverview);
        let later = editor.engine_layout.split(1, Axis::Horizontal).unwrap();
        editor.engine_layout.choose(later, EditorType::ModuleGraph);
        let mut memory = PaneMemory::new(Default::default(), &document);
        memory.snapshot.graph_scope = vec!["alternate".into()];
        memory.rebind_scope(&document);
        editor.engine_panes.insert(later, memory);
        let ctx = egui::Context::default();
        let mut time = 1.0;
        let mut frame = |editor: &mut Editor, events: Vec<egui::Event>| {
            time += 0.04;
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(2400., 1500.),
                    )),
                    time: Some(time),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| editor.draw(ui),
            );
            let text: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some((
                        text.galley.text().to_owned(),
                        text.pos + text.galley.size() * 0.5,
                    )),
                    _ => None,
                })
                .collect();
            output.drop_without_applying_deltas();
            text
        };
        let text = frame(&mut editor, vec![]);
        let pos = text
            .iter()
            .find(|(t, _)| t == "Open render graph: Root / renderer")
            .unwrap()
            .1;
        for pressed in [true, false] {
            frame(
                &mut editor,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert_eq!(editor.active_window(), Some(later));
        assert_eq!(
            editor.engine_panes[&later].snapshot.graph_scope,
            vec!["renderer"]
        );
        frame(&mut editor, vec![]);
        assert_eq!(
            editor.engine_panes[&later].snapshot.graph_scope,
            vec!["renderer"]
        );
        // The same real dock path must replace a reused source pane's old source.
        editor.engine_layout.choose(later, EditorType::RustCustom);
        editor.engine_panes.get_mut(&later).unwrap().current_source = "other".into();
        editor.engine_panes.get_mut(&1).unwrap().overview.selected = Some(CapabilityKey {
            id: "custom.rust".into(),
            kind: CapabilityKind::SourceContract,
        });
        let text = frame(&mut editor, vec![]);
        let pos = text
            .iter()
            .find(|(t, _)| t == "Edit Rust: speed")
            .unwrap()
            .1;
        for pressed in [true, false] {
            frame(
                &mut editor,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        frame(&mut editor, vec![]);
        assert_eq!(editor.engine_panes[&later].current_source, "speed");
        assert_eq!(editor.engine_layout.panes().len(), 2);
        assert_eq!(editor.document, document);
        assert!(!editor.dirty);
    }
}
