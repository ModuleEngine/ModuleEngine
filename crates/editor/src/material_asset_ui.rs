use crate::assets_ui::{AssetKind, AssetTile, BrowserAction};
use eframe::egui;
use module_engine_core::{GraphDocument, MaterialAsset, same_component_json};
use module_engine_project::{AssetManifest, hash_bytes, resolve_relative};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub(crate) struct MaterialDraft {
    root: PathBuf,
    id: String,
    path: String,
    hash: String,
    original: Value,
    value: Value,
    pending: Option<String>,
}

impl MaterialDraft {
    pub(crate) fn open(root: &Path, document: &GraphDocument, id: String) -> Result<Self, String> {
        let manifest = AssetManifest::from_value(&document.asset_manifest)?;
        let entry = manifest.assets.get(&id).ok_or("Unknown material asset")?;
        if module_engine_project::AssetKind::for_path(&entry.path)
            != module_engine_project::AssetKind::Material
        {
            return Err("Choose a .material asset".into());
        }
        let mut bytes = Vec::new();
        let source = resolve_relative(root, &entry.path)?;
        File::open(source)
            .and_then(|file| file.take(65_537).read_to_end(&mut bytes))
            .map_err(|e| e.to_string())?;
        if hash_bytes(&bytes) != entry.sha256 {
            return Err("Material changed on disk. Refresh Assets before editing.".into());
        }
        MaterialAsset::parse(&bytes)?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        Ok(Self {
            root: root.into(),
            id,
            path: entry.path.clone(),
            hash: entry.sha256.clone(),
            original: value.clone(),
            value,
            pending: None,
        })
    }

    pub(crate) fn ui(
        &mut self,
        ctx: &egui::Context,
        root: &Path,
        document: &GraphDocument,
        tiles: &[AssetTile],
        busy: bool,
    ) -> (bool, Option<BrowserAction>) {
        if self.root != root {
            return (false, None);
        }
        let current_hash = document.asset_manifest["assets"][&self.id]["sha256"].as_str();
        if !busy && self.pending.is_some() {
            if current_hash == self.pending.as_deref() {
                return (false, None);
            }
            self.pending = None;
        }
        let stale = current_hash != Some(&self.hash);
        let mut close = false;
        let mut action = None;
        let response = egui::Modal::new(egui::Id::new("material-asset-editor").with(&self.id))
            .show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 40.).clamp(180., 460.));
                ui.heading("Edit material");
                ui.label(&self.path);
                if stale {
                    ui.colored_label(
                        egui::Color32::LIGHT_RED,
                        "Material changed. Open its current version to edit it.",
                    );
                }
                ui.add_enabled_ui(!busy && !stale, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height((ctx.content_rect().height() - 145.).max(80.))
                        .show(ui, |ui| edit_fields(ui, &mut self.value, tiles));
                    let dirty = !same_component_json(Some(&self.original), Some(&self.value));
                    if ui.add_enabled(dirty, egui::Button::new("Apply")).clicked() {
                        let mut bytes = serde_json::to_vec_pretty(&self.value).expect("JSON value");
                        bytes.push(b'\n');
                        self.pending = Some(hash_bytes(&bytes));
                        action = Some(BrowserAction::SaveMaterial {
                            id: self.id.clone(),
                            expected_hash: self.hash.clone(),
                            bytes,
                        });
                    }
                });
                if busy {
                    ui.spinner();
                }
                if ui.add_enabled(!busy, egui::Button::new("Cancel")).clicked() {
                    close = true;
                }
            });
        (
            !(close || !busy && action.is_none() && response.should_close()),
            action,
        )
    }
}

fn edit_fields(ui: &mut egui::Ui, value: &mut Value, tiles: &[AssetTile]) {
    ui.weak("Inherited fields use the model’s material.");
    for (field, label, default) in [
        ("base_color", "Base color", &([1.; 4][..])),
        ("emissive", "Emissive", &([0.; 3][..])),
    ] {
        ui.push_id(field, |ui| {
            if override_field(ui, value, field, label, json!(default)) {
                ui.horizontal_wrapped(|ui| {
                    let channels = value[field].as_array_mut().expect("validated color");
                    for (name, channel) in ["R", "G", "B", "A"].into_iter().zip(channels) {
                        let mut linear = channel.as_f64().expect("validated color channel");
                        if ui
                            .add(
                                egui::DragValue::new(&mut linear)
                                    .range(0. ..=1.)
                                    .speed(0.01)
                                    .prefix(format!("{name} ")),
                            )
                            .changed()
                        {
                            *channel = json!(linear);
                        }
                    }
                });
            }
        });
    }
    for (field, label, default, range) in [
        ("metallic", "Metallic", 1., 0. ..=1.),
        ("roughness", "Roughness", 1., 0. ..=1.),
        (
            "normal_scale",
            "Normal scale",
            1.,
            f32::MIN as f64..=f32::MAX as f64,
        ),
        ("occlusion_strength", "Occlusion strength", 1., 0. ..=1.),
        ("alpha_cutoff", "Alpha cutoff", 0.5, 0. ..=f32::MAX as f64),
    ] {
        ui.push_id(field, |ui| {
            ui.horizontal_wrapped(|ui| {
                if override_field(ui, value, field, label, json!(default)) {
                    let mut factor = value[field].as_f64().expect("validated factor");
                    if ui
                        .add(egui::DragValue::new(&mut factor).range(range).speed(0.01))
                        .changed()
                    {
                        value[field] = json!(factor);
                    }
                }
            });
        });
    }
    choice(
        ui,
        value,
        "alpha_mode",
        "Alpha mode",
        &[
            ("Opaque", json!("opaque")),
            ("Mask", json!("mask")),
            ("Blend", json!("blend")),
        ],
    );
    choice(
        ui,
        value,
        "double_sided",
        "Sidedness",
        &[
            ("Single-sided", json!(false)),
            ("Double-sided", json!(true)),
        ],
    );
    for (field, label) in [
        ("base_color_texture", "Base color map"),
        ("metallic_roughness_texture", "Metallic / roughness map"),
        ("normal_texture", "Normal map"),
        ("occlusion_texture", "Occlusion map"),
        ("emissive_texture", "Emissive map"),
    ] {
        ui.label(label);
        let selected = value[field].as_str().unwrap_or("Inherited").to_owned();
        egui::ComboBox::from_id_salt(field)
            .selected_text(&selected)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                let inherit = value[field].is_null();
                if ui.selectable_label(inherit, "Inherited").clicked() && !inherit {
                    value.as_object_mut().unwrap().remove(field);
                }
                for tile in tiles.iter().filter(|tile| tile.kind == AssetKind::Texture) {
                    if ui
                        .selectable_label(selected == tile.path, &tile.path)
                        .clicked()
                    {
                        value[field] = json!(tile.path);
                    }
                }
            });
    }
}

fn override_field(
    ui: &mut egui::Ui,
    value: &mut Value,
    field: &str,
    label: &str,
    default: Value,
) -> bool {
    let mut enabled = !value[field].is_null();
    if ui
        .checkbox(&mut enabled, label)
        .on_hover_text("Override the model's material value")
        .changed()
    {
        if enabled {
            value[field] = default;
        } else {
            value.as_object_mut().unwrap().remove(field);
        }
    }
    enabled
}

fn choice(
    ui: &mut egui::Ui,
    value: &mut Value,
    field: &str,
    label: &str,
    choices: &[(&str, Value)],
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        let selected = choices
            .iter()
            .find(|(_, option)| option == &value[field])
            .map_or("Inherited", |(label, _)| *label);
        egui::ComboBox::from_id_salt(field)
            .selected_text(selected)
            .show_ui(ui, |ui| {
                let inherit = value[field].is_null();
                if ui.selectable_label(inherit, "Inherited").clicked() && !inherit {
                    value.as_object_mut().unwrap().remove(field);
                }
                for (label, option) in choices {
                    if ui
                        .selectable_label(option == &value[field], *label)
                        .clicked()
                    {
                        value[field] = option.clone();
                    }
                }
            });
    });
}
