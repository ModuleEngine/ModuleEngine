//! Asset browser presentation. The host owns scanning, previews and file transactions.
//!
//! Every operation is returned as a command; rendering this window never touches the
//! filesystem or authored scene. In particular, cancellation cannot bypass the host's
//! save guard, and a drag payload does not initialize a renderer.
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::collections::{BTreeMap, BTreeSet};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum AssetKind {
    Model,
    Material,
    Texture,
    Audio,
    Shader,
    Other,
    Scene,
}
impl AssetKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Material => "Material",
            Self::Texture => "Texture",
            Self::Audio => "Audio",
            Self::Shader => "Shader",
            Self::Other => "Other file",
            Self::Scene => "Scene",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Model => "3D",
            Self::Material => "MAT",
            Self::Texture => "IMG",
            Self::Audio => "WAV",
            Self::Shader => "HLSL",
            Self::Other => "FILE",
            Self::Scene => "SCENE",
        }
    }
    fn can_preview(self) -> bool {
        matches!(self, Self::Model | Self::Texture)
    }
}

/// Immutable, project-relative description supplied by the asset service adapter.
#[derive(Clone, Debug)]
pub struct AssetTile {
    pub id: String,
    pub reference: String,
    pub path: String,
    pub kind: AssetKind,
    pub size: u64,
    pub protected: bool,
    /// Source-scene policy, independent of immutable-resource protection.
    pub scene_entry: bool,
    pub preview: Option<egui::TextureId>,
    pub preview_status: Option<String>,
    pub usages: Vec<String>,
    pub metadata: Vec<(String, String)>,
}
impl AssetTile {
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
    fn drag(&self) -> AssetDrag {
        AssetDrag {
            id: self.id.clone(),
            reference: self.reference.clone(),
            path: self.path.clone(),
            kind: self.kind,
        }
    }
}

/// Scene/Inspector only accept the applicable kind; folders accept any file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetDrag {
    pub id: String,
    pub reference: String,
    pub path: String,
    pub kind: AssetKind,
}

#[derive(Clone, Debug)]
pub struct TrashTile {
    pub id: String,
    pub path: String,
    pub kind: AssetKind,
    pub deleted_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneAssetAction {
    Open,
    Duplicate,
    Rename,
    Move,
    Delete,
    SetEntry,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserAction {
    Scene {
        asset: String,
        action: SceneAssetAction,
    },
    Import {
        folder: String,
    },
    Refresh,
    NewFolder {
        path: String,
    },
    NewMaterial {
        path: String,
    },
    OpenMaterial {
        id: String,
    },
    SaveMaterial {
        id: String,
        expected_hash: String,
        bytes: Vec<u8>,
    },
    Rename {
        id: String,
        name: String,
    },
    Move {
        id: String,
        folder: String,
    },
    Delete {
        id: String,
        replacement: Option<String>,
        detach: bool,
    },
    Restore {
        trash_id: String,
    },
    Select {
        id: String,
    },
    Preview {
        id: String,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DeleteMode {
    #[default]
    Undecided,
    Detach,
    Replace,
}
#[derive(Clone, Debug)]
enum BrowserDialog {
    NewFolder {
        name: String,
    },
    NewMaterial {
        name: String,
    },
    Rename {
        id: String,
        name: String,
    },
    Move {
        id: String,
        folder: String,
    },
    Delete {
        id: String,
        mode: DeleteMode,
        replacement: Option<String>,
    },
    Usages {
        id: String,
    },
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrowserNavigation {
    pub folder: String,
    pub search: String,
    pub kind_filter: Option<AssetKind>,
    pub grid: bool,
    pub selected: Option<String>,
    pub show_address: bool,
    pub show_details: bool,
}
/// Small local UI state. Asset/project data remains outside the browser.
pub struct BrowserState {
    /// Canonical host-supplied location, used only for copying an absolute path.
    pub project_root: Option<std::path::PathBuf>,
    pub folder: String,
    pub search: String,
    pub kind_filter: Option<AssetKind>,
    pub grid: bool,
    pub selected: Option<String>,
    show_trash: bool,
    show_address: bool,
    show_details: bool,
    dialog: Option<BrowserDialog>,
    preview_requests: BTreeSet<String>,
    /// Last frame hit targets, useful for the host's deterministic UI diagnostics.
    pub hit_targets: BTreeMap<String, Rect>,
}
impl Default for BrowserState {
    fn default() -> Self {
        Self {
            project_root: None,
            folder: "assets".into(),
            search: String::new(),
            kind_filter: None,
            grid: true,
            selected: None,
            show_trash: false,
            show_address: false,
            show_details: false,
            dialog: None,
            preview_requests: BTreeSet::new(),
            hit_targets: BTreeMap::new(),
        }
    }
}
impl BrowserState {
    pub fn navigation(&self) -> BrowserNavigation {
        BrowserNavigation {
            folder: self.folder.clone(),
            search: self.search.clone(),
            kind_filter: self.kind_filter,
            grid: self.grid,
            selected: self.selected.clone(),
            show_address: self.show_address,
            show_details: self.show_details,
        }
    }
    pub fn restore_navigation(&mut self, nav: BrowserNavigation) {
        self.folder = nav.folder;
        self.search = nav.search;
        self.kind_filter = nav.kind_filter;
        self.grid = nav.grid;
        self.selected = nav.selected;
        self.show_address = nav.show_address;
        self.show_details = nav.show_details;
    }

    #[cfg(test)]
    pub fn has_dialog(&self) -> bool {
        self.dialog.is_some()
    }
    /// A changed asset needs one new preview request instead of one per UI frame.
    pub fn invalidate_preview(&mut self, id: &str) {
        self.preview_requests.remove(id);
    }
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        assets: &[AssetTile],
        folders: &[String],
        trash: &[TrashTile],
        busy: bool,
    ) -> Vec<BrowserAction> {
        let mut actions = vec![];
        self.hit_targets.clear();
        let folders = all_folders(assets, folders);
        if !folders.contains(&self.folder) {
            self.folder = String::new();
        }
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !assets.iter().any(|asset| &asset.id == id))
        {
            self.selected = None;
        }
        ui.add_enabled_ui(!busy, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(!self.show_trash, "Assets").clicked() {
                    self.show_trash = false;
                }
                if ui
                    .selectable_label(self.show_trash, format!("Trash ({})", trash.len()))
                    .clicked()
                {
                    self.show_trash = true;
                }
                ui.separator();
                let source_folder=self.folder=="scenes" || self.folder.starts_with("scenes/");
                let import = ui.add_enabled(!source_folder,egui::Button::new("Import…")).on_disabled_hover_text("Import models/textures into assets/. Create scene files through File → Scene.");
                self.hit_targets.insert("import".into(), import.rect);
                if import.clicked() {
                    actions.push(BrowserAction::Import {
                        folder: writable_folder(&self.folder),
                    });
                }
                if ui.button("Refresh").clicked() {
                    self.preview_requests.clear();
                    actions.push(BrowserAction::Refresh);
                }
                if ui
                    .add_enabled(!self.show_trash && !source_folder, egui::Button::new("New folder…"))
                    .on_disabled_hover_text("Move a scene to create a source-scene folder.")
                    .clicked()
                {
                    self.dialog = Some(BrowserDialog::NewFolder {
                        name: String::new(),
                    });
                }
                if ui.add_enabled(!self.show_trash && !source_folder, egui::Button::new("New material…")).clicked() {
                    self.dialog = Some(BrowserDialog::NewMaterial { name: String::new() });
                }
            });
            ui.separator();
            if self.show_trash {
                self.trash_ui(ui, trash, &mut actions);
            } else {
                self.search_ui(ui);
                self.breadcrumb_ui(ui);
                let wide = ui.available_width() >= 840.0;
                let medium = ui.available_width() >= 480.0;
                let height = ui.available_height().max(80.0);
                if medium {
                    ui.horizontal_top(|ui| {
                        let folder_width = if wide { 200.0 } else { 160.0 };
                        ui.allocate_ui_with_layout(
                            Vec2::new(folder_width, height),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(folder_width);
                                egui::ScrollArea::vertical()
                                    .id_salt("asset-folders")
                                    .max_height(height)
                                    .show(ui, |ui| self.folder_tree(ui, &folders, &mut actions));
                            },
                        );
                        ui.separator();
                        let inspector_width = if wide { 240.0 } else { 0.0 };
                        let width = (ui.available_width() - inspector_width - 12.0).max(80.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(width, height),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(width);
                                self.file_list(
                                    ui,
                                    assets,
                                    &folders,
                                    if wide || !self.show_details {
                                        height
                                    } else {
                                        height * 0.58
                                    },
                                    &mut actions,
                                );
                                if !wide && self.show_details {
                                    ui.separator();
                                    egui::CollapsingHeader::new("Selected asset")
                                        .default_open(true)
                                        .show(ui, |ui| {
                                            egui::ScrollArea::vertical()
                                                .id_salt("asset-details-medium")
                                                .max_height(ui.available_height().max(60.0))
                                                .show(ui, |ui| {
                                                    self.details_ui(
                                                        ui,
                                                        assets,
                                                        &folders,
                                                        &mut actions,
                                                    )
                                                });
                                        });
                                }
                            },
                        );
                        if wide {
                            ui.separator();
                            ui.allocate_ui_with_layout(
                                Vec2::new(inspector_width, height),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_width(inspector_width);
                                    egui::ScrollArea::vertical()
                                        .id_salt("asset-details")
                                        .max_height(height)
                                        .show(ui, |ui| {
                                            self.details_ui(ui, assets, &folders, &mut actions)
                                        });
                                },
                            );
                        }
                    });
                } else {
                    egui::ComboBox::from_id_salt("asset-folder-compact")
                        .selected_text(folder_label(&self.folder))
                        .width(ui.available_width().max(80.0))
                        .show_ui(ui, |ui| {
                            for folder in &folders {
                                ui.selectable_value(
                                    &mut self.folder,
                                    folder.clone(),
                                    folder_label(folder),
                                );
                            }
                        });
                    self.file_list(
                        ui,
                        assets,
                        &folders,
                        if self.show_details {
                            height * 0.58
                        } else {
                            height
                        },
                        &mut actions,
                    );
                    if self.show_details {
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .id_salt("asset-details-compact")
                            .max_height((height * 0.4).max(60.0))
                            .show(ui, |ui| self.details_ui(ui, assets, &folders, &mut actions));
                    }
                }
            }
        });
        self.dialog_ui(ui.ctx(), ui.id(), assets, &folders, busy, &mut actions);
        actions
    }
    fn search_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let width = (ui.available_width() - 280.0).max(80.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search this folder and subfolders")
                    .desired_width(width),
            );
            egui::ComboBox::from_id_salt("asset-kind")
                .selected_text(self.kind_filter.map_or("All types", AssetKind::label))
                .width(90.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.kind_filter, None, "All types");
                    for kind in [
                        AssetKind::Model,
                        AssetKind::Material,
                        AssetKind::Texture,
                        AssetKind::Audio,
                        AssetKind::Shader,
                        AssetKind::Other,
                        AssetKind::Scene,
                    ] {
                        ui.selectable_value(&mut self.kind_filter, Some(kind), kind.label());
                    }
                });
            ui.selectable_value(&mut self.grid, true, "Tiles");
            ui.selectable_value(&mut self.grid, false, "List");
        });
    }
    fn breadcrumb_ui(&mut self, ui: &mut egui::Ui) {
        use crate::app::editor_ui::{self, Icon};
        let current = self.folder.clone();
        let compact = ui.available_width() < 840.;
        ui.horizontal_wrapped(|ui| {
            if editor_ui::icon_button(ui,Icon::Folder,if self.show_address{"Show folder breadcrumbs"}else{"Show selectable folder address"},true,self.show_address).clicked(){self.show_address = !self.show_address;}
            if self.show_address {
                let mut address: &str=if current.is_empty(){"."}else{&current};
                let response=ui.add(egui::TextEdit::singleline(&mut address).desired_width((ui.available_width()-80.).max(80.)));
                response.clone().on_hover_text("Read-only project-relative folder path. Select and copy; this is not an asset reference.");
                if response.lost_focus() || ui.input(|i|i.key_pressed(egui::Key::Escape)){self.show_address=false;}
            } else {
                if ui.small_button("Project").clicked(){self.folder.clear();}
                let mut path=String::new();
                for part in current.split('/').filter(|s|!s.is_empty()) {ui.label("/");if !path.is_empty(){path.push('/');}path.push_str(part);if ui.small_button(part).clicked(){self.folder=path.clone();}}
            }
            let copy=editor_ui::icon_button(ui,Icon::Copy,"Copy folder path relative to project",true,false);
            if copy.clicked(){ui.ctx().copy_text(if current.is_empty(){".".into()}else{current.clone()});}
            copy.context_menu(|ui|{
                if let Some(root)=&self.project_root && ui.button("Copy absolute folder path").clicked(){ui.ctx().copy_text(root.join(&current).display().to_string());ui.close();}
                ui.label("Folder paths are not asset references.");
            });
            if compact {ui.toggle_value(&mut self.show_details,"Details");}
        });
    }
    fn folder_tree(
        &mut self,
        ui: &mut egui::Ui,
        folders: &BTreeSet<String>,
        actions: &mut Vec<BrowserAction>,
    ) {
        for folder in folders {
            let depth = if folder.is_empty() {
                0
            } else {
                folder.split('/').count()
            };
            ui.horizontal(|ui| {
                ui.add_space(((depth.saturating_sub(1) * 16) as f32).min(64.0));
                let name = if folder.is_empty() {
                    "Project"
                } else {
                    folder.rsplit('/').next().unwrap_or(folder)
                };
                let response = crate::app::editor_ui::type_label(
                    ui,
                    crate::app::editor_ui::Icon::Folder,
                    name,
                    self.folder == *folder,
                )
                .on_hover_text(folder_label(folder));
                self.hit_targets
                    .insert(format!("folder:{folder}"), response.rect);
                if response.clicked() {
                    self.folder = folder.clone();
                }
                if ui.is_enabled() {
                    if let Some(payload) = response.dnd_hover_payload::<AssetDrag>()
                        && accepts_drop(folder, payload.kind)
                    {
                        ui.painter().rect_stroke(
                            response.rect,
                            2.0,
                            Stroke::new(1.0, Color32::LIGHT_BLUE),
                            egui::StrokeKind::Inside,
                        );
                        response.clone().on_hover_text(format!(
                            "Move {} to {}",
                            payload.path,
                            folder_label(folder)
                        ));
                    }
                    if let Some(payload) = response.dnd_release_payload::<AssetDrag>()
                        && accepts_drop(folder, payload.kind)
                    {
                        actions.push(BrowserAction::Move {
                            id: payload.id.clone(),
                            folder: folder.clone(),
                        });
                    }
                }
            });
        }
    }
    fn file_list(
        &mut self,
        ui: &mut egui::Ui,
        assets: &[AssetTile],
        folders: &BTreeSet<String>,
        height: f32,
        actions: &mut Vec<BrowserAction>,
    ) {
        let visible: Vec<_> = assets.iter().filter(|asset| self.matches(asset)).collect();
        let children: Vec<_> = folders
            .iter()
            .filter(|folder| !folder.is_empty() && parent(folder) == self.folder)
            .cloned()
            .collect();
        ui.label(format!("Files: {}", visible.len()));
        egui::ScrollArea::vertical()
            .id_salt("asset-files")
            .auto_shrink([false, false])
            .max_height((height - 24.0).max(60.0))
            .show(ui, |ui| {
                if self.search.trim().is_empty() {
                    if self.grid {
                        ui.horizontal_wrapped(|ui| {
                            for folder in &children {
                                self.child_folder(ui, folder, height < 240., actions);
                            }
                        });
                    } else {
                        for folder in &children {
                            self.child_folder(ui, folder, height < 240., actions);
                        }
                    }
                }
                if self.grid {
                    ui.horizontal_wrapped(|ui| {
                        for asset in &visible {
                            self.tile(ui, asset, actions);
                        }
                    });
                } else {
                    for asset in &visible {
                        self.list_row(ui, asset, actions);
                    }
                }
                if visible.is_empty() && children.is_empty() {
                    ui.weak(if self.search.is_empty() && self.kind_filter.is_none() {
                        "This folder has no assets. Import a model, texture, or audio file."
                    } else {
                        "No assets found. Change the search or filter."
                    });
                }
            });
    }
    fn child_folder(
        &mut self,
        ui: &mut egui::Ui,
        folder: &str,
        compact: bool,
        actions: &mut Vec<BrowserAction>,
    ) {
        use crate::app::editor_ui::{self, Icon};
        let name = folder.rsplit('/').next().unwrap_or(folder);
        let response = if self.grid && !compact {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(96.), Sense::click());
            if response.hovered() {
                ui.painter().rect_filled(rect, 4, editor_ui::RAISED);
            }
            editor_ui::paint_icon(
                ui.painter(),
                Rect::from_center_size(rect.center_top() + Vec2::new(0., 30.), Vec2::splat(48.)),
                Icon::Folder,
                Color32::from_rgb(241, 196, 119),
            );
            let mut job = egui::text::LayoutJob::simple_singleline(
                name.into(),
                FontId::proportional(14.),
                editor_ui::TEXT,
            );
            job.wrap.max_width = 88.;
            job.wrap.max_rows = 2;
            job.wrap.break_anywhere = true;
            let galley = ui.painter().layout_job(job);
            ui.painter().galley(
                rect.left_top() + Vec2::new(4., 58.),
                galley,
                editor_ui::TEXT,
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    format!("Open folder {name}"),
                )
            });
            response
        } else if self.grid {
            let width = (name.chars().count() as f32 * 8. + 40.).clamp(72., 180.);
            ui.allocate_ui_with_layout(
                Vec2::new(width, 28.),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(width);
                    editor_ui::type_label(ui, Icon::Folder, name, false)
                },
            )
            .inner
        } else {
            editor_ui::type_label(ui, Icon::Folder, name, false)
        }
        .on_hover_text("Open folder; drag a file here to move it");
        self.hit_targets
            .insert(format!("folder-content:{folder}"), response.rect);
        if response.clicked() {
            self.folder = folder.into();
        }
        if ui.is_enabled()
            && let Some(payload) = response.dnd_release_payload::<AssetDrag>()
            && accepts_drop(folder, payload.kind)
        {
            actions.push(BrowserAction::Move {
                id: payload.id.clone(),
                folder: folder.into(),
            });
        }
    }
    fn matches(&self, asset: &AssetTile) -> bool {
        if self.kind_filter.is_some_and(|kind| kind != asset.kind) {
            return false;
        }
        let search = self.search.trim().to_lowercase();
        let folder_match = if search.is_empty() {
            parent(&asset.path) == self.folder
        } else {
            self.folder.is_empty() || asset.path.starts_with(&format!("{}/", self.folder))
        };
        folder_match
            && (search.is_empty()
                || asset.path.to_lowercase().contains(&search)
                || asset.id.to_lowercase().contains(&search)
                || (asset.kind == AssetKind::Scene
                    && asset.metadata.iter().any(|(key, value)| {
                        key == "Scene" && value.to_lowercase().contains(&search)
                    })))
    }
    fn tile(&mut self, ui: &mut egui::Ui, asset: &AssetTile, actions: &mut Vec<BrowserAction>) {
        let width = ui.available_width().clamp(64.0, 105.0);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, 123.0), Sense::click_and_drag());
        let selected = self.selected.as_ref() == Some(&asset.id);
        let fill = if selected {
            ui.visuals().selection.bg_fill
        } else if response.hovered() {
            ui.visuals().widgets.hovered.bg_fill
        } else {
            ui.visuals().faint_bg_color
        };
        ui.painter().rect_filled(rect.shrink(1.0), 4.0, fill);
        if selected {
            ui.painter().rect_stroke(
                rect.shrink(1.0),
                4.0,
                Stroke::new(1.0, ui.visuals().selection.stroke.color),
                egui::StrokeKind::Inside,
            );
        }
        let picture = Rect::from_min_max(
            rect.min + Vec2::new(7.0, 5.0),
            Pos2::new(rect.right() - 7.0, rect.top() + 76.0),
        );
        draw_preview(ui, picture, asset);
        ui.painter().text(
            Pos2::new(rect.left() + 7.0, rect.top() + 84.0),
            egui::Align2::LEFT_TOP,
            abbreviated(asset.name(), 14),
            FontId::proportional(12.0),
            ui.visuals().text_color(),
        );
        ui.painter().text(
            Pos2::new(rect.left() + 7.0, rect.top() + 103.0),
            egui::Align2::LEFT_TOP,
            asset_size(asset),
            FontId::proportional(11.0),
            ui.visuals().weak_text_color(),
        );
        if ui.is_enabled() && ui.is_rect_visible(rect) {
            self.request_preview(asset, actions);
        }
        self.asset_response(response, asset, actions);
    }
    fn list_row(&mut self, ui: &mut egui::Ui, asset: &AssetTile, actions: &mut Vec<BrowserAction>) {
        let selected = self.selected.as_ref() == Some(&asset.id);
        let label = format!(
            "{}  {}    {}",
            asset.kind.icon(),
            asset.name(),
            asset_size(asset)
        );
        let response = ui.add(
            egui::Button::new(label)
                .selected(selected)
                .truncate()
                .sense(Sense::click_and_drag())
                .min_size(Vec2::new(ui.available_width(), 27.0)),
        );
        self.asset_response(response, asset, actions);
    }
    fn asset_response(
        &mut self,
        response: egui::Response,
        asset: &AssetTile,
        actions: &mut Vec<BrowserAction>,
    ) {
        self.hit_targets
            .insert(format!("asset:{}", asset.id), response.rect);
        let response = response.on_hover_text(format!(
            "{}\n{} · {}\n{}",
            asset.path,
            asset.kind.label(),
            asset_size(asset),
            match asset.kind {
                AssetKind::Model =>
                    "Drag into Scene to create an object. Replace its model in Inspector.",
                AssetKind::Texture =>
                    "Drag onto an object in Scene or into Inspector to assign a texture.",
                AssetKind::Material => "Assign through Surface Material in Inspector.",
                _ => "Select a file to view details and usages.",
            }
        ));
        if response.double_clicked() && asset.kind == AssetKind::Scene {
            actions.push(BrowserAction::Scene {
                asset: asset.id.clone(),
                action: SceneAssetAction::Open,
            });
        }
        if response.clicked() {
            self.selected = Some(asset.id.clone());
            actions.push(BrowserAction::Select {
                id: asset.id.clone(),
            });
            self.request_preview(asset, actions);
        }
        response.dnd_set_drag_payload(asset.drag());
        response.context_menu(|ui| {
            if ui.button("Select").clicked() {
                self.selected = Some(asset.id.clone());
                actions.push(BrowserAction::Select {
                    id: asset.id.clone(),
                });
                self.request_preview(asset, actions);
                ui.close();
            }
            self.file_commands(ui, asset, true, actions);
        });
    }
    fn request_preview(&mut self, asset: &AssetTile, actions: &mut Vec<BrowserAction>) {
        if asset.kind.can_preview()
            && asset.preview.is_none()
            && actions
                .iter()
                .filter(|action| matches!(action, BrowserAction::Preview { .. }))
                .count()
                < 4
            && self.preview_requests.insert(asset.id.clone())
        {
            actions.push(BrowserAction::Preview {
                id: asset.id.clone(),
            });
        }
    }
    fn file_commands(
        &mut self,
        ui: &mut egui::Ui,
        asset: &AssetTile,
        popup: bool,
        actions: &mut Vec<BrowserAction>,
    ) {
        if asset.kind == AssetKind::Scene {
            for (label, action) in [
                ("Open scene", SceneAssetAction::Open),
                ("Duplicate scene…", SceneAssetAction::Duplicate),
                ("Rename scene…", SceneAssetAction::Rename),
                ("Move scene file…", SceneAssetAction::Move),
                ("Delete scene…", SceneAssetAction::Delete),
                ("Set as entry scene", SceneAssetAction::SetEntry),
            ] {
                let entry_action = asset.scene_entry
                    && matches!(
                        action,
                        SceneAssetAction::Delete | SceneAssetAction::SetEntry
                    );
                let enabled =
                    (!asset.protected || matches!(action, SceneAssetAction::Open)) && !entry_action;
                let response = ui
                    .add_enabled(enabled, egui::Button::new(label))
                    .on_disabled_hover_text(if entry_action {
                        "This is the entry scene. Choose another entry scene before deleting it."
                    } else {
                        "This source is protected."
                    });
                self.hit_targets
                    .insert(format!("scene-action:{action:?}"), response.rect);
                if response.clicked() {
                    actions.push(BrowserAction::Scene {
                        asset: asset.id.clone(),
                        action,
                    });
                    if popup {
                        ui.close();
                    }
                }
            }
            ui.weak("Entry and game-owned scene references are checked before changes.");
            return;
        }

        if asset.kind == AssetKind::Material
            && ui
                .add_enabled(!asset.protected, egui::Button::new("Edit material…"))
                .clicked()
        {
            actions.push(BrowserAction::OpenMaterial {
                id: asset.id.clone(),
            });
            if popup {
                ui.close();
            }
        }

        let rename = ui.add_enabled(!asset.protected, egui::Button::new("Rename…"));
        self.hit_targets.insert("rename".into(), rename.rect);
        if rename.clicked() {
            self.dialog = Some(BrowserDialog::Rename {
                id: asset.id.clone(),
                name: asset.name().into(),
            });
            if popup {
                ui.close();
            }
        }
        let moving = ui.add_enabled(!asset.protected, egui::Button::new("Move…"));
        self.hit_targets.insert("move".into(), moving.rect);
        if moving.clicked() {
            self.dialog = Some(BrowserDialog::Move {
                id: asset.id.clone(),
                folder: parent(&asset.path).into(),
            });
            if popup {
                ui.close();
            }
        }
        let delete = ui.add_enabled(!asset.protected, egui::Button::new("Move to trash…"));
        self.hit_targets.insert("delete".into(), delete.rect);
        if delete.clicked() {
            self.dialog = Some(BrowserDialog::Delete {
                id: asset.id.clone(),
                mode: DeleteMode::Undecided,
                replacement: None,
            });
            if popup {
                ui.close();
            }
        }
        if ui
            .button(format!("Usages ({})", asset.usages.len()))
            .clicked()
        {
            self.dialog = Some(BrowserDialog::Usages {
                id: asset.id.clone(),
            });
            if popup {
                ui.close();
            }
        }
        if asset.protected {
            ui.weak("This asset is protected from changes.");
        }
    }
    fn details_ui(
        &mut self,
        ui: &mut egui::Ui,
        assets: &[AssetTile],
        _folders: &BTreeSet<String>,
        actions: &mut Vec<BrowserAction>,
    ) {
        let Some(asset) = self
            .selected
            .as_ref()
            .and_then(|id| assets.iter().find(|asset| &asset.id == id))
        else {
            ui.weak("Select an asset to view its details.");
            return;
        };
        ui.strong(asset.name());
        ui.label(asset.kind.label());
        if asset.kind.can_preview() {
            if ui.is_enabled() {
                self.request_preview(asset, actions);
            }
            let width = ui.available_width().clamp(50.0, 220.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(width), Sense::hover());
            draw_preview(ui, rect, asset);
            if let Some(status) = &asset.preview_status {
                ui.weak(status);
                if asset.preview.is_none() && ui.small_button("Retry preview").clicked() {
                    self.preview_requests.remove(&asset.id);
                    self.request_preview(asset, actions);
                }
            } else if asset.preview.is_none() {
                ui.weak("Preview loading…");
            }
        }
        ui.label(&asset.path);
        ui.label(asset_size(asset));
        for (name, value) in &asset.metadata {
            ui.label(format!("{name}: {value}"));
        }
        egui::CollapsingHeader::new(if asset.kind == AssetKind::Scene {
            "Scene ID"
        } else {
            "Asset reference"
        })
        .show(ui, |ui| {
            ui.add(egui::Label::new(&asset.reference).selectable(true).wrap());
            if ui
                .small_button(if asset.kind == AssetKind::Scene {
                    "Copy scene ID"
                } else {
                    "Copy reference"
                })
                .clicked()
            {
                ui.ctx().copy_text(asset.reference.clone());
            }
        });
        ui.separator();
        self.file_commands(ui, asset, false, actions);
    }
    fn trash_ui(
        &mut self,
        ui: &mut egui::Ui,
        trash: &[TrashTile],
        actions: &mut Vec<BrowserAction>,
    ) {
        ui.weak("Restore returns the file and its ID. Current project references are preserved; Undo reverses deletion together with its references.");
        egui::ScrollArea::vertical()
            .id_salt("asset-trash")
            .show(ui, |ui| {
                if trash.is_empty() {
                    ui.label("Trash is empty.");
                }
                for entry in trash {
                    ui.push_id(&entry.id, |ui| {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.label(&entry.path);
                            ui.weak(format!("{} · {}", entry.kind.label(), entry.deleted_at));
                            let restore = ui.button("Restore");
                            self.hit_targets
                                .insert(format!("restore:{}", entry.id), restore.rect);
                            if restore.clicked() {
                                actions.push(BrowserAction::Restore {
                                    trash_id: entry.id.clone(),
                                });
                            }
                        });
                    });
                }
            });
    }
    fn dialog_ui(
        &mut self,
        ctx: &egui::Context,
        pane_id: egui::Id,
        assets: &[AssetTile],
        folders: &BTreeSet<String>,
        busy: bool,
        actions: &mut Vec<BrowserAction>,
    ) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let mut close = false;
        let mut command = None;
        let response = egui::Modal::new(pane_id.with("asset-command-dialog")).show(ctx, |ui| {
            ui.set_width(390.0_f32.min(ctx.content_rect().width() - 40.0).max(160.0));
            ui.add_enabled_ui(!busy, |ui| {
                match &mut dialog {
                    BrowserDialog::NewMaterial { name } => {
                        ui.heading("New material");
                        ui.label(writable_folder(&self.folder));
                        ui.add(egui::TextEdit::singleline(name).hint_text("Material name"));
                        let error = valid_leaf(name).err();
                        if let Some(error) = error {
                            ui.weak(error);
                        }
                        if ui.add_enabled(error.is_none(), egui::Button::new("Create")).clicked() {
                            let name = format!("{}.material", name.trim());
                            command = Some(BrowserAction::NewMaterial {
                                path: join_folder(&writable_folder(&self.folder), &name),
                            });
                        }
                    }
                    BrowserDialog::NewFolder { name } => {
                        ui.heading("New folder");
                        ui.label(format!("In folder: {}", writable_folder(&self.folder)));
                        let input = ui.add(egui::TextEdit::singleline(name).hint_text("Folder name"));
                        self.hit_targets.insert("dialog-input".into(), input.rect);
                        let error = valid_leaf(name).err();
                        if let Some(error) = &error {
                            ui.weak(*error);
                        }
                        let create = ui.add_enabled(error.is_none(), egui::Button::new("Create"));
                        self.hit_targets.insert("dialog-confirm".into(), create.rect);
                        if create.clicked() {
                            command = Some(BrowserAction::NewFolder {
                                path: join_folder(&writable_folder(&self.folder), name.trim()),
                            });
                        }
                    }
                    BrowserDialog::Rename { id, name } => {
                        ui.heading("Rename asset");
                        if let Some(asset) = assets.iter().find(|asset| &asset.id == id) {
                            ui.label(&asset.path);
                            let input = ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                            self.hit_targets.insert("dialog-input".into(), input.rect);
                            ui.weak("Keep the file extension. ID references will be preserved.");
                            ui.weak("The project will be saved first; canceling the save cancels the rename.");
                            let error = valid_leaf(name).err();
                            if let Some(error) = &error {
                                ui.weak(*error);
                            }
                            let valid = error.is_none()
                                && !asset.protected
                                && name.trim() != asset.name();
                            let confirm = ui.add_enabled(valid, egui::Button::new("Rename"));
                            self.hit_targets.insert("dialog-confirm".into(), confirm.rect);
                            if confirm.clicked() {
                                command = Some(BrowserAction::Rename {
                                    id: id.clone(),
                                    name: name.trim().into(),
                                });
                            }
                        } else {
                            ui.label("The asset is no longer available.");
                        }
                    }
                    BrowserDialog::Move { id, folder } => {
                        ui.heading("Move asset");
                        if let Some(asset) = assets.iter().find(|asset| &asset.id == id) {
                            ui.label(&asset.path);
                            egui::ComboBox::from_id_salt("asset-move-target")
                                .selected_text(folder_label(folder))
                                .width(ui.available_width())
                                .show_ui(ui, |ui| {
                                    for path in folders.iter().filter(|path| folder_is_writable(path)) {
                                        ui.selectable_value(folder, path.clone(), path);
                                    }
                                });
                            ui.weak("The ID stays the same; old path references are updated.");
                            ui.weak("The project will be saved first; canceling the save cancels the move.");
                            let confirm = ui.add_enabled(
                                !asset.protected
                                    && folder_is_writable(folder)
                                    && *folder != parent(&asset.path),
                                egui::Button::new("Move"),
                            );
                            self.hit_targets.insert("dialog-confirm".into(), confirm.rect);
                            if confirm.clicked() {
                                command = Some(BrowserAction::Move {
                                    id: id.clone(),
                                    folder: folder.clone(),
                                });
                            }
                        } else {
                            ui.label("The asset is no longer available.");
                        }
                    }
                    BrowserDialog::Delete { id, mode, replacement } => {
                        ui.heading("Move asset to trash?");
                        if let Some(asset) = assets.iter().find(|asset| &asset.id == id) {
                            ui.label(&asset.path);
                            if asset.usages.is_empty() {
                                ui.label("No known usages in the project.");
                            } else {
                                ui.label(format!("Usages: {}", asset.usages.len()));
                                usage_list(ui, &asset.usages, 145.0);
                                ui.label("Before deleting, choose how to handle references:");
                                ui.radio_value(mode, DeleteMode::Replace, "Replace with another asset");
                                ui.radio_value(mode, DeleteMode::Detach, "Explicitly unlink the asset; keep objects");
                                if *mode == DeleteMode::Replace {
                                    let candidates: Vec<_> = assets
                                        .iter()
                                        .filter(|candidate| candidate.id != asset.id && candidate.kind == asset.kind)
                                        .collect();
                                    let selected = replacement.as_ref().and_then(|id| {
                                        candidates.iter().find(|candidate| &candidate.id == id)
                                    });
                                    let label = selected.map_or("Choose a replacement", |candidate| candidate.path.as_str());
                                    egui::ComboBox::from_id_salt("asset-delete-replacement")
                                        .selected_text(label)
                                        .width(ui.available_width())
                                        .show_ui(ui, |ui| {
                                            for candidate in &candidates {
                                                ui.selectable_value(replacement, Some(candidate.id.clone()), &candidate.path);
                                            }
                                        });
                                    if candidates.is_empty() {
                                        ui.weak("The project has no other asset of this type yet.");
                                    }
                                }
                                ui.weak("Model and shader dependencies are checked before the operation; incompatible unlinking will be rejected.");
                            }
                            ui.weak("The project will be saved first. The file will remain in trash for restoration.");
                            let valid_replacement = replacement.as_ref().is_some_and(|id| {
                                assets.iter().any(|candidate| candidate.id == *id && candidate.id != asset.id && candidate.kind == asset.kind)
                            });
                            let references_resolved = asset.usages.is_empty()
                                || *mode == DeleteMode::Detach
                                || (*mode == DeleteMode::Replace && valid_replacement);
                            let confirm = ui.add_enabled(
                                !asset.protected && references_resolved,
                                egui::Button::new("Move to trash"),
                            );
                            self.hit_targets.insert("dialog-confirm".into(), confirm.rect);
                            if confirm.clicked() {
                                command = Some(BrowserAction::Delete {
                                    id: id.clone(),
                                    replacement: if *mode == DeleteMode::Replace { replacement.clone() } else { None },
                                    detach: *mode == DeleteMode::Detach,
                                });
                            }
                        } else {
                            ui.label("The asset is no longer available.");
                        }
                    }
                    BrowserDialog::Usages { id } => {
                        ui.heading("Asset usages");
                        if let Some(asset) = assets.iter().find(|asset| &asset.id == id) {
                            ui.label(&asset.path);
                            usage_list(ui, &asset.usages, 260.0);
                        } else {
                            ui.label("The asset is no longer available.");
                        }
                    }
                }
                ui.separator();
                let cancel = ui.button(if matches!(dialog, BrowserDialog::Usages { .. }) {
                    "Close"
                } else {
                    "Cancel"
                });
                self.hit_targets.insert("dialog-cancel".into(), cancel.rect);
                if cancel.clicked() {
                    close = true;
                }
            });
        });
        if response.should_close() {
            close = true;
            command = None;
        }
        if let Some(command) = command {
            actions.push(command);
        } else if !close {
            self.dialog = Some(dialog);
        }
    }
}

fn usage_list(ui: &mut egui::Ui, usages: &[String], height: f32) {
    egui::ScrollArea::vertical()
        .id_salt("asset-usage-list")
        .max_height(height)
        .show(ui, |ui| {
            if usages.is_empty() {
                ui.weak("No known usages.");
            }
            for usage in usages {
                ui.label(format!("• {usage}"));
            }
        });
}
fn draw_preview(ui: &egui::Ui, rect: Rect, asset: &AssetTile) {
    ui.painter()
        .rect_filled(rect, 3.0, Color32::from_rgb(29, 34, 40));
    if let Some(texture) = asset.preview {
        let size = ui
            .ctx()
            .tex_manager()
            .read()
            .meta(texture)
            .map(|meta| Vec2::new(meta.size[0] as f32, meta.size[1] as f32));
        let fitted = size.map_or(rect.shrink(3.0), |size| {
            let inner = rect.shrink(3.0);
            let scale = (inner.width() / size.x.max(1.0)).min(inner.height() / size.y.max(1.0));
            Rect::from_center_size(inner.center(), size * scale)
        });
        ui.painter().image(
            texture,
            fitted,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            asset.kind.icon(),
            FontId::proportional((rect.height() * 0.23).clamp(14.0, 32.0)),
            Color32::from_rgb(161, 184, 205),
        );
    }
}
fn all_folders(assets: &[AssetTile], folders: &[String]) -> BTreeSet<String> {
    let mut result = BTreeSet::from([String::new(), "assets".into()]);
    for folder in folders
        .iter()
        .map(String::as_str)
        .chain(assets.iter().map(|asset| parent(&asset.path)))
    {
        let mut current = folder;
        while !current.is_empty() {
            result.insert(current.into());
            current = parent(current);
        }
    }
    result
}
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}
fn folder_label(folder: &str) -> &str {
    if folder.is_empty() { "Project" } else { folder }
}
fn accepts_drop(folder: &str, kind: AssetKind) -> bool {
    if kind == AssetKind::Scene {
        folder == "scenes" || folder.starts_with("scenes/")
    } else {
        folder_is_writable(folder)
    }
}
fn folder_is_writable(folder: &str) -> bool {
    folder == "assets" || folder.starts_with("assets/")
}
fn writable_folder(folder: &str) -> String {
    if folder_is_writable(folder) {
        folder.into()
    } else {
        "assets".into()
    }
}
fn join_folder(folder: &str, name: &str) -> String {
    format!("{folder}/{name}")
}
fn valid_leaf(name: &str) -> Result<(), &'static str> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a name.");
    }
    if name == "." || name == ".." || name.ends_with('.') {
        return Err("This name cannot be used.");
    }
    if name
        .chars()
        .any(|character| character.is_control() || "/\\:*?\"<>|".contains(character))
    {
        return Err("The name contains invalid characters.");
    }
    Ok(())
}
fn abbreviated(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.into()
    } else {
        format!(
            "{}…",
            text.chars()
                .take(limit.saturating_sub(1))
                .collect::<String>()
        )
    }
}
fn asset_size(asset: &AssetTile) -> String {
    if asset.kind == AssetKind::Scene && asset.size == 0 {
        "Not saved".into()
    } else {
        format_size(asset.size)
    }
}
fn format_size(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    } else if size >= 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{size} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(id: &str, path: &str, kind: AssetKind) -> AssetTile {
        AssetTile {
            id: id.into(),
            reference: format!("asset://{id}"),
            path: path.into(),
            kind,
            size: 256,
            protected: false,
            scene_entry: false,
            preview: None,
            preview_status: None,
            usages: vec![],
            metadata: vec![],
        }
    }
    #[test]
    fn scene_entry_disables_delete_and_redundant_set_entry_but_keeps_rename() {
        for entry in [false, true] {
            let ctx = egui::Context::default();
            let mut browser = BrowserState::default();
            let mut asset = tile("scene-source", "scenes/arena.scene.json", AssetKind::Scene);
            asset.scene_entry = entry;
            let mut draw = |events| {
                let mut actions = vec![];
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(480., 480.))),
                        events,
                        ..Default::default()
                    },
                    |ui| browser.file_commands(ui, &asset, false, &mut actions),
                )
                .drop_without_applying_deltas();
                (actions, browser.hit_targets.clone())
            };
            for action in [
                SceneAssetAction::Delete,
                SceneAssetAction::SetEntry,
                SceneAssetAction::Rename,
            ] {
                let (_, targets) = draw(vec![]);
                let pos = targets[&format!("scene-action:{action:?}")].center();
                let (actions, _) = draw(click(pos));
                let expected = !(entry
                    && matches!(
                        action,
                        SceneAssetAction::Delete | SceneAssetAction::SetEntry
                    ));
                assert_eq!(
                    actions.contains(&BrowserAction::Scene {
                        asset: asset.id.clone(),
                        action
                    }),
                    expected
                );
            }
        }
    }
    fn frame(
        ctx: &egui::Context,
        state: &mut BrowserState,
        assets: &[AssetTile],
        events: Vec<egui::Event>,
        busy: bool,
    ) -> Vec<BrowserAction> {
        let mut actions = vec![];
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 720.0))),
            events,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            actions = state.ui(ui, assets, &[], &[], busy);
        })
        .drop_without_applying_deltas();
        actions
    }
    fn click(position: Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }
    #[test]
    fn search_is_recursive_but_plain_navigation_lists_direct_children() {
        let mut state = BrowserState::default();
        let direct = tile("a", "assets/Crate.glb", AssetKind::Model);
        let nested = tile("b", "assets/props/Crate.glb", AssetKind::Model);
        let adjacent = tile("c", "assets-other/Crate.glb", AssetKind::Model);
        assert!(state.matches(&direct));
        assert!(!state.matches(&nested));
        state.search = "crATe".into();
        assert!(state.matches(&nested));
        assert!(!state.matches(&adjacent));
        state.kind_filter = Some(AssetKind::Audio);
        assert!(!state.matches(&direct));
        state.folder.clear();
        state.kind_filter = None;
        assert!(state.matches(&adjacent));
    }
    #[test]
    fn selected_preview_is_requested_once_and_retries_after_invalidation() {
        let ctx = egui::Context::default();
        let assets = [tile("a", "assets/Crate.glb", AssetKind::Model)];
        let mut state = BrowserState {
            selected: Some("a".into()),
            ..Default::default()
        };
        assert_eq!(
            frame(&ctx, &mut state, &assets, vec![], false),
            vec![BrowserAction::Preview { id: "a".into() }]
        );
        assert!(frame(&ctx, &mut state, &assets, vec![], false).is_empty());
        state.invalidate_preview("a");
        assert_eq!(
            frame(&ctx, &mut state, &assets, vec![], false),
            vec![BrowserAction::Preview { id: "a".into() }]
        );
    }
    #[test]
    fn pointer_selection_emits_typed_action_without_touching_scene() {
        let ctx = egui::Context::default();
        let assets = [tile("a", "assets/Crate.glb", AssetKind::Model)];
        let mut state = BrowserState::default();
        assert!(
            frame(&ctx, &mut state, &assets, vec![], false)
                .contains(&BrowserAction::Preview { id: "a".into() })
        );
        let center = state.hit_targets["asset:a"].center();
        let actions = frame(&ctx, &mut state, &assets, click(center), false);
        assert_eq!(state.selected.as_deref(), Some("a"));
        assert!(actions.contains(&BrowserAction::Select { id: "a".into() }));
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, BrowserAction::Preview { .. }))
        );
        assert!(!actions.iter().any(|action| matches!(
            action,
            BrowserAction::Rename { .. }
                | BrowserAction::Move { .. }
                | BrowserAction::Delete { .. }
        )));
    }
    #[test]
    fn used_deletion_requires_explicit_resolution_and_escape_cancels() {
        let ctx = egui::Context::default();
        let mut asset = tile("a", "assets/Crate.glb", AssetKind::Model);
        asset.usages = vec!["Scene / Crate".into()];
        let assets = [asset];
        let mut state = BrowserState {
            dialog: Some(BrowserDialog::Delete {
                id: "a".into(),
                mode: DeleteMode::Undecided,
                replacement: None,
            }),
            ..Default::default()
        };
        frame(&ctx, &mut state, &assets, vec![], false);
        frame(&ctx, &mut state, &assets, vec![], false);
        let center = state.hit_targets["dialog-confirm"].center();
        assert!(frame(&ctx, &mut state, &assets, click(center), false).is_empty());
        assert!(state.has_dialog());
        let escape = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        assert!(frame(&ctx, &mut state, &assets, vec![escape], false).is_empty());
        assert!(!state.has_dialog());
        state.dialog = Some(BrowserDialog::Delete {
            id: "a".into(),
            mode: DeleteMode::Detach,
            replacement: None,
        });
        frame(&ctx, &mut state, &assets, vec![], false);
        frame(&ctx, &mut state, &assets, vec![], false);
        let center = state.hit_targets["dialog-confirm"].center();
        assert_eq!(
            frame(&ctx, &mut state, &assets, click(center), false),
            vec![BrowserAction::Delete {
                id: "a".into(),
                replacement: None,
                detach: true
            }]
        );
        assert!(!state.has_dialog());
    }
    #[test]
    fn busy_blocks_import_and_file_commands() {
        let ctx = egui::Context::default();
        let assets = [tile("a", "assets/Crate.glb", AssetKind::Model)];
        let mut state = BrowserState::default();
        frame(&ctx, &mut state, &assets, vec![], true);
        let center = state.hit_targets["import"].center();
        assert!(frame(&ctx, &mut state, &assets, click(center), true).is_empty());
    }
    #[test]
    fn asset_drag_payload_is_typed_and_escape_does_not_emit_file_mutations() {
        let ctx = egui::Context::default();
        let assets = [tile("a", "assets/Crate.glb", AssetKind::Model)];
        let mut state = BrowserState::default();
        frame(&ctx, &mut state, &assets, vec![], false);
        let center = state.hit_targets["asset:a"].center();
        frame(
            &ctx,
            &mut state,
            &assets,
            vec![
                egui::Event::PointerMoved(center),
                egui::Event::PointerButton {
                    pos: center,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            false,
        );
        let actions = frame(
            &ctx,
            &mut state,
            &assets,
            vec![egui::Event::PointerMoved(center + Vec2::new(120.0, 12.0))],
            false,
        );
        let payload = egui::DragAndDrop::payload::<AssetDrag>(&ctx).unwrap();
        assert_eq!(payload.id, "a");
        assert_eq!(payload.reference, "asset://a");
        assert_eq!(payload.kind, AssetKind::Model);
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, BrowserAction::Move { .. }))
        );
        let actions = frame(
            &ctx,
            &mut state,
            &assets,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            false,
        );
        assert!(egui::DragAndDrop::payload::<AssetDrag>(&ctx).is_none());
        assert!(!actions.iter().any(|action| matches!(
            action,
            BrowserAction::Rename { .. }
                | BrowserAction::Move { .. }
                | BrowserAction::Delete { .. }
        )));
    }
    #[test]
    fn compact_browser_keeps_a_visible_asset_and_selected_details() {
        let ctx = egui::Context::default();
        let assets = [tile("a", "assets/Crate.glb", AssetKind::Model)];
        let mut state = BrowserState {
            selected: Some("a".into()),
            ..Default::default()
        };
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 540.0));
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| {
                let _ = state.ui(ui, &assets, &[], &[], false);
            },
        );
        let center = state.hit_targets["asset:a"].center();
        assert!(screen.contains(center));
        let details = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "Details" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("Compact browser exposes Details");
        output.drop_without_applying_deltas();
        for pressed in [true, false] {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![
                        egui::Event::PointerMoved(details),
                        egui::Event::PointerButton {
                            pos: details,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    state.ui(ui, &assets, &[], &[], false);
                },
            )
            .drop_without_applying_deltas();
        }
        assert!(state.show_details);
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| {
                state.ui(ui, &assets, &[], &[], false);
            },
        );
        let visible_name_count = output
            .shapes
            .iter()
            .filter(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    text.galley.text() == "Crate.glb"
                        && shape
                            .clip_rect
                            .contains(text.pos + text.galley.size() * 0.5)
                } else {
                    false
                }
            })
            .count();
        output.drop_without_applying_deltas();
        assert_eq!(
            visible_name_count, 2,
            "tile and selected details should remain visible after Details"
        );
    }
    #[test]
    fn short_wide_grid_keeps_full_tiles_visible_beside_three_compact_folder_buttons() {
        let ctx = egui::Context::default();
        let assets = [
            tile("model", "assets/Crate.glb", AssetKind::Model),
            tile("image", "assets/Albedo.png", AssetKind::Texture),
        ];
        let folders = [
            "assets/models".into(),
            "assets/textures".into(),
            "assets/audio".into(),
        ];
        let mut state = BrowserState::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 300.0));
        let input = |events| egui::RawInput {
            screen_rect: Some(screen),
            events,
            ..Default::default()
        };
        let output = ctx.run_ui(input(vec![]), |ui| {
            let _ = state.ui(ui, &assets, &folders, &[], false);
        });
        let model_rect = state.hit_targets["asset:model"];
        let image_rect = state.hit_targets["asset:image"];
        for (name, rect) in [("Crate.glb", model_rect), ("Albedo.png", image_rect)] {
            assert!(screen.contains_rect(rect));
            assert!(
                output.shapes.iter().any(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        text.galley.text() == name
                            && shape.clip_rect.contains_rect(rect.shrink(1.0))
                            && shape
                                .clip_rect
                                .contains_rect(Rect::from_min_size(text.pos, text.galley.size()))
                    } else {
                        false
                    }
                }),
                "full thumbnail, filename and size must fit without scrolling: {name}"
            );
        }
        let audio = state.hit_targets["folder-content:assets/audio"];
        let models = state.hit_targets["folder-content:assets/models"];
        let textures = state.hit_targets["folder-content:assets/textures"];
        assert_eq!(audio.top(), models.top());
        assert_eq!(models.top(), textures.top());
        let folder_center = models.center();
        output.drop_without_applying_deltas();
        ctx.run_ui(input(click(folder_center)), |ui| {
            let _ = state.ui(ui, &assets, &folders, &[], false);
        })
        .drop_without_applying_deltas();
        assert_eq!(state.folder, "assets/models");
    }
    #[test]
    fn compact_folder_buttons_accept_typed_file_drop_and_respect_busy_state() {
        let assets = [tile("model", "assets/Crate.glb", AssetKind::Model)];
        let folders = ["assets/models".into()];
        for busy_on_drop in [false, true] {
            let ctx = egui::Context::default();
            let mut state = BrowserState::default();
            let mut draw = |events, busy| {
                let mut actions = vec![];
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1000.0, 300.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| actions = state.ui(ui, &assets, &folders, &[], busy),
                )
                .drop_without_applying_deltas();
                (actions, state.hit_targets.clone())
            };
            let (_, targets) = draw(vec![], false);
            let source = targets["asset:model"].center();
            let destination = targets["folder-content:assets/models"].center();
            draw(
                vec![
                    egui::Event::PointerMoved(source),
                    egui::Event::PointerButton {
                        pos: source,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                false,
            );
            draw(vec![egui::Event::PointerMoved(destination)], false);
            assert_eq!(
                egui::DragAndDrop::payload::<AssetDrag>(&ctx).unwrap().id,
                "model"
            );
            let (actions, _) = draw(
                vec![egui::Event::PointerButton {
                    pos: destination,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                busy_on_drop,
            );
            let expected = BrowserAction::Move {
                id: "model".into(),
                folder: "assets/models".into(),
            };
            assert_eq!(actions.contains(&expected), !busy_on_drop);
        }
    }
    #[test]
    fn folder_hierarchy_keeps_empty_directories_and_rejects_leaf_traversal() {
        let assets = [tile("a", "assets/props/a.glb", AssetKind::Model)];
        let folders = all_folders(&assets, &["assets/empty/deep".into()]);
        assert!(folders.contains("assets/props"));
        assert!(folders.contains("assets/empty"));
        assert!(folders.contains("assets/empty/deep"));
        assert!(valid_leaf("../outside").is_err());
        assert!(valid_leaf("..").is_err());
        assert!(valid_leaf("folder:name").is_err());
        assert!(valid_leaf("Арена 2").is_ok());
    }
}
