//! Named personal arrangements. They share the open document and its history.
use super::panes::PaneSnapshot;
use super::*;
use serde::{Deserialize, Serialize};

struct Contents {
    layout: DockLayout,
    panes: BTreeMap<u64, PaneMemory>,
    navigation: window_navigation::WindowNavigation,
    library_graph: Option<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedWorkspace {
    id: u64,
    name: String,
    kind: WorkspaceKind,
    layout: DockLayout,
    panes: BTreeMap<u64, PaneSnapshot>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SavedWorkspaces {
    version: u32,
    next_id: u64,
    active: Option<u64>,
    workspaces: Vec<SavedWorkspace>,
}
pub(super) struct Workspaces {
    active: Option<u64>,
    base: Option<(WorkspaceKind, Contents)>,
    saved: BTreeMap<u64, SavedWorkspace>,
    memory: BTreeMap<u64, Contents>,
    next_id: u64,
    save_error: Option<String>,
}
impl Default for Workspaces {
    fn default() -> Self {
        Self {
            active: None,
            base: None,
            saved: BTreeMap::new(),
            memory: BTreeMap::new(),
            next_id: 1,
            save_error: None,
        }
    }
}
impl Workspaces {
    pub(super) fn active_id(&self) -> Option<u64> {
        self.active
    }
    pub(super) fn active_name(&self) -> Option<String> {
        self.active
            .and_then(|id| self.saved.get(&id).map(|s| s.name.clone()))
    }
}
pub(super) enum WorkspaceDialog {
    Create(String),
    Rename(u64, String),
    Delete(u64),
    Reset(Option<u64>, WorkspaceKind),
}
impl Editor {
    pub(super) fn window_menu_group(&self, editor: EditorType) -> &'static str {
        match editor {
            EditorType::EngineOverview
            | EditorType::EngineSettings
            | EditorType::ModuleGraph
            | EditorType::ModuleLibrary
            | EditorType::GameSystems => "Composition",
            EditorType::Scene
            | EditorType::Hierarchy
            | EditorType::Inspector
            | EditorType::Assets
            | EditorType::ProjectSettings => "Authoring",
            EditorType::RustCustom | EditorType::Hlsl | EditorType::ProjectFile => "Code",
            EditorType::Game => "Play",
            EditorType::History | EditorType::Diagnostics => "Review",
            EditorType::Viewport | EditorType::ObjectTree | EditorType::ObjectInspector => "Legacy",
            EditorType::Extension(id) => match self
                .panel_registry
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.metadata.owner)
            {
                Some(module_engine_core::CapabilityOwner::Project) => "Game tools",
                Some(module_engine_core::CapabilityOwner::Engine) => "Composition",
                _ => "Module tools",
            },
        }
    }
    pub(super) fn workspace_key(&self) -> String {
        self.workspaces
            .active
            .map_or_else(|| self.workspace.key().into(), |id| format!("custom-{id}"))
    }
    fn take_workspace_contents(&mut self, kind: WorkspaceKind) -> Contents {
        let (layout, panes, navigation, library_graph) = match kind {
            WorkspaceKind::Engine => (
                &mut self.engine_layout,
                &mut self.engine_panes,
                &mut self.engine_navigation,
                &mut self.engine_library_graph,
            ),
            WorkspaceKind::Project => (
                &mut self.project_layout,
                &mut self.project_panes,
                &mut self.project_navigation,
                &mut self.project_library_graph,
            ),
        };
        Contents {
            layout: std::mem::replace(layout, DockLayout::default_for(kind)),
            panes: std::mem::take(panes),
            navigation: std::mem::take(navigation),
            library_graph: library_graph.take(),
        }
    }
    fn put_workspace_contents(&mut self, kind: WorkspaceKind, contents: Contents) {
        match kind {
            WorkspaceKind::Engine => {
                self.engine_layout = contents.layout;
                self.engine_panes = contents.panes;
                self.engine_navigation = contents.navigation;
                self.engine_library_graph = contents.library_graph;
            }
            WorkspaceKind::Project => {
                self.project_layout = contents.layout;
                self.project_panes = contents.panes;
                self.project_navigation = contents.navigation;
                self.project_library_graph = contents.library_graph;
            }
        }
    }
    pub(super) fn unmount_custom_workspace(&mut self) -> Option<u64> {
        let id = self.workspaces.active.take()?;
        let (kind, base) = self
            .workspaces
            .base
            .take()
            .expect("Mounted custom workspace owns its default shelf");
        let current = self.take_workspace_contents(kind);
        self.workspaces.memory.insert(id, current);
        self.put_workspace_contents(kind, base);
        Some(id)
    }
    fn mount_custom_workspace(&mut self, id: u64) -> Result<(), String> {
        let saved = self
            .workspaces
            .saved
            .get(&id)
            .ok_or("Workspace no longer exists")?
            .clone();
        if saved.kind == WorkspaceKind::Engine && !self.can_author_engine() {
            return Err("Engine workspace requires Full with checked sources".into());
        }
        let contents = self
            .workspaces
            .memory
            .remove(&id)
            .unwrap_or_else(|| Contents {
                layout: saved.layout,
                panes: saved
                    .panes
                    .into_iter()
                    .map(|(id, s)| (id, PaneMemory::new(s, &self.document)))
                    .collect(),
                navigation: Default::default(),
                library_graph: None,
            });
        let base = self.take_workspace_contents(saved.kind);
        self.workspaces.base = Some((saved.kind, base));
        self.put_workspace_contents(saved.kind, contents);
        self.workspaces.active = Some(id);
        self.workspace = saved.kind;
        Ok(())
    }
    pub(super) fn reconcile_workspace_selection(&mut self) {
        if self.workspaces.base.as_ref().is_some_and(|(kind, _)| {
            *kind != self.workspace || (*kind == WorkspaceKind::Engine && !self.can_author_engine())
        }) {
            let target = self.workspace;
            self.unmount_custom_workspace();
            self.workspace = target;
        }
    }
    pub(super) fn switch_workspace_tab(&mut self, kind: WorkspaceKind, id: Option<u64>) {
        if self.workflow.blocks_document_edits() {
            return;
        }
        if self.workspace == kind && self.workspaces.active == id {
            return;
        }
        if kind == WorkspaceKind::Engine && !self.can_author_engine() {
            self.status = "Engine workspace requires Full with checked sources".into();
            return;
        }
        if let Some(id) = id
            && self
                .workspaces
                .saved
                .get(&id)
                .is_none_or(|s| s.kind != kind)
        {
            self.status = "Workspace no longer exists in this context".into();
            return;
        }
        self.cancel_active_scene_gestures();
        self.unmount_custom_workspace();
        self.workspace = kind;
        if let Some(id) = id
            && let Err(error) = self.mount_custom_workspace(id)
        {
            self.status = error;
        }
        self.personal_dirty = true;
    }
    fn workspace_name_valid(&self, name: &str, except: Option<u64>) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
            return Err("Use a name of 1–64 characters without control characters".into());
        }
        if ["engine", "project"].contains(&name.to_lowercase().as_str())
            || self
                .workspaces
                .saved
                .values()
                .any(|s| Some(s.id) != except && s.name.to_lowercase() == name.to_lowercase())
        {
            return Err("Choose a unique workspace name".into());
        }
        Ok(name.into())
    }
    fn create_named_workspace(&mut self, name: &str) -> Result<u64, String> {
        if let Some(error) = self.workspace_storage_error() {
            return Err(format!(
                "Repair and reload workspace storage first: {error}"
            ));
        }
        if self.workspaces.saved.len() >= 64 {
            return Err("At most 64 custom workspaces".into());
        }
        let name = self.workspace_name_valid(name, None)?;
        let kind = self.workspace;
        let (layout, panes) = match kind {
            WorkspaceKind::Engine => (&self.engine_layout, &mut self.engine_panes),
            WorkspaceKind::Project => (&self.project_layout, &mut self.project_panes),
        };
        let saved = SavedWorkspace {
            id: self.workspaces.next_id,
            name,
            kind,
            layout: layout.clone(),
            panes: panes
                .iter_mut()
                .map(|(id, m)| (*id, m.snapshot(&self.document)))
                .collect(),
        };
        let id = saved.id;
        self.workspaces.next_id = id.checked_add(1).ok_or("Workspace IDs exhausted")?;
        self.workspaces.saved.insert(id, saved);
        self.switch_workspace_tab(kind, Some(id));
        Ok(id)
    }
    fn workspaces_path(&self) -> PathBuf {
        self.personal_path(WorkspaceKind::Project)
            .with_file_name("workspaces.json")
    }
    pub(super) fn read_named_workspace_storage(&self) -> Result<Option<SavedWorkspaces>, String> {
        let path = self.workspaces_path();
        if !path.exists() {
            return Ok(None);
        }
        if !path.is_file() {
            return Err("Workspace path is not a file".into());
        }
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 8 * 1024 * 1024 {
            return Err("Workspace collection is too large".into());
        }
        let candidate: SavedWorkspaces =
            serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if candidate.version != 1 || candidate.next_id == 0 || candidate.workspaces.len() > 64 {
            return Err("Unsupported or invalid workspace collection".into());
        }
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for item in &candidate.workspaces {
            item.layout.validate()?;
            if item.id == 0
                || item.id >= candidate.next_id
                || !ids.insert(item.id)
                || !names.insert(item.name.to_lowercase())
                || item.name.trim() != item.name
                || item.name.is_empty()
                || item.name.chars().count() > 64
                || item.name.chars().any(char::is_control)
                || ["engine", "project"].contains(&item.name.to_lowercase().as_str())
            {
                return Err("Invalid/duplicate custom workspace identity".into());
            }
            for pane in item.panes.values() {
                panes::validate_pane_snapshot(pane)?;
            }
        }
        if candidate.active.is_some_and(|id| !ids.contains(&id)) {
            return Err("Selected workspace is missing".into());
        }
        Ok(Some(candidate))
    }
    pub(super) fn block_workspace_save(&mut self, error: String) {
        self.workspaces.save_error = Some(error);
    }
    pub(super) fn workspace_storage_error(&self) -> Option<&str> {
        self.workspaces.save_error.as_deref()
    }
    pub(super) fn adopt_named_workspaces(
        &mut self,
        candidate: Option<SavedWorkspaces>,
    ) -> Result<(), String> {
        self.unmount_custom_workspace();
        let Some(candidate) = candidate else {
            self.workspaces = Default::default();
            return Ok(());
        };
        self.workspaces = Workspaces {
            saved: candidate
                .workspaces
                .into_iter()
                .map(|s| (s.id, s))
                .collect(),
            next_id: candidate.next_id,
            ..Default::default()
        };
        if let Some(id) = candidate.active
            && (self.workspaces.saved[&id].kind != WorkspaceKind::Engine
                || self.can_author_engine())
        {
            self.mount_custom_workspace(id)?;
        }
        Ok(())
    }
    pub(super) fn save_named_workspaces(&mut self) -> Result<(), String> {
        if let Some(error) = &self.workspaces.save_error {
            return Err(format!(
                "Workspace storage was not loaded; repair it and reload saved layouts before saving: {error}"
            ));
        }
        let active = self.unmount_custom_workspace();
        for (id, memory) in &mut self.workspaces.memory {
            if let Some(saved) = self.workspaces.saved.get_mut(id) {
                saved.layout = memory.layout.clone();
                saved.panes = memory
                    .panes
                    .iter_mut()
                    .map(|(id, m)| (*id, m.snapshot(&self.document)))
                    .collect();
            }
        }
        let data = SavedWorkspaces {
            version: 1,
            next_id: self.workspaces.next_id,
            active,
            workspaces: self.workspaces.saved.values().cloned().collect(),
        };
        let path = self.workspaces_path();
        let result = (|| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let bytes = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
            let pending = path.with_extension("pending");
            std::fs::write(&pending, bytes).map_err(|e| e.to_string())?;
            std::fs::rename(pending, path).map_err(|e| e.to_string())
        })();
        if let Some(id) = active {
            self.mount_custom_workspace(id)?;
        }
        result
    }
    pub(super) fn with_default_workspace_storage(
        &mut self,
        save: impl FnOnce(&mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        let active = self.unmount_custom_workspace();
        let result = save(self);
        if let Some(id) = active {
            self.mount_custom_workspace(id)?;
        }
        result
    }
    pub(super) fn workspace_tabs(&mut self, ui: &mut egui::Ui) {
        let mut select = None;
        let mut dialog = None;
        let entries: Vec<_> = self
            .workspaces
            .saved
            .values()
            .map(|s| (s.id, s.name.clone(), s.kind))
            .collect();
        let active = self.workspaces.active;
        if ui.available_width() < 220. {
            let current = active
                .and_then(|id| self.workspaces.saved.get(&id).map(|s| s.name.clone()))
                .unwrap_or_else(|| match self.workspace {
                    WorkspaceKind::Engine => "Engine".into(),
                    WorkspaceKind::Project => "Project".into(),
                });
            ui.menu_button(current, |ui| {
                for (kind, label) in [
                    (WorkspaceKind::Engine, "Engine"),
                    (WorkspaceKind::Project, "Project"),
                ] {
                    if ui
                        .add_enabled(
                            kind != WorkspaceKind::Engine || self.can_author_engine(),
                            egui::Button::new(label),
                        )
                        .clicked()
                    {
                        select = Some((kind, None));
                        ui.close();
                    }
                }
                for (id, name, kind) in &entries {
                    if ui.button(name).clicked() {
                        select = Some((*kind, Some(*id)));
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Create workspace…").clicked() {
                    dialog = Some(WorkspaceDialog::Create(String::new()));
                    ui.close();
                }
            });
        } else {
            let width = (ui.available_width() - 40.).max(88.);
            egui::ScrollArea::horizontal()
                .id_salt("workspace-tabs")
                .max_width(width)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (kind, label) in [
                            (WorkspaceKind::Engine, "Engine"),
                            (WorkspaceKind::Project, "Project"),
                        ] {
                            if kind == WorkspaceKind::Engine && !self.can_author_engine() {
                                continue;
                            }
                            let response = ui.add_sized(
                                [88., editor_ui::COMPACT_CONTROL_SIZE],
                                egui::Button::new(label)
                                    .frame(false)
                                    .selected(active.is_none() && self.workspace == kind),
                            );
                            if active.is_none() && self.workspace == kind {
                                ui.painter().line_segment(
                                    [response.rect.left_bottom(), response.rect.right_bottom()],
                                    egui::Stroke::new(2., super::editor_ui::ACCENT),
                                );
                            }
                            if response.clicked() {
                                select = Some((kind, None));
                            }
                            response.context_menu(|ui| {
                                if ui.button("Reset this layout…").clicked() {
                                    dialog = Some(WorkspaceDialog::Reset(None, kind));
                                    ui.close();
                                }
                            });
                        }
                        for (id, name, kind) in &entries {
                            let response = ui
                                .add_sized(
                                    [
                                        (name.chars().count() * 7 + 24).clamp(88, 180) as f32,
                                        editor_ui::COMPACT_CONTROL_SIZE,
                                    ],
                                    egui::Button::new(name)
                                        .truncate()
                                        .frame(false)
                                        .selected(active == Some(*id)),
                                )
                                .on_hover_text(name);
                            if active == Some(*id) {
                                ui.painter().line_segment(
                                    [response.rect.left_bottom(), response.rect.right_bottom()],
                                    egui::Stroke::new(2., super::editor_ui::ACCENT),
                                );
                            }
                            if response.clicked() {
                                select = Some((*kind, Some(*id)));
                            }
                            response.context_menu(|ui| {
                                if ui.button("Rename workspace…").clicked() {
                                    dialog = Some(WorkspaceDialog::Rename(*id, name.clone()));
                                    ui.close();
                                }
                                if ui.button("Delete workspace…").clicked() {
                                    dialog = Some(WorkspaceDialog::Delete(*id));
                                    ui.close();
                                }
                                if ui.button("Reset this layout…").clicked() {
                                    dialog = Some(WorkspaceDialog::Reset(Some(*id), *kind));
                                    ui.close();
                                }
                            });
                        }
                    });
                });
            if editor_ui::icon_button_sized(
                ui,
                editor_ui::Icon::Plus,
                "Create workspace · copy current arrangement",
                !self.workflow.blocks_document_edits(),
                false,
                editor_ui::COMPACT_CONTROL_SIZE,
            )
            .clicked()
            {
                dialog = Some(WorkspaceDialog::Create(String::new()));
            }
        }
        if let Some((kind, id)) = select {
            self.switch_workspace_tab(kind, id);
        }
        if let Some(dialog) = dialog {
            self.cancel_active_scene_gestures();
            self.workflow.workspace_dialog = Some(dialog);
        }
    }
    fn reset_named_layout(&mut self, id: Option<u64>, kind: WorkspaceKind) -> Result<(), String> {
        if kind == WorkspaceKind::Engine && !self.can_author_engine() {
            return Err("Engine workspace requires Full with checked sources".into());
        }
        if id.is_some_and(|id| {
            self.workspaces
                .saved
                .get(&id)
                .is_none_or(|s| s.kind != kind)
        }) {
            return Err("Workspace no longer exists".into());
        }
        let previous_kind = self.workspace;
        let previous = self.workspaces.active;
        self.switch_workspace_tab(kind, id);
        if self.workspace != kind || self.workspaces.active != id {
            return Err("Workspace could not be selected; other layouts preserved".into());
        }
        match kind {
            WorkspaceKind::Engine => {
                self.engine_layout = DockLayout::default_for(kind);
                self.engine_panes.clear();
                self.engine_navigation = Default::default();
            }
            WorkspaceKind::Project => {
                self.project_layout = DockLayout::default_for(kind);
                self.project_panes.clear();
                self.project_navigation = Default::default();
            }
        }
        self.switch_workspace_tab(previous_kind, previous);
        self.personal_dirty = true;
        Ok(())
    }
    pub(super) fn draw_workspace_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.workflow.workspace_dialog.take() else {
            return;
        };
        let mut confirm = false;
        let mut cancel = false;
        let response=egui::Modal::new(egui::Id::new("workspace-dialog")).show(ctx,|ui|{
            ui.set_max_width(440.);
            match &mut dialog {
                WorkspaceDialog::Create(name)=>{ui.heading("Create workspace");ui.label("Copy the current arrangement. The project and editing rights stay the same.");ui.text_edit_singleline(name);}
                WorkspaceDialog::Rename(_,name)=>{ui.heading("Rename workspace");ui.text_edit_singleline(name);}
                WorkspaceDialog::Delete(id)=>{ui.heading("Delete workspace");ui.label(format!("Remove '{}' from personal layouts? The project is preserved.",self.workspaces.saved.get(id).map(|s|s.name.as_str()).unwrap_or("Missing workspace")));}
                WorkspaceDialog::Reset(_,_)=>{ui.heading("Reset selected layout");ui.label("Restore this arrangement's default areas. Other layouts and the project are preserved.");}
            }
            let validation=if let Some(error)=self.workspace_storage_error(){Err(format!("Repair and reload saved layouts first: {error}"))}else{match &dialog {WorkspaceDialog::Create(n)=>self.workspace_name_valid(n,None),WorkspaceDialog::Rename(id,n)=>self.workspace_name_valid(n,Some(*id)),_=>Ok(String::new())}};
            if let Err(error)=&validation {ui.colored_label(Color32::LIGHT_RED,error);}
            ui.horizontal(|ui|{confirm=ui.add_enabled(validation.is_ok(),egui::Button::new("Confirm")).clicked();cancel=ui.button("Cancel").clicked();});
        });
        if confirm {
            let result = match dialog {
                WorkspaceDialog::Create(name) => self.create_named_workspace(&name).map(|_| ()),
                WorkspaceDialog::Rename(id, name) => {
                    if let Some(saved) = self.workspaces.saved.get_mut(&id) {
                        saved.name = name.trim().into();
                    }
                    Ok(())
                }
                WorkspaceDialog::Delete(id) => {
                    if self.workspaces.active == Some(id) {
                        let kind = self.workspace;
                        self.unmount_custom_workspace();
                        self.workspace = kind;
                    }
                    self.workspaces.saved.remove(&id);
                    self.workspaces.memory.remove(&id);
                    Ok(())
                }
                WorkspaceDialog::Reset(id, kind) => self.reset_named_layout(id, kind),
            };
            self.personal_dirty = true;
            if let Err(error) = result {
                self.status = error;
            }
        } else if !cancel && !response.should_close() {
            self.workflow.workspace_dialog = Some(dialog);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(document: &GraphDocument) -> module_engine_script::TestArtifactGuard {
        let guard = module_engine_script::TestArtifactGuard::new(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.work/test-temp"),
            "named-workspaces",
        )
        .unwrap();
        crate::distribution::copy_native_fixture(guard.path());
        crate::distribution::write_full_manifest(guard.path(), document).unwrap();
        guard
    }
    #[test]
    fn custom_arrangements_roundtrip_without_overwriting_default_layout_or_authored_history() {
        let document = module_engine_core::demo_document(WorldKind::Objects);
        let root = fixture(&document);
        let mut editor = Editor::new(document, root.path().join("project.json"));
        editor.workspace = WorkspaceKind::Engine;
        let original = editor.document.clone();
        let defaults = editor.engine_layout.clone();
        let id = editor.create_named_workspace("Graph work").unwrap();
        let added = editor
            .engine_layout
            .split(1, dock::Axis::Horizontal)
            .unwrap();
        editor.engine_layout.choose(added, EditorType::RustCustom);
        let customized = editor.engine_layout.clone();
        editor.save_personal_layouts();
        assert!(!editor.personal_dirty, "{}", editor.status);
        let saved_default = panes::load(
            &editor.personal_path(WorkspaceKind::Engine),
            WorkspaceKind::Engine,
            &editor.document,
        )
        .unwrap()
        .0;
        assert_eq!(saved_default, defaults);
        assert_eq!(editor.engine_layout, customized);
        assert_eq!(editor.workspaces.active, Some(id));
        editor.switch_workspace_tab(WorkspaceKind::Engine, None);
        assert_eq!(editor.engine_layout, defaults);
        editor.load_personal_layouts();
        assert_eq!(editor.workspaces.active, Some(id));
        assert_eq!(editor.engine_layout, customized);
        assert_eq!(editor.document, original);
        assert!(!editor.history.can_undo());
        assert!(!editor.has_unsaved_work());
    }
    #[test]
    fn invalid_workspace_selection_names_and_failed_publication_preserve_current_arrangement() {
        let document = module_engine_core::demo_document(WorldKind::Objects);
        let root = fixture(&document);
        let mut editor = Editor::new(document, root.path().join("project.json"));
        let before = editor.document.clone();
        let id = editor.create_named_workspace("My layout").unwrap();
        let arrangement = editor.engine_layout.clone();
        assert!(editor.create_named_workspace("Engine").is_err());
        assert!(editor.create_named_workspace("my LAYOUT").is_err());
        editor.switch_workspace_tab(WorkspaceKind::Engine, Some(u64::MAX));
        assert_eq!(editor.workspaces.active, Some(id));
        assert_eq!(editor.engine_layout, arrangement);
        let path = editor.workspaces_path();
        std::fs::create_dir_all(&path).unwrap();
        assert!(editor.save_named_workspaces().is_err());
        assert_eq!(editor.workspaces.active, Some(id));
        assert_eq!(editor.engine_layout, arrangement);
        assert_eq!(editor.document, before);
    }
    #[test]
    fn unreadable_reload_keeps_active_layout_and_blocks_automatic_overwrite_until_repair() {
        let document = module_engine_core::demo_document(WorldKind::Objects);
        let root = fixture(&document);
        let mut editor = Editor::new(document, root.path().join("project.json"));
        let id = editor.create_named_workspace("My work").unwrap();
        editor.save_personal_layouts();
        let path = editor.workspaces_path();
        let good = std::fs::read(&path).unwrap();
        let layout = editor.engine_layout.clone();
        std::fs::write(&path, b"{bad").unwrap();
        editor.load_personal_layouts();
        assert_eq!(editor.workspaces.active, Some(id));
        assert_eq!(editor.engine_layout, layout);
        assert!(editor.workspace_storage_error().is_some());
        editor.personal_dirty = true;
        editor.save_personal_layouts();
        assert_eq!(std::fs::read(&path).unwrap(), b"{bad");
        assert!(editor.create_named_workspace("Unsaved").is_err());
        std::fs::write(&path, good).unwrap();
        editor.load_personal_layouts();
        assert!(editor.workspace_storage_error().is_none());
        assert_eq!(editor.workspaces.active, Some(id));
    }
    #[test]
    fn reset_unavailable_engine_custom_does_not_mutate_default_or_project_layouts() {
        let document = module_engine_core::demo_document(WorldKind::Objects);
        let root = fixture(&document);
        let mut editor = Editor::new(document, root.path().join("project.json"));
        let id = editor.create_named_workspace("Engine work").unwrap();
        editor.switch_workspace_tab(WorkspaceKind::Project, None);
        editor
            .distribution
            .as_mut()
            .unwrap()
            .source_status
            .available = false;
        let engine = editor.engine_layout.clone();
        let project = editor.project_layout.clone();
        let saved = serde_json::to_value(&editor.workspaces.saved[&id]).unwrap();
        assert!(
            editor
                .reset_named_layout(Some(id), WorkspaceKind::Engine)
                .is_err()
        );
        assert_eq!(editor.engine_layout, engine);
        assert_eq!(editor.project_layout, project);
        assert_eq!(
            serde_json::to_value(&editor.workspaces.saved[&id]).unwrap(),
            saved
        );
    }
}
