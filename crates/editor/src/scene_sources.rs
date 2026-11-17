//! Source-scene navigation and explicit file actions. No IO occurs while drawing.
use super::{Editor, dock::EditorType, editor_ui};
use eframe::egui::{self, Key};
use module_engine_core::{
    AuthoredName, AuthoredObject, AuthoredScene, SceneAssetBinding, Transform, WorldObject,
};
use module_engine_project::SceneSourceEdit;

#[derive(Clone, Debug)]
pub(super) enum SceneCommand {
    Open(String),
    Reveal(String),
    New,
    Duplicate(String),
    Rename(String),
    Move(String),
    Delete(String),
    SetEntry(String),
    Convert,
    RenameObject { scene: String, object: u64 },
}

#[cfg(test)]
mod tests {
    use super::super::editor_workflow::workflow_tests::{Fixture, complete_asset_job};
    use super::*;
    fn frame(editor: &mut Editor, ctx: &egui::Context, events: Vec<egui::Event>) {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(480., 360.),
                )),
                events,
                ..Default::default()
            },
            |ui| editor.draw_scene_source_dialog(ui.ctx()),
        )
        .drop_without_applying_deltas();
    }
    fn click(editor: &mut Editor, ctx: &egui::Context, target: &str) {
        // egui measures a new modal before centering it. Hit-test the settled
        // frame that is actually shown, not the initial measurement coordinates.
        frame(editor, ctx, vec![]);
        frame(editor, ctx, vec![]);
        let pos = editor.scene_source_ui.targets[target].center();
        for pressed in [true, false] {
            frame(
                editor,
                ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    #[test]
    fn object_name_dialog_pointer_apply_cancel_undo_save_and_scene_qualified_selection() {
        let (fixture, mut editor) = Fixture::new(true);
        let ctx = egui::Context::default();
        editor.game_workbench.scene = "arena".into();
        editor.game_workbench.select(Some(1));
        let before = editor.document.clone();
        let bytes = std::fs::read(&fixture.path).unwrap();
        let history = editor.history.entries().len();
        editor.request_scene_command(
            SceneCommand::RenameObject {
                scene: "arena".into(),
                object: 1,
            },
            &ctx,
        );
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![egui::Event::Text("Hero".into())]);
        assert_eq!(editor.scene_source_ui.dialog.as_ref().unwrap().name, "Hero");
        click(&mut editor, &ctx, "cancel");
        assert!(editor.document == before);
        assert_eq!(editor.history.entries().len(), history);
        editor.request_scene_command(
            SceneCommand::RenameObject {
                scene: "arena".into(),
                object: 1,
            },
            &ctx,
        );
        frame(&mut editor, &ctx, vec![]);
        frame(&mut editor, &ctx, vec![egui::Event::Text("Hero".into())]);
        click(&mut editor, &ctx, "apply");
        assert!(!editor.scene_source_ui.active(), "{}", editor.status);
        let hero = |e: &Editor| {
            e.document.game.as_ref().unwrap().scenes["arena"]
                .objects
                .iter()
                .find(|o| o.object.id == 1)
                .unwrap()
                .name
                .clone()
        };
        assert_eq!(hero(&editor), AuthoredName::Text("Hero".into()));
        assert_eq!(editor.history.entries().len(), history + 1);
        assert_eq!(
            std::fs::read(&fixture.path).unwrap(),
            bytes,
            "Authoring waits for Save"
        );
        editor.undo();
        assert!(editor.document == before);
        editor.redo();
        assert_eq!(hero(&editor), AuthoredName::Text("Hero".into()));
        assert!(editor.save());
        assert!(crate::project::read(&fixture.path).unwrap() == editor.document);
        editor.game_workbench.scene = "removed-scene".into();
        editor.game_workbench.select(Some(1));
        editor
            .game_workbench
            .normalize_selection(editor.document.game.as_ref().unwrap());
        assert_eq!(editor.game_workbench.selected(), None);
    }
    #[test]
    fn scene_creation_and_assets_move_share_file_history_and_cancel_is_inert() {
        let (fixture, mut editor) = Fixture::new(true);
        let ctx = egui::Context::default();
        let before = editor.document.clone();
        let bytes = std::fs::read(&fixture.path).unwrap();
        editor.request_scene_command(SceneCommand::New, &ctx);
        click(&mut editor, &ctx, "cancel");
        assert!(editor.document == before);
        assert_eq!(std::fs::read(&fixture.path).unwrap(), bytes);
        editor.request_scene_command(SceneCommand::New, &ctx);
        click(&mut editor, &ctx, "apply");
        complete_asset_job(&mut editor, &ctx);
        let game = editor.document.game.as_ref().unwrap();
        assert_eq!(
            game.scenes.len(),
            before.game.as_ref().unwrap().scenes.len() + 1
        );
        let id = game
            .scenes
            .keys()
            .find(|id| !before.game.as_ref().unwrap().scenes.contains_key(*id))
            .unwrap()
            .clone();
        let binding = game.scene_assets[&id].clone();
        assert!(fixture.root.join(&binding.path).is_file());
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&fixture.path).unwrap()).unwrap();
        assert_eq!(raw["scene_storage_version"], 1);
        assert!(raw["game"].get("scenes").is_none());
        editor.request_asset_action(
            crate::assets_ui::BrowserAction::Scene {
                asset: binding.id.clone(),
                action: crate::assets_ui::SceneAssetAction::Move,
            },
            &ctx,
        );
        assert!(editor.scene_source_ui.active());
        editor.scene_source_ui.dialog.as_mut().unwrap().folder = "scenes/rooms".into();
        click(&mut editor, &ctx, "apply");
        complete_asset_job(&mut editor, &ctx);
        let moved = editor.document.game.as_ref().unwrap().scene_assets[&id].clone();
        assert_eq!(moved.id, binding.id);
        assert!(moved.path.starts_with("scenes/rooms/"));
        assert!(!fixture.root.join(&binding.path).exists());
        editor.undo();
        assert_eq!(
            editor.document.game.as_ref().unwrap().scene_assets[&id],
            binding
        );
        assert!(fixture.root.join(&binding.path).is_file());
        editor.redo();
        assert_eq!(
            editor.document.game.as_ref().unwrap().scene_assets[&id],
            moved
        );
        assert!(crate::project::read(&fixture.path).unwrap() == editor.document);
    }
    #[test]
    fn unchanged_null_name_stays_null_and_stale_dialog_keeps_newer_authoring() {
        let (fixture, mut editor) = Fixture::new(true);
        let ctx = egui::Context::default();
        editor
            .document
            .game
            .as_mut()
            .unwrap()
            .scenes
            .get_mut("arena")
            .unwrap()
            .objects
            .iter_mut()
            .find(|o| o.object.id == 1)
            .unwrap()
            .name = AuthoredName::Null;
        editor.changed(false);
        assert!(editor.save());
        let before = editor.document.clone();
        let history = editor.history.entries().len();
        let bytes = std::fs::read(&fixture.path).unwrap();
        let command = SceneCommand::RenameObject {
            scene: "arena".into(),
            object: 1,
        };
        editor.request_scene_command(command.clone(), &ctx);
        click(&mut editor, &ctx, "apply");
        assert!(!editor.scene_source_ui.active());
        assert!(editor.document == before);
        assert_eq!(editor.history.entries().len(), history);
        assert_eq!(std::fs::read(&fixture.path).unwrap(), bytes);
        editor.request_scene_command(command, &ctx);
        frame(&mut editor, &ctx, vec![]);
        frame(
            &mut editor,
            &ctx,
            vec![egui::Event::Text("Candidate".into())],
        );
        let mut newer = editor.document.clone();
        newer
            .game
            .as_mut()
            .unwrap()
            .scenes
            .get_mut("arena")
            .unwrap()
            .objects[0]
            .object
            .color = [1, 2, 3, 255];
        editor
            .commit_scene_document(newer.clone(), "Independent authoring update")
            .unwrap();
        click(&mut editor, &ctx, "apply");
        assert!(editor.scene_source_ui.active());
        assert!(
            editor
                .scene_source_ui
                .dialog
                .as_ref()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("Scene changed")
        );
        assert!(editor.document == newer);
        assert_eq!(std::fs::read(&fixture.path).unwrap(), bytes);
        click(&mut editor, &ctx, "cancel");
        assert!(editor.document == newer);
    }
}

#[derive(Default)]
pub(super) struct SceneSourceUi {
    dialog: Option<SceneDialog>,
    #[cfg(test)]
    targets: std::collections::BTreeMap<String, egui::Rect>,
    pub(super) external_error: Option<String>,
}
struct SceneDialog {
    command: SceneCommand,
    name: String,
    filename: String,
    folder: String,
    base: Option<(AuthoredScene, Option<SceneAssetBinding>)>,
    error: Option<String>,
    first_frame: bool,
}
impl SceneSourceUi {
    pub(super) fn active(&self) -> bool {
        self.dialog.is_some()
    }
}

impl Editor {
    fn commit_hierarchy_document(
        &mut self,
        candidate: crate::app::GraphDocument,
        label: &str,
    ) -> Result<(), String> {
        let before = self.document.clone();
        self.commit_scene_document(candidate, label)?;
        if before != self.document {
            self.history.record("Project change", &before);
            self.history.record(label, &self.document);
            self.history_dirty = false;
        }
        Ok(())
    }

    pub(super) fn apply_hierarchy_action(
        &mut self,
        action: crate::game_ui::HierarchyAction,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        use crate::game_ui::{HierarchyAction as A, SceneMenuAction as S};
        let scene = self.game_workbench.scene.clone();
        match action {
            A::Open(id) => self.request_scene_command(SceneCommand::Open(id), ctx),
            A::Rename(object) => {
                self.request_scene_command(SceneCommand::RenameObject { scene, object }, ctx)
            }
            A::SceneMenu(command) => self.request_scene_command(
                match command {
                    S::New => SceneCommand::New,
                    S::Duplicate => SceneCommand::Duplicate(scene),
                    S::Rename => SceneCommand::Rename(scene),
                    S::Move => SceneCommand::Move(scene),
                    S::Delete => SceneCommand::Delete(scene),
                    S::SetEntry => SceneCommand::SetEntry(scene),
                    S::Reveal => SceneCommand::Reveal(scene),
                    S::Convert => SceneCommand::Convert,
                },
                ctx,
            ),
            A::Add | A::Delete(_) => {
                if matches!(action, A::Delete(_)) {
                    self.require_resolved_module_tools("deleting an object")?;
                }
                if self.workflow.blocks_document_edits() || self.scene_source_ui.active() {
                    return Err("Resolve the active operation first".into());
                }
                self.cancel_active_scene_gestures();
                let mut candidate = self.document.clone();
                let source = candidate
                    .game
                    .as_mut()
                    .and_then(|g| g.scenes.get_mut(&scene))
                    .ok_or("No authored scene")?;
                let selection = match action {
                    A::Add => {
                        let id = crate::game_ui::next_object_id(source)?;
                        source.objects.push(AuthoredObject::new(WorldObject {
                            id,
                            parent: None,
                            transform: Transform::default(),
                            color: [190, 190, 190, 255],
                        }));
                        Some(id)
                    }
                    A::Delete(id) => {
                        if !source.objects.iter().any(|o| o.object.id == id) {
                            return Err("Object is missing".into());
                        }
                        crate::game_ui::delete_object(source, id);
                        None
                    }
                    _ => unreachable!(),
                };
                self.commit_hierarchy_document(
                    candidate,
                    if selection.is_some() {
                        "Create object"
                    } else {
                        "Delete object"
                    },
                )?;
                self.game_workbench.select(selection);
            }
        }
        Ok(())
    }
    pub(super) fn scene_id_for_asset(&self, asset_id: &str) -> Option<String> {
        self.document
            .game
            .as_ref()?
            .scene_assets
            .iter()
            .find(|(_, b)| b.id == asset_id)
            .map(|(id, _)| id.clone())
    }
    pub(super) fn open_authored_scene(&mut self, id: &str) {
        if self.playing || self.game_workbench.is_play_session() {
            self.status = "Stop Play before opening an authored scene".into();
            return;
        }
        if !self
            .document
            .game
            .as_ref()
            .is_some_and(|g| g.scenes.contains_key(id))
        {
            self.status = "Scene is not registered in this project".into();
            return;
        }
        if self.game_workbench.scene != id {
            self.cancel_active_scene_gestures();
            self.game_workbench.scene = id.into();
            self.game_workbench.select(None);
            self.game_workbench.invalidate_preview_assets();
        }
        self.open_window(EditorType::Scene);
        self.status = "Opened authored scene; game session is unchanged".into();
    }
    pub(super) fn request_scene_command(&mut self, command: SceneCommand, ctx: &egui::Context) {
        if self.playing
            || self.game_workbench.is_play_session()
            || self.workflow.blocks_document_edits()
        {
            return;
        }
        if self.scene_source_ui.active() {
            return;
        }
        if let SceneCommand::Open(id) = &command {
            self.open_authored_scene(id);
            return;
        }
        if let SceneCommand::Reveal(id) = &command {
            self.open_window(EditorType::Assets);
            if let Some(binding) = self
                .document
                .game
                .as_ref()
                .and_then(|g| g.scene_assets.get(id))
                && let Some(pane) = self.pending_area_focus.or_else(|| self.active_window())
            {
                self.assets
                    .reveal_source((self.workspace_key(), pane), &binding.id, &binding.path);
            }
            return;
        }
        if let SceneCommand::SetEntry(id) = &command {
            self.request_scene_transaction(SceneSourceEdit::SetEntry { scene: id.clone() }, ctx);
            return;
        }
        if self.scene_source_ui.active() {
            return;
        }
        self.cancel_active_scene_gestures();
        let Some(game) = &self.document.game else {
            self.status = "Create a game project first".into();
            return;
        };
        if matches!(&command,SceneCommand::Delete(id) if id==&game.entry_scene) {
            self.status = "Choose another entry scene before deleting this scene".into();
            return;
        }
        let id = match &command {
            SceneCommand::Duplicate(id)
            | SceneCommand::Rename(id)
            | SceneCommand::Move(id)
            | SceneCommand::Delete(id) => Some(id.as_str()),
            SceneCommand::RenameObject { scene, .. } => Some(scene.as_str()),
            _ => None,
        };
        let base = id.and_then(|id| {
            game.scenes
                .get(id)
                .map(|s| (s.clone(), game.scene_assets.get(id).cloned()))
        });
        if id.is_some() && base.is_none() {
            self.status = "Scene is missing".into();
            return;
        }
        let name = if let SceneCommand::RenameObject { object, .. } = &command {
            let Some(authored) = base
                .as_ref()
                .and_then(|v| v.0.objects.iter().find(|o| o.object.id == *object))
            else {
                self.status = "Object is missing".into();
                return;
            };
            authored.name.as_deref().unwrap_or("").to_owned()
        } else if matches!(command, SceneCommand::Duplicate(_)) {
            format!("{} copy", base.as_ref().unwrap().0.name)
        } else {
            base.as_ref()
                .map_or("New scene".into(), |v| v.0.name.clone())
        };
        let path = base
            .as_ref()
            .and_then(|v| v.1.as_ref())
            .map_or("", |v| v.path.as_str());
        self.scene_source_ui.dialog = Some(SceneDialog {
            command,
            name,
            filename: path.rsplit('/').next().unwrap_or("").into(),
            folder: path.rsplit_once('/').map_or("scenes", |v| v.0).into(),
            base,
            error: None,
            first_frame: true,
        });
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
    }
    pub(super) fn scene_file_menu(&mut self, ui: &mut egui::Ui) {
        let mut command = None;
        let ready = self.document.game.is_some()
            && !self.playing
            && !self.game_workbench.is_play_session()
            && !self.workflow.blocks_document_edits();
        ui.add_enabled_ui(ready,|ui| {
            ui.menu_button("Scene",|ui| {
                if ui.button("New scene…").clicked() { command=Some(SceneCommand::New);ui.close(); }
                ui.menu_button("Open scene",|ui| {
                    if let Some(game)=&self.document.game {
                        egui::ScrollArea::vertical().max_height(320.).show(ui,|ui| {
                            for (id,scene) in &game.scenes {
                                let name=if scene.name.is_empty(){id}else{&scene.name};
                                if ui.selectable_label(self.game_workbench.scene==*id,name).on_hover_text(format!("Scene ID: {id}")).clicked() { command=Some(SceneCommand::Open(id.clone()));ui.close(); }
                            }
                        });
                    }
                });
                if let Some(game)=&self.document.game {
                    let id=if game.scenes.contains_key(&self.game_workbench.scene){&self.game_workbench.scene}else{&game.entry_scene};
                    for (label,next) in [("Duplicate current scene…",SceneCommand::Duplicate(id.clone())),("Show current scene in Assets",SceneCommand::Reveal(id.clone())),("Set current scene as entry",SceneCommand::SetEntry(id.clone()))] {
                        if ui.add_enabled(!matches!(next,SceneCommand::Reveal(_)) || game.scene_assets.contains_key(id),egui::Button::new(label)).on_disabled_hover_text("Convert embedded scenes to files first.").clicked(){command=Some(next);ui.close();}
                    }
                    for (label,next) in [("Rename current scene…",SceneCommand::Rename(id.clone())),("Move current scene file…",SceneCommand::Move(id.clone())),("Delete current scene…",SceneCommand::Delete(id.clone()))] {
                        let allowed=game.scene_assets.contains_key(id) && (!matches!(next,SceneCommand::Delete(_)) || id!=&game.entry_scene);
                        if ui.add_enabled(allowed,egui::Button::new(label)).on_disabled_hover_text("External scene files are required; choose another entry before deleting the current entry scene.").clicked() {command=Some(next);ui.close();}
                    }
                    if game.scene_assets.is_empty() {
                        ui.separator();
                        if ui.button("Convert embedded scenes to files…").clicked(){command=Some(SceneCommand::Convert);ui.close();}
                    }
                }
            });
        });
        if let Some(command) = command {
            self.request_scene_command(command, ui.ctx());
        }
    }
    pub(super) fn draw_scene_source_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.scene_source_ui.dialog.take() else {
            return;
        };
        let (title, apply) = match dialog.command {
            SceneCommand::New => ("New scene", "Create scene"),
            SceneCommand::Duplicate(_) => ("Duplicate scene", "Create copy"),
            SceneCommand::Rename(_) => ("Rename scene", "Rename"),
            SceneCommand::Move(_) => ("Move scene file", "Move"),
            SceneCommand::Delete(_) => ("Delete scene file", "Move to trash"),
            SceneCommand::Convert => ("Convert embedded scenes", "Convert"),
            SceneCommand::RenameObject { .. } => ("Rename object", "Rename"),
            _ => unreachable!(),
        };
        let mut submit = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("authored-scene-file-dialog")).show(ctx,|ui| {
            ui.set_width((ctx.content_rect().width()-48.).clamp(120.,480.));
            ui.heading(title);ui.add_space(12.);
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-180.).max(48.)).show(ui,|ui| {
            if let Some((source,binding))=&dialog.base {
                let id=match &dialog.command {SceneCommand::Duplicate(id)|SceneCommand::Rename(id)|SceneCommand::Move(id)|SceneCommand::Delete(id)=>Some(id.as_str()),SceneCommand::RenameObject{scene,..}=>Some(scene.as_str()),_=>None};
                if let Some(id)=id {ui.label(egui::RichText::new(format!("Scene: {} · ID: {id}",source.name)).small());}
                if let Some(binding)=binding {ui.label(egui::RichText::new(format!("Source: {}",binding.path)).small());}
            }
            match &dialog.command {
                SceneCommand::RenameObject{object,..}=>{
                    ui.label(format!("Object ID: {object}"));ui.label("Name");
                    let response=ui.add(egui::TextEdit::singleline(&mut dialog.name).desired_width(f32::INFINITY).hint_text("Unnamed object"));
                    if dialog.first_frame{response.request_focus();}
                    ui.label("Display name only. Object identity and gameplay data stay the same.");
                }
                SceneCommand::New|SceneCommand::Duplicate(_)|SceneCommand::Rename(_) => {
                    ui.label("Scene name");let field=ui.add(egui::TextEdit::singleline(&mut dialog.name).desired_width(f32::INFINITY));if dialog.first_frame {field.request_focus();}
                    if matches!(dialog.command,SceneCommand::Rename(_)) {
                        ui.label("File name (.scene.json)");ui.add(egui::TextEdit::singleline(&mut dialog.filename).desired_width(f32::INFINITY));
                    }
                    if self.document.game.as_ref().is_some_and(|g|g.scene_assets.is_empty()) {
                        ui.label("This writes the current embedded scenes as independent files too. Scene and object IDs are retained.");
                    }
                }
                SceneCommand::Move(_) => {ui.label("Project-relative folder inside scenes/");ui.add(egui::TextEdit::singleline(&mut dialog.folder).desired_width(f32::INFINITY));}
                SceneCommand::Delete(_) => {ui.label(format!("Delete “{}” and move its source to project trash?",dialog.name));ui.label("Entry scene and source-owned references must remain valid. Objects in other scenes are preserved.");}
                SceneCommand::Convert => {ui.label("Write every current authored scene into its own file and keep references in this project. Unsaved authored content is preserved. Opening an old project never performs this conversion automatically.");}
                _=>{}
            }
            if let Some(error)=&dialog.error {ui.colored_label(egui::Color32::from_rgb(255,159,159),error);}
            });
            ui.add_space(16.);
            ui.horizontal(|ui| {
                let response=ui.add(egui::Button::new(egui::RichText::new(apply).color(egui::Color32::from_rgb(14,34,36))).fill(editor_ui::ACCENT));
                #[cfg(test)]
                self.scene_source_ui.targets.insert("apply".into(),response.rect);
                submit=response.clicked();
                let response=ui.button("Cancel");
                #[cfg(test)]
                self.scene_source_ui.targets.insert("cancel".into(),response.rect);
                cancel=response.clicked();
            });
            if !dialog.first_frame { submit|=ui.input_mut(|i|i.consume_key(egui::Modifiers::NONE,Key::Enter)); }
            cancel|=ui.input_mut(|i|i.consume_key(egui::Modifiers::NONE,Key::Escape));
        });
        dialog.first_frame = false;
        if cancel {
            return;
        }
        if !submit {
            self.scene_source_ui.dialog = Some(dialog);
            return;
        }
        if let SceneCommand::RenameObject { scene, object } = &dialog.command {
            let result = (|| {
                let live = self
                    .document
                    .game
                    .as_ref()
                    .and_then(|g| g.scenes.get(scene))
                    .ok_or("Scene is missing")?;
                if dialog.base.as_ref().map(|b| &b.0) != Some(live) {
                    return Err(
                        "Scene changed while this dialog was open; cancel and reopen it".into(),
                    );
                }
                let original = live
                    .objects
                    .iter()
                    .find(|o| o.object.id == *object)
                    .ok_or("Object is missing")?;
                if original.name.as_deref().unwrap_or("") == dialog.name {
                    return Ok(());
                }
                let mut candidate = self.document.clone();
                let target = candidate
                    .game
                    .as_mut()
                    .unwrap()
                    .scenes
                    .get_mut(scene)
                    .unwrap()
                    .objects
                    .iter_mut()
                    .find(|o| o.object.id == *object)
                    .unwrap();
                target.name = if dialog.name.is_empty() {
                    AuthoredName::Absent
                } else {
                    AuthoredName::Text(dialog.name.clone())
                };
                self.commit_hierarchy_document(candidate, "Rename object")?;
                Ok::<_, String>(())
            })();
            if let Err(error) = result {
                dialog.error = Some(error);
                self.scene_source_ui.dialog = Some(dialog);
            }
            return;
        }
        let edit = match &dialog.command {
            SceneCommand::New => SceneSourceEdit::New {
                name: dialog.name.clone(),
                duplicate: None,
            },
            SceneCommand::Duplicate(id) => SceneSourceEdit::New {
                name: dialog.name.clone(),
                duplicate: Some(id.clone()),
            },
            SceneCommand::Rename(id) => SceneSourceEdit::Rename {
                scene: id.clone(),
                name: dialog.name.clone(),
                filename: dialog.filename.clone(),
            },
            SceneCommand::Move(id) => SceneSourceEdit::Move {
                scene: id.clone(),
                folder: dialog.folder.clone(),
            },
            SceneCommand::Delete(id) => SceneSourceEdit::Delete { scene: id.clone() },
            SceneCommand::Convert => SceneSourceEdit::Convert,
            _ => unreachable!(),
        };
        if matches!(
            edit,
            SceneSourceEdit::New { .. } | SceneSourceEdit::Rename { .. }
        ) && dialog.name.trim().is_empty()
        {
            dialog.error = Some("Enter a scene name".into());
            self.scene_source_ui.dialog = Some(dialog);
            return;
        }
        let id = match &dialog.command {
            SceneCommand::Duplicate(id)
            | SceneCommand::Rename(id)
            | SceneCommand::Move(id)
            | SceneCommand::Delete(id) => Some(id),
            _ => None,
        };
        if let Some(id) = id {
            let live = self.document.game.as_ref().and_then(|g| {
                g.scenes
                    .get(id)
                    .map(|s| (s.clone(), g.scene_assets.get(id).cloned()))
            });
            if live != dialog.base {
                dialog.error =
                    Some("Scene changed while this dialog was open; cancel and reopen it".into());
                self.scene_source_ui.dialog = Some(dialog);
                return;
            }
        }
        self.request_scene_transaction(edit, ctx);
    }
}
