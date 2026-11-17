//! Transactional local preferences and author menu state, without filesystem IO.
use module_engine_core::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(crate) struct PlayerSettingsState {
    pub choices: BTreeMap<String, String>,
    pub menus: ScriptUiSnapshot,
    pub rejection: BTreeMap<String, String>,
    payload: Value,
    enabled: bool,
    saved: bool,
    error: Option<String>,
}

impl PlayerSettingsState {
    pub fn load(
        &mut self,
        document: &GraphDocument,
        data: Option<&Value>,
        enabled: bool,
        diagnostics: &Value,
    ) {
        self.choices.clear();
        self.rejection.clear();
        self.menus.clear();
        self.enabled = enabled;
        self.saved = true;
        self.error = None;
        self.payload = data.cloned().unwrap_or_else(|| json!({}));
        if let Some(options) = self.payload.get("variants").and_then(Value::as_object) {
            for (setting, option) in options {
                let request = ScriptSettingRequest {
                    setting: setting.clone(),
                    option: option.as_str().unwrap_or_default().into(),
                };
                match validate_script_setting_request(document, &request) {
                    Ok(()) => {
                        self.choices.insert(setting.clone(), request.option);
                    }
                    Err(error) => {
                        self.rejection.insert(setting.clone(),format!("Saved choice is unavailable; using the current project choice. {error}"));
                    }
                }
            }
        } else if data.is_some()
            && (!self.payload.is_object() || self.payload.get("variants").is_some())
        {
            self.error=Some("Saved player preferences do not contain a variants object; original payload preserved".into());
        }
        self.observe_storage(diagnostics, false);
    }
    pub fn persistence(&self) -> ScriptSettingPersistence {
        ScriptSettingPersistence {
            enabled: self.enabled,
            saved: self.saved,
            error: self.error.clone(),
        }
    }
    pub fn snapshot(
        &self,
        document: &GraphDocument,
        time: f64,
        raw: &RenderOverrides,
    ) -> Result<ScriptSettingsSnapshot, String> {
        Ok(resolve_script_settings(
            document,
            time,
            raw,
            &self.choices,
            &self.persistence(),
            &self.rejection,
        )?
        .1)
    }
    pub fn controls(
        &self,
        document: &GraphDocument,
        time: f64,
        raw: &RenderOverrides,
    ) -> Result<RenderOverrides, String> {
        Ok(resolve_script_settings(
            document,
            time,
            raw,
            &self.choices,
            &self.persistence(),
            &self.rejection,
        )?
        .0)
    }
    pub fn menus(&self, settings: &ScriptSettingsSnapshot) -> ScriptUiSnapshot {
        self.menus
            .iter()
            .map(|(id, panels)| {
                (
                    id.clone(),
                    panels
                        .iter()
                        .filter_map(|panel| {
                            let mut panel = panel.clone();
                            panel.items.retain(|item| match item {
                                ScriptUiItem::Choice { setting, .. } => {
                                    settings.contains_key(setting)
                                }
                                _ => true,
                            });
                            (!panel.items.is_empty()).then_some(panel)
                        })
                        .collect(),
                )
            })
            .collect()
    }
    pub fn request(
        &mut self,
        document: &GraphDocument,
        requests: &[ScriptSettingRequest],
    ) -> Result<Option<GameEvent>, String> {
        if requests.len() > 128 {
            return Err("Player settings request batch exceeds 128".into());
        }
        for request in requests {
            validate_script_setting_request(document, request)?;
        }
        if requests.is_empty() {
            return Ok(None);
        }
        for request in requests {
            self.choices
                .insert(request.setting.clone(), request.option.clone());
            self.rejection.remove(&request.setting);
        }
        self.saved = false;
        let mut payload = self.payload.clone();
        if !payload.is_object()
            || payload
                .get("variants")
                .is_some_and(|value| !value.is_object())
        {
            self.error=Some("Choice is active only for this session: saved player preferences payload is incompatible; original bytes preserved".into());
            return Ok(None);
        }
        if payload.get("variants").is_none() {
            payload["variants"] = json!({});
        }
        for request in requests {
            payload["variants"][&request.setting] = json!(request.option);
        }
        self.payload = payload;
        self.error = if self.enabled {
            None
        } else {
            Some("This choice is active only for this session; player preferences persistence is unavailable".into())
        };
        Ok(self
            .enabled
            .then(|| GameEvent::new(PLAYER_SETTINGS_WRITE_EVENT, json!({"data":self.payload}))))
    }
    pub fn reject(&mut self, requests: &[ScriptSettingRequest], error: &str) {
        for request in requests {
            self.rejection.insert(request.setting.clone(), error.into());
        }
    }
    pub fn stage_request(
        &self,
        document: &GraphDocument,
        time: f64,
        raw: &RenderOverrides,
        requests: &[ScriptSettingRequest],
        validate: impl FnOnce(&RenderOverrides) -> Result<(), String>,
    ) -> Result<(Self, Option<GameEvent>), String> {
        let mut candidate = self.clone();
        let event = candidate.request(document, requests)?;
        if !requests.is_empty() {
            validate(&candidate.controls(document, time, raw)?)?;
        }
        Ok((candidate, event))
    }
    pub fn validate_loaded(
        &mut self,
        document: &GraphDocument,
        validate: impl FnOnce(&RenderOverrides) -> Result<(), String>,
    ) -> Result<(), String> {
        if !self.choices.is_empty() {
            let controls = self.controls(document, 0., &RenderOverrides::default())?;
            if let Err(error) = validate(&controls) {
                for setting in self.choices.keys() {
                    self.rejection.insert(
                        setting.clone(),
                        format!(
                            "Saved choice was rejected; using the current project choice. {error}"
                        ),
                    );
                }
                self.choices.clear();
            }
        }
        Ok(())
    }
    pub fn reconcile(&mut self, document: &GraphDocument) {
        let instances = document
            .game
            .as_ref()
            .map(|game| {
                game.execution
                    .stages
                    .iter()
                    .flat_map(|stage| &stage.nodes)
                    .map(|node| node.id.clone())
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        self.menus.retain(|id, _| instances.contains(id));
    }
    pub fn observe_storage(&mut self, diagnostics: &Value, wrote: bool) {
        if let Some(service) = diagnostics.get("persistence.json") {
            self.enabled = service.get("enabled").and_then(Value::as_bool) == Some(true);
            if let Some(error) = service.get("last_error").and_then(Value::as_str) {
                self.error = Some(format!(
                    "Choice is active but may not survive exit: {error}"
                ));
                self.saved = false;
            } else if wrote && service.get("enabled").and_then(Value::as_bool) == Some(true) {
                self.error = None;
                self.saved = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> GraphDocument {
        let mut document = demo_document(WorldKind::Ecs);
        document.engine.initialized = true;
        document.engine.exposure = default_renderer_exposure("tone", "aa");
        document
    }
    #[test]
    fn unknown_payload_survives_valid_choice_and_unavailable_saved_option_falls_back() {
        let document = document();
        let mut state = PlayerSettingsState::default();
        state.load(&document,Some(&json!({"future":{"null":null},"variants":{"antialiasing":"removed","future.group":"future.choice"}})),true,&json!({}));
        assert!(state.choices.is_empty());
        assert_eq!(
            state
                .snapshot(&document, 0., &RenderOverrides::default())
                .unwrap()["antialiasing"]
                .selected
                .as_deref(),
            Some("fxaa")
        );
        let before = state.clone();
        assert!(
            state
                .request(
                    &document,
                    &[ScriptSettingRequest {
                        setting: "antialiasing".into(),
                        option: "invalid".into()
                    }]
                )
                .is_err()
        );
        assert_eq!(state.payload, before.payload);
        let event = state
            .request(
                &document,
                &[ScriptSettingRequest {
                    setting: "antialiasing".into(),
                    option: "off".into(),
                }],
            )
            .unwrap()
            .unwrap();
        assert_eq!(event.payload["data"]["future"]["null"], Value::Null);
        assert_eq!(
            event.payload["data"]["variants"]["future.group"],
            json!("future.choice")
        );
        state.observe_storage(
            &json!({"persistence.json":{"enabled":true,"last_error":"write failed"}}),
            true,
        );
        assert!(!state.persistence().saved);
        assert_eq!(state.choices["antialiasing"], "off");
        state.observe_storage(
            &json!({"persistence.json":{"enabled":true,"last_error":null}}),
            true,
        );
        assert!(state.persistence().saved);
    }
    #[test]
    fn request_only_candidate_is_fully_validated_before_choices_menu_or_payload_publish() {
        let document = document();
        let mut state = PlayerSettingsState::default();
        state.load(&document, None, true, &json!({}));
        state.menus.insert(
            "instance".into(),
            vec![ScriptUiPanel {
                id: "menu".into(),
                title: "Author menu".into(),
                position: [0., 0.],
                width: 240.,
                items: vec![ScriptUiItem::Text {
                    text: "Last good".into(),
                }],
            }],
        );
        let before = state.clone();
        let request = ScriptSettingRequest {
            setting: "antialiasing".into(),
            option: "off".into(),
        };
        let result = state.stage_request(
            &document,
            0.,
            &RenderOverrides::default(),
            &[request],
            |effective| {
                assert!(!effective.enabled["aa"]);
                Err("Renderer adapter rejects complete combined candidate".into())
            },
        );
        assert!(result.is_err());
        assert_eq!(state.choices, before.choices);
        assert_eq!(state.menus, before.menus);
        assert_eq!(state.payload, before.payload);
    }
    #[test]
    fn malformed_saved_payload_allows_runtime_choice_without_overwriting_original() {
        let document = document();
        for payload in [json!(["future"]), json!({"variants":false,"unknown":null})] {
            let mut state = PlayerSettingsState::default();
            state.load(&document, Some(&payload), true, &json!({}));
            let event = state
                .request(
                    &document,
                    &[ScriptSettingRequest {
                        setting: "antialiasing".into(),
                        option: "off".into(),
                    }],
                )
                .unwrap();
            assert!(event.is_none());
            assert_eq!(state.payload, payload);
            assert_eq!(state.choices["antialiasing"], "off");
            assert!(!state.persistence().saved && state.persistence().error.is_some());
        }
    }
    #[test]
    fn loaded_composite_rejection_uses_project_without_altering_saved_payload() {
        let document = document();
        let mut state = PlayerSettingsState::default();
        let payload = json!({"variants":{"antialiasing":"off"}});
        state.load(&document, Some(&payload), true, &json!({}));
        state
            .validate_loaded(&document, |controls| {
                assert!(!controls.enabled["aa"]);
                Err("Combination unavailable".into())
            })
            .unwrap();
        assert!(state.choices.is_empty());
        assert_eq!(state.payload, payload);
        let settings = state
            .snapshot(&document, 0., &RenderOverrides::default())
            .unwrap();
        assert_eq!(settings["antialiasing"].selected.as_deref(), Some("fxaa"));
        assert!(settings["antialiasing"].rejection.is_some());
    }
    #[test]
    fn preferences_require_stable_identity_and_never_share_gameplay_storage_slot() {
        let mut game = GameProject::empty();
        assert!(player_preferences_service_request(&game).unwrap().is_none());
        game.player_preferences = Some(PlayerPreferencesConfig {
            namespace: "sample/game-a".into(),
        });
        let request = player_preferences_service_request(&game).unwrap().unwrap();
        assert!(super::super::preference_requests(&game, std::slice::from_ref(&request)).is_err());
        let mut different = request.clone();
        different.config["slot"] = json!("gameplay");
        assert!(super::super::preference_requests(&game, &[different]).is_ok());
        game.player_preferences.as_mut().unwrap().namespace.clear();
        assert!(player_preferences_service_request(&game).is_err());
    }
}
