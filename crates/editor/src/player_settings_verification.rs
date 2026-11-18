//! Preparation only: an editable, separately saved author-menu binding sample.
use module_engine_core::{
    ExecutionGraph, PlayerPreferencesConfig, SystemNode, SystemResource, SystemStage,
};
use module_engine_project::{AssetManifest, externalize_scenes, hash_file};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

pub const NAMESPACE: &str = "org.moduleengine.samples.contrasting-edges";
pub const SETTING_ID: &str = "antialiasing";
pub const SOURCE_ID: &str = "settings-menu";

/// The embedded Custom source is authoritative; menu.rs is a readable copy.
pub const MENU_SOURCE: &str = r#"// module-engine-state: {"schema":"settings-menu.v1","initial":{"message":""}}
use module_engine_core::{ScriptUiItem, ScriptUiPanel};

fn update(ctx: &mut ScriptContext) {
    // The game owns this menu. Only this stable configuration ID binds it.
    let Some(setting) = ctx.setting("antialiasing").cloned() else {
        return; // A removed capability has no control or empty menu.
    };
    // IDs and labels come from the configuration, never a duplicate option list.
    let allowed = setting.options.iter()
        .map(|option| (option.id.clone(), option.label.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    if let Some(action) = ctx.ui_action("edge-treatment").cloned() {
        let result = match action.as_str() {
            Some(option) if allowed.contains_key(option) => {
                // Read the effective choice. Re-selecting an unsaved choice retries saving.
                if setting.selected.as_deref() == Some(option)
                    && setting.source == "local"
                    && setting.persistence.saved
                    && setting.persistence.error.is_none()
                    && setting.rejection.is_none() {
                    Ok(())
                } else {
                    ctx.request_setting("antialiasing", option)
                }
            }
            _ => Err(format!("Choose a listed value: {}", allowed.values()
                .cloned().collect::<Vec<_>>().join(", "))),
        };
        ctx.state["message"] = serde_json::json!(result.err().unwrap_or_default());
    }
    let mut items = vec![ScriptUiItem::Choice {
        id: "edge-treatment".into(),
        label: "Edge smoothing".into(),
        setting: "antialiasing".into(),
    }];
    // The Host draws the current accepted selection/status next to the choice.
    // Do not copy a possibly stale "current value" into a second text label.
    if let Some(message) = ctx.state["message"].as_str().filter(|text| !text.is_empty()) {
        items.push(ScriptUiItem::Text { text: message.into() });
    }
    ctx.script_ui.push(ScriptUiPanel {
        id: "display".into(),
        title: "Display".into(),
        position: [16., 16.],
        width: 320.,
        items,
    });
}
"#;

/// No compilation, game execution, rendering or export is performed here.
pub fn create(output: &Path) -> Result<Value, String> {
    if output.exists()
        && (!output.is_dir()
            || fs::read_dir(output)
                .map_err(|error| error.to_string())?
                .next()
                .is_some())
    {
        return Err("Player settings sample needs a new or empty output directory".into());
    }
    fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let output = output.canonicalize().map_err(|error| error.to_string())?;
    let backend = module_engine_vulkan::resource_root();
    let mut document = crate::render_settings_verification::sample(&backend)?;
    document.asset_manifest =
        serde_json::to_value(AssetManifest::default()).map_err(|error| error.to_string())?;
    let game = document
        .game
        .as_mut()
        .ok_or("Contrasting edges game missing")?;
    game.player_preferences = Some(PlayerPreferencesConfig {
        namespace: NAMESPACE.into(),
    });
    game.execution = ExecutionGraph {
        resources: [
            ("world", "GameWorld", Value::Null),
            ("input", "GameInput", Value::Null),
            ("events", "GameEvents", json!([])),
        ]
        .into_iter()
        .map(|(name, data_type, initial)| SystemResource {
            name: name.into(),
            data_type: data_type.into(),
            external: true,
            initial,
        })
        .collect(),
        stages: vec![SystemStage {
            name: "Author menu".into(),
            nodes: vec![SystemNode {
                id: SOURCE_ID.into(),
                module: "custom.rust.update".into(),
                params: json!({"source_id":SOURCE_ID}),
                bindings: BTreeMap::from([
                    ("world".into(), "world".into()),
                    ("input".into(), "input".into()),
                    ("events".into(), "events".into()),
                ]),
                after: vec![],
            }],
        }],
    };
    document
        .custom_sources
        .insert(SOURCE_ID.into(), MENU_SOURCE.into());
    crate::engine_profiles::finish_template(
        &mut document,
        crate::engine_profiles::ProfileKind::Neutral,
        &backend,
    )?;
    let document = externalize_scenes(&document, false)?;
    let project = output.join("project.json");
    crate::project::save(&project, &document)?;
    let source = output.join("menu.rs");
    fs::write(&source, MENU_SOURCE).map_err(|error| error.to_string())?;
    let group = document
        .engine
        .exposure
        .variants
        .get(SETTING_ID)
        .ok_or("Sample setting is not exposed")?;
    let selected = document
        .project
        .variants
        .get(SETTING_ID)
        .unwrap_or(&group.default);
    let report = json!({
        "schema_version":1,"kind":"player-settings-sample-preparation",
        "project":project,"namespace":NAMESPACE,"setting":SETTING_ID,
        "authored_selection":selected,"options":group.options.iter()
            .map(|(id,option)|json!({"id":id,"label":option.label})).collect::<Vec<_>>(),
        "source_id":SOURCE_ID,"instance_id":SOURCE_ID,"source_copy":source,
        "source_sha256":hash_file(&source)?,"project_sha256":hash_file(&project)?,
        "source_of_truth":"project.custom_sources[settings-menu]; menu.rs is a readable reference copy",
        "geometry":"Unmodified contrasting-edge objects and ordinary default GameHost camera",
        "scope":"Sample preparation only; no Custom compilation, GPU execution, Player export or usability verification"
    });
    fs::write(
        output.join("sample-report.json"),
        serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(report)
}
