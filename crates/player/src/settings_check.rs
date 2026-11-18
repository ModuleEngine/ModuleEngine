//! Bounded package diagnostic through the actual Custom menu and settings API.
//! Evidence is outside the sealed package; normal play does not run this code.
use eframe::egui;
use module_engine_core::{GameInput, ScriptSettingsSnapshot, ScriptUiItem, ScriptUiSnapshot};
use module_engine_project::hash_file;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

fn destination(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
        return Err(
            "Settings diagnostics need absolute evidence/profile paths without '..'".into(),
        );
    }
    let mut ancestor = path;
    let mut tail = vec![];
    while !ancestor.exists() {
        tail.push(
            ancestor
                .file_name()
                .ok_or("Invalid diagnostic destination")?
                .to_owned(),
        );
        ancestor = ancestor.parent().ok_or("Invalid diagnostic parent")?;
    }
    let mut resolved = ancestor.canonicalize().map_err(|e| e.to_string())?;
    for part in tail.into_iter().rev() {
        resolved.push(part);
    }
    Ok(resolved)
}

fn sealed_hashes(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let manifest = crate::package::read_manifest(root)?;
    let mut hashes = manifest
        .files
        .keys()
        .map(|name| Ok((name.clone(), hash_file(&root.join(name))?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    hashes.insert("player.json".into(), hash_file(&root.join("player.json"))?);
    Ok(hashes)
}

fn menu_frame(
    ctx: &egui::Context,
    menus: &ScriptUiSnapshot,
    settings: &ScriptSettingsSnapshot,
    displayed: &ScriptSettingsSnapshot,
    events: Vec<egui::Event>,
) -> (egui::FullOutput, GameInput) {
    let mut input = GameInput::default();
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960., 540.),
            )),
            focused: true,
            events,
            ..Default::default()
        },
        |ui| {
            let rect = ui.max_rect();
            input =
                crate::script_ui::draw_script_ui(ui, rect, menus, settings, displayed, None).input;
        },
    );
    (output, input)
}
fn text(output: &egui::FullOutput) -> Vec<String> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
            _ => None,
        })
        .collect()
}
fn center(output: &egui::FullOutput, label: &str, last: bool) -> Result<egui::Pos2, String> {
    let candidates = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
                shape.clip_rect.contains_rect(rect).then_some(rect.center())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    (if last {
        candidates.last()
    } else {
        candidates.first()
    })
    .copied()
    .ok_or_else(|| format!("Menu text is not fully visible: {label}"))
}
fn pointer(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        },
    ]
}

/// Select through the same egui control used in Player, then return its action.
pub fn choose_in_menu(
    ctx: &egui::Context,
    menus: &ScriptUiSnapshot,
    settings: &ScriptSettingsSnapshot,
    displayed: &ScriptSettingsSnapshot,
    setting: &str,
    option: &str,
) -> Result<GameInput, String> {
    let snapshot = settings.get(setting).ok_or("Setting is unavailable")?;
    let chosen = snapshot
        .options
        .iter()
        .find(|item| item.id == option)
        .ok_or("Option is unavailable")?;
    let selected = snapshot
        .selected
        .as_ref()
        .and_then(|id| snapshot.options.iter().find(|item| &item.id == id))
        .map_or("Runtime controlled", |item| item.label.as_str());
    let bindings = menus
        .iter()
        .flat_map(|(instance, panels)| {
            panels.iter().flat_map(move |panel| {
                panel.items.iter().filter_map(move |item| match item {
                    ScriptUiItem::Choice {
                        id, setting: bound, ..
                    } if bound == setting => Some((instance, id)),
                    _ => None,
                })
            })
        })
        .collect::<Vec<_>>();
    if bindings.len() != 1 {
        return Err("Diagnostic needs one menu binding for the selected setting".into());
    }
    // Settle egui's first-pass window geometry without advancing the game.
    for _ in 0..2 {
        menu_frame(ctx, menus, settings, displayed, vec![])
            .0
            .drop_without_applying_deltas();
    }
    let (output, _) = menu_frame(ctx, menus, settings, displayed, vec![]);
    let combo = center(&output, selected, false)?;
    output.drop_without_applying_deltas();
    for pressed in [true, false] {
        menu_frame(ctx, menus, settings, displayed, pointer(combo, pressed))
            .0
            .drop_without_applying_deltas();
    }
    let (output, _) = menu_frame(ctx, menus, settings, displayed, vec![]);
    let button = center(&output, &chosen.label, true)?;
    output.drop_without_applying_deltas();
    let mut input = GameInput::default();
    for pressed in [true, false] {
        let (output, action) =
            menu_frame(ctx, menus, settings, displayed, pointer(button, pressed));
        output.drop_without_applying_deltas();
        input.actions.extend(action.actions);
        input.action_values.extend(action.action_values);
    }
    let key = module_engine_core::script_ui_action_key(bindings[0].0, bindings[0].1);
    if !input.actions.contains(&key) || input.action_values.get(&key) != Some(&json!(option)) {
        return Err("The actual menu control did not emit the requested scoped action".into());
    }
    Ok(input)
}

#[cfg(windows)]
fn lock_save(root: &Path, namespace: &str) -> Result<fs::File, String> {
    use std::os::windows::fs::OpenOptionsExt;
    let mut pending = vec![root.to_path_buf()];
    let mut found = vec![];
    while let Some(folder) = pending.pop() {
        for entry in fs::read_dir(folder).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink() {
                return Err("Save-failure fixture cannot follow symlinks".into());
            }
            if kind.is_dir() {
                pending.push(entry.path());
            } else if entry.path().extension().is_some_and(|ext| ext == "json")
                && let Ok(value) = serde_json::from_slice::<Value>(
                    &fs::read(entry.path()).map_err(|e| e.to_string())?,
                )
                && value["namespace"] == namespace
                && value["data_key"] == module_engine_core::PLAYER_SETTINGS_DATA_KEY
            {
                found.push(entry.path());
            }
        }
        if pending.len() + found.len() > 256 {
            return Err("Save-failure profile fixture is too large".into());
        }
    }
    if found.len() != 1 {
        return Err("Save-failure check needs one existing valid preference save".into());
    }
    // Allow reads but deny replacement after the normal host loaded the save.
    fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&found[0])
        .map_err(|e| e.to_string())
}
#[cfg(not(windows))]
fn lock_save(_: &Path, _: &str) -> Result<fs::File, String> {
    Err("The native save-sharing diagnostic is Windows-only".into())
}

pub fn run(
    root: &Path,
    output: &Path,
    state_root: Option<&Path>,
    choice: Option<(&str, &str)>,
    deny_save: bool,
) -> Result<Value, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let output = destination(output)?;
    if output.starts_with(&root) || root.starts_with(&output) {
        return Err("Evidence must be outside the sealed package".into());
    }
    if output.exists()
        && (!output.is_dir()
            || fs::read_dir(&output)
                .map_err(|e| e.to_string())?
                .next()
                .is_some())
    {
        return Err("Settings diagnostics need a new or empty evidence directory".into());
    }
    let profile = state_root.map(destination).transpose()?;
    if profile
        .as_ref()
        .is_some_and(|path| path.starts_with(&root) || root.starts_with(path))
    {
        return Err("Player preferences must be outside the sealed package".into());
    }
    let before_files = sealed_hashes(&root)?;
    let mut host =
        crate::package::build_package_host_with_state_root(&root, false, true, profile.as_deref())?;
    for _ in 0..3 {
        host.step(1. / 60., &GameInput::default())?;
    }
    let before = host.player_settings_snapshot()?;
    let menus = host.script_ui_snapshot()?;
    let ctx = egui::Context::default();
    crate::configure_style(&ctx);
    ctx.all_styles_mut(|style| style.animation_time = 0.);
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    crate::write_png(&output.join("before.png"), &host.render(960, 540)?)?;
    // A new egui Window has an invisible sizing pass. Inspect settled content
    // even in read-only restart checks, which never call choose_in_menu.
    for _ in 0..2 {
        menu_frame(&ctx, &menus, &before, &before, vec![])
            .0
            .drop_without_applying_deltas();
    }
    let (view, _) = menu_frame(&ctx, &menus, &before, &before, vec![]);
    let labels_before = text(&view);
    view.drop_without_applying_deltas();
    let _save_lock = if deny_save {
        let profile = profile
            .as_deref()
            .ok_or("Save-failure diagnostic requires an explicit profile")?;
        let namespace = host
            .accepted_document()
            .game
            .as_ref()
            .and_then(|g| g.player_preferences.as_ref())
            .ok_or("Game does not declare preferences")?
            .namespace
            .clone();
        Some(lock_save(profile, &namespace)?)
    } else {
        None
    };
    let input = if let Some((setting, option)) = choice {
        choose_in_menu(&ctx, &menus, &before, &before, setting, option)?
    } else {
        GameInput::default()
    };
    if choice.is_some() {
        host.step(1. / 60., &input)?;
    }
    let current = host.player_settings_snapshot()?;
    let current_menus = host.script_ui_snapshot()?;
    let (view, _) = menu_frame(&ctx, &current_menus, &current, &before, vec![]);
    let transition_labels = text(&view);
    view.drop_without_applying_deltas();
    crate::write_png(&output.join("after.png"), &host.render(960, 540)?)?;
    let (view, _) = menu_frame(&ctx, &current_menus, &current, &current, vec![]);
    let labels_after = text(&view);
    view.drop_without_applying_deltas();
    if sealed_hashes(&root)? != before_files {
        return Err("Settings diagnostics changed the sealed package".into());
    }
    let report = json!({"kind":"standalone-player-custom-menu","process_id":std::process::id(),
        "executable":std::env::current_exe().map_err(|e|e.to_string())?,"sdk":module_engine_core::SdkIdentity::current(),
        "state_root":profile,"package_root":root,"source_compilation":false,"native_features":crate::compiled_features(),
        "before":before,"after":current,"menu_before":menus,"menu_after":current_menus,"input":input,
        "labels_before":labels_before,"labels_after":labels_after,"transition_labels":transition_labels,
        "preference_service":host.player_preferences_diagnostics(),"package_files_unchanged":true,"package_hashes":before_files,
        "denied_save_test":deny_save,"renderer":host.device,"render_size":[960,540],
        "scope":"Actual sealed Custom worker and egui choice pointer events, then real native GameHost pixels. No desktop window or human observation is implied."});
    fs::write(
        output.join("settings-report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(report)
}
