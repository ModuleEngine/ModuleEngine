//! Portable, author-owned setting bindings. No native UI or filesystem access.
use crate::{
    GameInput, GameProject, GraphDocument, HostServiceRequest, RenderCommand, RenderOverrides,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const PLAYER_SETTINGS_DATA_KEY: &str = "module-engine.player-settings";
pub const PLAYER_SETTINGS_WRITE_EVENT: &str = "module-engine.player-settings.save";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerPreferencesConfig {
    /// Stable game identity, independent of package location and build identity.
    pub namespace: String,
}

pub fn player_preferences_service_request(
    game: &GameProject,
) -> Result<Option<HostServiceRequest>, String> {
    game.player_preferences.as_ref().map(|config| {
        if config.namespace.trim().is_empty() || config.namespace.len() > 96 || config.namespace.contains('\0') {
            return Err("Player preferences namespace needs 1..=96 non-NUL UTF-8 bytes".into());
        }
        Ok(HostServiceRequest {
            id: "persistence.json".into(),
            config: json!({"namespace":config.namespace,"slot":"player-settings","data_key":PLAYER_SETTINGS_DATA_KEY,
                "write_event":PLAYER_SETTINGS_WRITE_EVENT,"payload_field":"data","version":1,"invalid_save":"read_only"}),
            asset_references: BTreeSet::new(),
        })
    }).transpose()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptSettingOption {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptSettingPersistence {
    pub enabled: bool,
    /// Whether the current choice will survive a process restart.
    pub saved: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptSettingSnapshot {
    pub label: String,
    pub options: Vec<ScriptSettingOption>,
    /// None means final render controls do not represent one unambiguous preset.
    pub selected: Option<String>,
    /// Stable literals: local, project, runtime, mixed. project includes rules/defaults.
    pub source: String,
    pub persistence: ScriptSettingPersistence,
    pub rejection: Option<String>,
}
pub type ScriptSettingsSnapshot = BTreeMap<String, ScriptSettingSnapshot>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptSettingRequest {
    pub setting: String,
    pub option: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptUiPanel {
    pub id: String,
    pub title: String,
    pub position: [f32; 2],
    pub width: f32,
    pub items: Vec<ScriptUiItem>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ScriptUiItem {
    Text {
        text: String,
    },
    Choice {
        id: String,
        label: String,
        setting: String,
    },
}
pub type ScriptUiSnapshot = BTreeMap<String, Vec<ScriptUiPanel>>;

/// A JSON tuple keeps arbitrary IDs distinct without splitting on punctuation.
pub fn script_ui_action_key(instance: &str, control: &str) -> String {
    format!("script-ui:{}", json!([instance, control]))
}
pub fn script_ui_actions(input: &GameInput, instance: &str) -> BTreeMap<String, Value> {
    input
        .actions
        .iter()
        .filter_map(|key| {
            let pair: [String; 2] = serde_json::from_str(key.strip_prefix("script-ui:")?).ok()?;
            (pair[0] == instance)
                .then(|| {
                    input
                        .action_values
                        .get(key)
                        .map(|v| (pair[1].clone(), v.clone()))
                })
                .flatten()
        })
        .collect()
}

fn bounded_text(value: &str, max: usize) -> bool {
    !value.contains('\0') && value.len() <= max
}
pub fn validate_script_ui(panels: &[ScriptUiPanel]) -> Result<(), String> {
    if panels.len() > 16 {
        return Err("Custom menu exceeds 16 panels".into());
    }
    let mut ids = BTreeSet::new();
    let mut controls = BTreeSet::new();
    let mut count = 0;
    for panel in panels {
        if panel.id.is_empty()
            || !bounded_text(&panel.id, 256)
            || !ids.insert(&panel.id)
            || !bounded_text(&panel.title, 1024)
            || !panel
                .position
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 10000.)
            || !panel.width.is_finite()
            || !(120. ..=1600.).contains(&panel.width)
        {
            return Err("Invalid Custom menu panel identity, geometry or title".into());
        }
        count += panel.items.len();
        if count > 128 {
            return Err("Custom menu exceeds 128 items".into());
        }
        for item in &panel.items {
            match item {
                ScriptUiItem::Text { text } if bounded_text(text, 4096) => {}
                ScriptUiItem::Choice { id, label, setting }
                    if !id.is_empty()
                        && bounded_text(id, 256)
                        && controls.insert(id)
                        && bounded_text(label, 1024)
                        && !setting.is_empty()
                        && bounded_text(setting, 256) => {}
                _ => return Err("Invalid or duplicate Custom menu item".into()),
            }
        }
    }
    Ok(())
}

fn target(command: &RenderCommand) -> Option<String> {
    match command {
        RenderCommand::SetParameter { pass, name, .. } => {
            Some(json!(["parameter", pass, name]).to_string())
        }
        RenderCommand::SetEnabled { pass, .. } => Some(json!(["enabled", pass]).to_string()),
        _ => None,
    }
}
/// Player choices must be persistent scalar/enabled presets with exclusive targets.
pub fn validate_script_setting_request(
    document: &GraphDocument,
    request: &ScriptSettingRequest,
) -> Result<(), String> {
    let group = document
        .engine
        .exposure
        .variants
        .get(&request.setting)
        .ok_or_else(|| format!("Setting '{}' is unavailable", request.setting))?;
    let option = group.options.get(&request.option).ok_or_else(|| {
        format!(
            "Option '{}' is unavailable for '{}'",
            request.option, request.setting
        )
    })?;
    if option.commands.is_empty()
        || group
            .options
            .values()
            .flat_map(|v| &v.commands)
            .any(|c| target(c).is_none())
    {
        return Err("Player setting requires persistent parameter/enabled variants".into());
    }
    let targets = group
        .options
        .values()
        .flat_map(|v| &v.commands)
        .filter_map(target)
        .collect::<BTreeSet<_>>();
    if document
        .engine
        .exposure
        .variants
        .iter()
        .filter(|(id, _)| *id != &request.setting)
        .flat_map(|(_, g)| g.options.values())
        .flat_map(|v| &v.commands)
        .filter_map(target)
        .any(|t| targets.contains(&t))
    {
        return Err(format!(
            "Setting '{}' shares render targets with another setting",
            request.setting
        ));
    }
    // Repeated commands for one target can make the UI's apparent preset ambiguous.
    let mut signatures = BTreeSet::new();
    for variant in group.options.values() {
        let mut seen = BTreeSet::new();
        if variant
            .commands
            .iter()
            .filter_map(target)
            .any(|t| !seen.insert(t))
        {
            return Err("Player setting contains repeated render targets".into());
        }
        if seen != targets {
            return Err(
                "Player setting variants must define the same complete render targets".into(),
            );
        }
        let signature = variant
            .commands
            .iter()
            .filter_map(|command| {
                target(command).map(|id| {
                    (
                        id,
                        serde_json::to_value(command).expect("Serializable render command"),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        if !signatures
            .insert(serde_json::to_string(&signature).expect("Serializable render command map"))
        {
            return Err("Player setting variants have ambiguous identical commands".into());
        }
    }
    Ok(())
}

fn matches_controls(commands: &[RenderCommand], controls: &RenderOverrides) -> bool {
    !commands.is_empty()
        && commands.iter().all(|command| match command {
            RenderCommand::SetEnabled { pass, enabled } => {
                controls.enabled.get(pass) == Some(enabled)
            }
            RenderCommand::SetParameter { pass, name, value } => {
                controls.parameters.get(pass).and_then(|p| p.get(name)) == Some(value)
            }
            _ => false,
        })
}

/// One resolver is used for script input, menu snapshots and all render routes.
/// Local choices override group rules and raw commands; untouched groups retain legacy precedence.
pub fn resolve_script_settings(
    document: &GraphDocument,
    time: f64,
    raw: &RenderOverrides,
    choices: &BTreeMap<String, String>,
    persistence: &ScriptSettingPersistence,
    rejections: &BTreeMap<String, String>,
) -> Result<(RenderOverrides, ScriptSettingsSnapshot), String> {
    let commands = crate::project_commands(document, time)?
        .into_iter()
        .filter_map(|c| match c {
            crate::ControlCommand::Render(c) => Some(c),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut controls = RenderOverrides::default();
    controls.apply_commands(&commands)?;
    let project_controls = controls.clone();
    for (pass, parameters) in &raw.parameters {
        controls
            .parameters
            .entry(pass.clone())
            .or_default()
            .extend(parameters.clone());
    }
    controls.enabled.extend(raw.enabled.clone());
    controls
        .resource_commands
        .extend(raw.resource_commands.clone());
    for (setting, option) in choices {
        let request = ScriptSettingRequest {
            setting: setting.clone(),
            option: option.clone(),
        };
        if validate_script_setting_request(document, &request).is_ok() {
            controls.apply_commands(
                &document.engine.exposure.variants[setting].options[option].commands,
            )?;
        }
    }
    let mut snapshots = BTreeMap::new();
    for (id, group) in &document.engine.exposure.variants {
        if !group.options.keys().any(|option| {
            validate_script_setting_request(
                document,
                &ScriptSettingRequest {
                    setting: id.clone(),
                    option: option.clone(),
                },
            )
            .is_ok()
        }) {
            continue;
        }
        let matching = group
            .options
            .iter()
            .filter(|(_, v)| matches_controls(&v.commands, &controls))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        let selected = (matching.len() == 1).then(|| matching[0].clone());
        let local = choices.get(id).filter(|option| {
            validate_script_setting_request(
                document,
                &ScriptSettingRequest {
                    setting: id.clone(),
                    option: (*option).clone(),
                },
            )
            .is_ok()
        });
        let source = if local.is_some() && local == selected.as_ref() {
            "local"
        } else if selected.is_none() {
            "mixed"
        } else if selected
            .as_ref()
            .is_some_and(|o| matches_controls(&group.options[o].commands, &project_controls))
        {
            "project"
        } else {
            "runtime"
        };
        snapshots.insert(
            id.clone(),
            ScriptSettingSnapshot {
                label: group.label.clone(),
                options: group
                    .options
                    .iter()
                    .map(|(id, v)| ScriptSettingOption {
                        id: id.clone(),
                        label: v.label.clone(),
                    })
                    .collect(),
                selected,
                source: source.into(),
                persistence: persistence.clone(),
                rejection: rejections.get(id).cloned(),
            },
        );
    }
    Ok((controls, snapshots))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_choice_overrides_rules_and_raw_commands_without_mutating_authored_data() {
        let mut document = crate::demo_document(crate::WorldKind::Ecs);
        document.engine.initialized = true;
        document.engine.exposure = crate::default_renderer_exposure("tone", "aa");
        document
            .project
            .variants
            .insert("antialiasing".into(), "fxaa".into());
        document.project.rules.push(crate::VariantRule {
            group: "antialiasing".into(),
            variant: "fxaa".into(),
            condition: crate::RuleCondition::Time {
                op: crate::Comparison::GreaterOrEqual,
                value: 0.,
            },
        });
        let before = document.clone();
        let mut raw = RenderOverrides::default();
        raw.enabled.insert("aa".into(), true);
        let choices = BTreeMap::from([("antialiasing".into(), "off".into())]);
        let (effective, settings) = resolve_script_settings(
            &document,
            1.,
            &raw,
            &choices,
            &ScriptSettingPersistence::default(),
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(!effective.enabled["aa"]);
        assert_eq!(settings["antialiasing"].selected.as_deref(), Some("off"));
        assert_eq!(settings["antialiasing"].source, "local");
        assert_eq!(document, before);
        let bad = ScriptSettingRequest {
            setting: "antialiasing".into(),
            option: "missing".into(),
        };
        assert!(validate_script_setting_request(&document, &bad).is_err());
    }
    #[test]
    fn menu_actions_keep_instance_and_control_ids_distinct() {
        let mut input = GameInput::default();
        let a = script_ui_action_key("one.two", "three");
        let b = script_ui_action_key("one", "two.three");
        assert_ne!(a, b);
        input.actions.insert(a.clone());
        input.action_values.insert(a, json!("off"));
        assert_eq!(script_ui_actions(&input, "one.two")["three"], json!("off"));
        assert!(script_ui_actions(&input, "one").is_empty());
    }
    #[test]
    fn unsupported_groups_are_hidden_but_legacy_render_commands_are_preserved() {
        let mut document = crate::demo_document(crate::WorldKind::Ecs);
        document.engine.initialized = true;
        document.engine.exposure = crate::default_renderer_exposure("tone", "aa");
        document.engine.exposure.variants.insert(
            "resource".into(),
            crate::VariantGroup {
                label: "One frame operation".into(),
                default: "clear".into(),
                options: BTreeMap::from([(
                    "clear".into(),
                    crate::ReadyVariant {
                        label: "Clear".into(),
                        commands: vec![RenderCommand::ClearResource {
                            name: "image".into(),
                            value: [0.; 4],
                        }],
                    },
                )]),
            },
        );
        let (controls, settings) = resolve_script_settings(
            &document,
            0.,
            &RenderOverrides::default(),
            &BTreeMap::new(),
            &ScriptSettingPersistence::default(),
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(!settings.contains_key("resource"));
        assert_eq!(controls.resource_commands.len(), 1);
        assert!(
            validate_script_setting_request(
                &document,
                &ScriptSettingRequest {
                    setting: "missing".into(),
                    option: "clear".into()
                }
            )
            .is_err()
        );
        document.engine.exposure.variants.insert(
            "overlap".into(),
            document.engine.exposure.variants["antialiasing"].clone(),
        );
        let (_, settings) = resolve_script_settings(
            &document,
            0.,
            &RenderOverrides::default(),
            &BTreeMap::new(),
            &ScriptSettingPersistence::default(),
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(!settings.contains_key("antialiasing") && !settings.contains_key("overlap"));
    }
    #[test]
    fn mixed_runtime_state_is_not_reported_as_a_prepared_selection() {
        let mut document = crate::demo_document(crate::WorldKind::Ecs);
        document.engine.initialized = true;
        document.engine.exposure = crate::default_renderer_exposure("tone", "aa");
        let mut raw = RenderOverrides::default();
        raw.parameters
            .insert("tone".into(), BTreeMap::from([("mode".into(), json!(0.5))]));
        let (_, settings) = resolve_script_settings(
            &document,
            0.,
            &raw,
            &BTreeMap::new(),
            &ScriptSettingPersistence::default(),
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(settings["tonemap"].selected.is_none());
        assert_eq!(settings["tonemap"].source, "mixed");
    }
}
