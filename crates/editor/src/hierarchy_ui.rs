//! Authored object navigation. Drawing never clones or edits the document.
use super::*;
use crate::app::editor_ui;

#[derive(Default)]
pub(super) struct HierarchyState {
    search: String,
    collapsed: BTreeSet<(String, u64)>,
    #[cfg(test)]
    targets: BTreeMap<u64, egui::Rect>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HierarchyAction {
    Open(String),
    Add,
    Delete(u64),
    Rename(u64),
    SceneMenu(SceneMenuAction),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SceneMenuAction {
    New,
    Duplicate,
    Rename,
    Move,
    Delete,
    SetEntry,
    Reveal,
    Convert,
}

pub(super) fn object_label(object: &AuthoredObject) -> String {
    object
        .name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .or(object.prefab.as_deref())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Object {}", object.object.id))
}

/// Filter includes ancestors so a result never loses its parent context.
fn visible_rows(
    scene: &AuthoredScene,
    query: &str,
    collapsed: &BTreeSet<u64>,
) -> Vec<(usize, usize)> {
    let query = query.trim().to_lowercase();
    let by_id: BTreeMap<_, _> = scene
        .objects
        .iter()
        .enumerate()
        .map(|(i, o)| (o.object.id, i))
        .collect();
    let mut children: BTreeMap<Option<u64>, Vec<usize>> = BTreeMap::new();
    let mut included = BTreeSet::new();
    for (index, object) in scene.objects.iter().enumerate() {
        children
            .entry(object.object.parent)
            .or_default()
            .push(index);
        if !query.is_empty()
            && (object_label(object).to_lowercase().contains(&query)
                || object.object.id.to_string().contains(&query)
                || object
                    .components
                    .keys()
                    .any(|key| key.to_lowercase().contains(&query)))
        {
            let mut next = Some(object.object.id);
            while let Some(id) = next {
                if !included.insert(id) {
                    break;
                }
                next = by_id.get(&id).and_then(|i| scene.objects[*i].object.parent);
            }
        }
    }
    let mut pending: Vec<_> = children
        .get(&None)
        .into_iter()
        .flatten()
        .rev()
        .map(|i| (*i, 0))
        .collect();
    let mut rows = Vec::new();
    while let Some((index, depth)) = pending.pop() {
        let object = &scene.objects[index];
        if !query.is_empty() && !included.contains(&object.object.id) {
            continue;
        }
        rows.push((index, depth));
        if (!query.is_empty() || !collapsed.contains(&object.object.id))
            && let Some(list) = children.get(&Some(object.object.id))
        {
            pending.extend(list.iter().rev().map(|i| (*i, depth + 1)));
        }
    }
    rows
}

impl GameWorkbench {
    pub fn hierarchy_ui(
        &mut self,
        ui: &mut egui::Ui,
        document: &GraphDocument,
    ) -> Option<HierarchyAction> {
        let Some(game) = &document.game else {
            ui.label("Open or create a game project through File.");
            return None;
        };
        self.normalize_selection(game);
        let scene = &game.scenes[&self.scene];
        let mut action = None;
        let editable = !self.play_session;
        let compact = ui.available_height() < 180.;
        #[cfg(test)]
        self.hierarchy.targets.clear();
        let name = if scene.name.is_empty() {
            &self.scene
        } else {
            &scene.name
        };
        let caption = if compact && !self.hierarchy.search.trim().is_empty() {
            format!("{name} · filtered")
        } else {
            name.clone()
        };
        ui.horizontal(|ui| {
            let width=(ui.available_width()-42.).max(64.);
            let mut selected=self.scene.clone();
            egui::ComboBox::from_id_salt("hierarchy-scene")
                .width(width).selected_text(caption)
                .show_ui(ui,|ui| { for (id,s) in &game.scenes {
                    ui.add_enabled_ui(editable,|ui|{ui.selectable_value(&mut selected,id.clone(),if s.name.is_empty(){id}else{&s.name});});
                }}).response.on_hover_text(format!("Scene ID: {}\n{}",self.scene,game.scene_assets.get(&self.scene).map_or("Embedded scene — File → Scene can convert it to a file",|b|b.path.as_str())));
            if selected!=self.scene { action=Some(HierarchyAction::Open(selected)); }
            ui.menu_button("…",|ui| {
                if compact {
                    ui.add(egui::TextEdit::singleline(&mut self.hierarchy.search).hint_text("Search objects").desired_width(220.));
                    if ui.add_enabled(editable,egui::Button::new("Add object")).clicked(){action=Some(HierarchyAction::Add);ui.close();}
                    ui.separator();
                }
                for (label,command,allowed) in [
                    ("New scene…",SceneMenuAction::New,true),
                    ("Duplicate scene…",SceneMenuAction::Duplicate,true),
                    ("Show in Assets",SceneMenuAction::Reveal,!game.scene_assets.is_empty()),
                    ("Set as entry",SceneMenuAction::SetEntry,game.entry_scene!=self.scene),
                    ("Rename scene…",SceneMenuAction::Rename,!game.scene_assets.is_empty()),
                    ("Move scene file…",SceneMenuAction::Move,!game.scene_assets.is_empty()),
                    ("Delete scene…",SceneMenuAction::Delete,!game.scene_assets.is_empty()&&game.entry_scene!=self.scene),
                    ("Convert embedded scenes to files…",SceneMenuAction::Convert,game.scene_assets.is_empty()),
                ] { if ui.add_enabled(editable&&allowed,egui::Button::new(label)).on_disabled_hover_text("Stop Play; file actions require external scenes, and the entry scene cannot be deleted.").clicked(){action=Some(HierarchyAction::SceneMenu(command));ui.close();} }
            }).response.on_hover_text(if compact{"Scene actions, object search and Add object"}else{"Scene file actions"});
        });
        if !compact {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.hierarchy.search)
                        .hint_text("Search objects")
                        .desired_width((ui.available_width() - 40.).max(40.)),
                );
                if editor_ui::icon_button(ui, editor_ui::Icon::Plus, "Add object", editable, false)
                    .clicked()
                {
                    action = Some(HierarchyAction::Add);
                }
            });
            ui.label(
                egui::RichText::new(format!(
                    "{} objects{}",
                    scene.objects.len(),
                    if game.entry_scene == self.scene {
                        " · Entry scene"
                    } else {
                        ""
                    }
                ))
                .small()
                .color(editor_ui::SECONDARY),
            );
        }
        let collapsed = self
            .hierarchy
            .collapsed
            .iter()
            .filter(|(s, _)| s == &self.scene)
            .map(|(_, id)| *id)
            .collect();
        let rows = visible_rows(scene, &self.hierarchy.search, &collapsed);
        let parents: BTreeSet<_> = scene
            .objects
            .iter()
            .filter_map(|o| o.object.parent)
            .collect();
        ui.spacing_mut().item_spacing.y = 0.;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, 28., rows.len(), |ui, range| {
                for row in range {
                    let (index, depth) = rows[row];
                    let object = &scene.objects[index];
                    let id = object.object.id;
                    let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width(), 28.));
                    let response = ui.interact(
                        rect,
                        ui.id().with(("hierarchy-object", &self.scene, id)),
                        egui::Sense::click(),
                    );
                    #[cfg(test)]
                    self.hierarchy.targets.insert(id, rect);
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::SelectableLabel,
                            true,
                            self.selected == Some(id),
                            object_label(object),
                        )
                    });
                    if self.selected == Some(id) {
                        ui.painter()
                            .rect_filled(rect, 3., ui.visuals().selection.bg_fill);
                        ui.painter().line_segment(
                            [
                                rect.left_top() + egui::vec2(1., 3.),
                                rect.left_bottom() - egui::vec2(-1., 3.),
                            ],
                            egui::Stroke::new(2., editor_ui::ACCENT),
                        );
                    } else if response.hovered() {
                        ui.painter().rect_filled(rect, 3., editor_ui::RAISED);
                    }
                    if response.has_focus() {
                        ui.painter().rect_stroke(
                            rect.shrink(1.),
                            3.,
                            egui::Stroke::new(1., editor_ui::ACCENT),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let indent = (depth as f32 * 16.).min((rect.width() - 120.).max(0.));
                    let disclosure = egui::Rect::from_min_size(
                        rect.min + egui::vec2(indent, 4.),
                        egui::vec2(20., 20.),
                    );
                    let has_children = parents.contains(&id);
                    if has_children {
                        let center = disclosure.center();
                        let points =
                            if collapsed.contains(&id) && self.hierarchy.search.trim().is_empty() {
                                vec![
                                    center + egui::vec2(-2., -4.),
                                    center + egui::vec2(3., 0.),
                                    center + egui::vec2(-2., 4.),
                                ]
                            } else {
                                vec![
                                    center + egui::vec2(-4., -2.),
                                    center + egui::vec2(4., -2.),
                                    center + egui::vec2(0., 3.),
                                ]
                            };
                        ui.painter().add(egui::Shape::convex_polygon(
                            points,
                            editor_ui::SECONDARY,
                            egui::Stroke::NONE,
                        ));
                    }
                    let text_rect = egui::Rect::from_min_max(
                        disclosure.right_top() + egui::vec2(4., -4.),
                        rect.right_bottom() - egui::vec2(8., 0.),
                    );
                    let mut job = egui::text::LayoutJob::simple_singleline(
                        object_label(object),
                        egui::FontId::proportional(14.),
                        editor_ui::TEXT,
                    );
                    job.wrap.max_width = text_rect.width();
                    job.wrap.max_rows = 1;
                    job.wrap.break_anywhere = true;
                    let galley = ui.painter().layout_job(job);
                    ui.painter().with_clip_rect(text_rect).galley(
                        text_rect.left_center() - egui::vec2(0., galley.size().y / 2.),
                        galley,
                        editor_ui::TEXT,
                    );
                    if response.clicked() {
                        response.request_focus();
                        if has_children
                            && response
                                .interact_pointer_pos()
                                .is_some_and(|p| disclosure.contains(p))
                        {
                            let key = (self.scene.clone(), id);
                            if !self.hierarchy.collapsed.remove(&key) {
                                self.hierarchy.collapsed.insert(key);
                            }
                        } else {
                            self.select(Some(id));
                        }
                    }
                    if response.has_focus() && editable && self.selected == Some(id) {
                        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F2)) {
                            action = Some(HierarchyAction::Rename(id));
                        }
                        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete))
                        {
                            action = Some(HierarchyAction::Delete(id));
                        }
                    }
                    response.context_menu(|ui| {
                        if ui
                            .add_enabled(editable, egui::Button::new("Rename…").shortcut_text("F2"))
                            .clicked()
                        {
                            self.select(Some(id));
                            action = Some(HierarchyAction::Rename(id));
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                editable,
                                egui::Button::new("Delete object and children")
                                    .shortcut_text("Delete"),
                            )
                            .clicked()
                        {
                            self.select(Some(id));
                            action = Some(HierarchyAction::Delete(id));
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Copy object ID").clicked() {
                            ui.ctx().copy_text(id.to_string());
                            ui.close();
                        }
                    });
                    response.on_hover_text(format!(
                        "{}\nObject ID: {id}\nParent: {}\nPrefab: {}\nComponents: {}",
                        object_label(object),
                        object
                            .object
                            .parent
                            .map_or("Scene".into(), |p| p.to_string()),
                        object.prefab.as_deref().unwrap_or("None"),
                        object
                            .components
                            .keys()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            });
        if rows.is_empty() {
            ui.label(if scene.objects.is_empty() {
                "This scene is empty. Add an object to begin."
            } else {
                "No matching objects. Clear the search to show the scene."
            });
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filter_retains_ancestors_and_temporarily_reveals_collapsed_matches() {
        let mut scene = AuthoredScene::default();
        for (id, parent, name) in [(1, None, "Room"), (2, Some(1), "Lamp"), (3, None, "Door")] {
            let mut object = AuthoredObject::new(WorldObject {
                id,
                parent,
                transform: Transform::default(),
                color: [255; 4],
            });
            object.name = AuthoredName::Text(name.into());
            scene.objects.push(object);
        }
        let collapsed = BTreeSet::from([1]);
        assert_eq!(visible_rows(&scene, "", &collapsed), vec![(0, 0), (2, 0)]);
        assert_eq!(
            visible_rows(&scene, "lamp", &collapsed),
            vec![(0, 0), (1, 1)]
        );
        assert_eq!(visible_rows(&scene, "missing", &collapsed), vec![]);
        assert_eq!(scene.objects[1].name.as_deref(), Some("Lamp"));
    }
    #[test]
    fn short_hierarchy_keeps_a_visible_row_instead_of_only_controls() {
        let ctx = egui::Context::default();
        editor_ui::configure(&ctx);
        let mut workbench = GameWorkbench::default();
        let mut document = module_engine_core::demo_document(WorldKind::Objects);
        document.game = Some(module_engine_editor_composition::two_arenas::demo_project());
        let before = document.clone();
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300., 80.));
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                ..Default::default()
            },
            |ui| {
                workbench.hierarchy_ui(ui, &document);
            },
        )
        .drop_without_applying_deltas();
        assert!(
            workbench
                .hierarchy
                .targets
                .values()
                .any(|rect| bounds.contains_rect(*rect)),
            "At least one full 28-point object row must remain usable"
        );
        assert!(document == before);
        assert!(workbench.preview_host.is_none());
    }
    #[test]
    fn full_width_row_selection_f2_and_text_delete_have_separate_owners() {
        let ctx = egui::Context::default();
        editor_ui::configure(&ctx);
        let mut workbench = GameWorkbench::default();
        let mut document = module_engine_core::demo_document(WorldKind::Objects);
        document.game = Some(module_engine_editor_composition::two_arenas::demo_project());
        workbench.scene = "arena".into();
        workbench.select(None);
        let before = document.clone();
        fn draw(
            ctx: &egui::Context,
            w: &mut GameWorkbench,
            d: &GraphDocument,
            events: Vec<egui::Event>,
        ) -> Option<HierarchyAction> {
            let mut action = None;
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(320., 480.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| action = w.hierarchy_ui(ui, d),
            )
            .drop_without_applying_deltas();
            action
        }
        draw(&ctx, &mut workbench, &document, vec![]);
        let rect = workbench.hierarchy.targets[&1];
        assert!(rect.width() > 290.);
        let pos = egui::pos2(rect.right() - 28., rect.center().y);
        for pressed in [true, false] {
            draw(
                &ctx,
                &mut workbench,
                &document,
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
        assert_eq!(workbench.selected(), Some(1));
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        assert_eq!(
            draw(&ctx, &mut workbench, &document, vec![key(egui::Key::F2)]),
            Some(HierarchyAction::Rename(1))
        );
        assert_eq!(
            draw(
                &ctx,
                &mut workbench,
                &document,
                vec![key(egui::Key::Delete)]
            ),
            Some(HierarchyAction::Delete(1))
        );
        workbench.hierarchy.search = "no result".into();
        assert!(
            draw(
                &ctx,
                &mut workbench,
                &document,
                vec![key(egui::Key::Delete)]
            )
            .is_none()
        );
        assert!(document == before);
        assert!(workbench.preview_host.is_none());
        assert!(workbench.host.is_none());
    }
}
