//! Game-owned presentation callbacks shared by native application shells.
use egui::{self, Rect};
use module_engine_core::{Camera, GameInput, GameState, GameUiResponse, GameUiStatus};
use std::collections::BTreeMap;

pub type InputCallback = fn(&egui::Ui, &egui::Response, Rect, Camera, bool) -> GameInput;
pub type HudCallback = fn(&mut egui::Ui, Rect, &GameState, Camera, Option<[f32; 3]>) -> bool;
pub type ControlsCallback =
    fn(&mut egui::Ui, Rect, &GameState, Camera, Option<[f32; 3]>) -> GameInput;
pub type StatusControlsCallback =
    fn(&mut egui::Ui, Rect, &GameState, Camera, Option<[f32; 3]>, &GameUiStatus) -> GameUiResponse;
pub type AimCallback = fn(&GameState, &GameInput, bool, bool) -> Option<[f32; 3]>;

pub struct UiContribution {
    pub id: &'static str,
    pub input: InputCallback,
    pub hud: HudCallback,
    /// Replaces the legacy restart-only HUD when present.
    pub controls: Option<ControlsCallback>,
    /// Status-aware source UI; replaces older controls without changing GameInput.
    pub controls_with_status: Option<StatusControlsCallback>,
    pub preview_aim: Option<AimCallback>,
    pub has_input: Option<fn(&GameInput) -> bool>,
    pub capture_input: Option<fn(&GameState) -> GameInput>,
}

#[derive(Default)]
pub struct UiRegistry {
    contributions: BTreeMap<&'static str, UiContribution>,
    systems: Option<module_engine_core::SystemRegistry>,
}
impl UiRegistry {
    /// Bind presentation registration to the exact installed runtime catalog.
    pub fn for_systems(systems: &module_engine_core::SystemRegistry) -> Self {
        Self {
            contributions: BTreeMap::new(),
            systems: Some(systems.clone()),
        }
    }

    pub fn register(&mut self, contribution: UiContribution) -> Result<(), String> {
        if self.systems.as_ref().is_some_and(|systems| {
            !systems
                .game_contributions()
                .any(|owner| owner.id == contribution.id)
        }) {
            return Err(format!(
                "UI contribution {} has no installed runtime owner",
                contribution.id
            ));
        }
        if contribution.id.is_empty() || self.contributions.contains_key(contribution.id) {
            return Err(format!(
                "Invalid or duplicate UI contribution {}",
                contribution.id
            ));
        }
        self.contributions.insert(contribution.id, contribution);
        Ok(())
    }
    pub fn get(&self, id: Option<&str>) -> Option<&UiContribution> {
        self.contributions.get(id?)
    }
}

impl UiContribution {
    pub fn draw_controls_with_status(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        state: &GameState,
        camera: Camera,
        aim: Option<[f32; 3]>,
        status: &GameUiStatus,
    ) -> GameUiResponse {
        if let Some(callback) = self.controls_with_status {
            let mut response = callback(ui, rect, state, camera, aim, status);
            response.close_application &= status.can_close_application;
            response
        } else {
            GameUiResponse {
                input: self.draw_controls(ui, rect, state, camera, aim),
                close_application: false,
            }
        }
    }
    pub fn draw_controls(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        state: &GameState,
        camera: Camera,
        aim: Option<[f32; 3]>,
    ) -> GameInput {
        if let Some(callback) = self.controls_with_status {
            callback(ui, rect, state, camera, aim, &GameUiStatus::default()).input
        } else if let Some(callback) = self.controls {
            callback(ui, rect, state, camera, aim)
        } else {
            GameInput {
                restart: (self.hud)(ui, rect, state, camera, aim),
                ..Default::default()
            }
        }
    }
}

pub mod authoring;

#[cfg(test)]
mod tests {
    use super::*;
    use module_engine_core::{GameContribution, SystemDescriptor, SystemRegistry};
    use std::collections::BTreeSet;

    fn presentation(id: &'static str) -> UiContribution {
        UiContribution {
            id,
            input: |_, _, _, _, _| GameInput::default(),
            hud: |_, _, _, _, _| false,
            controls: None,
            controls_with_status: None,
            preview_aim: None,
            has_input: None,
            capture_input: None,
        }
    }

    #[test]
    fn presentation_requires_its_installed_runtime_owner_and_preserves_existing_entry() {
        let mut systems = SystemRegistry::default();
        let mut absent = UiRegistry::for_systems(&systems);
        assert!(
            absent
                .register(presentation("independent.presentation"))
                .is_err()
        );
        assert!(absent.get(Some("independent.presentation")).is_none());
        systems
            .register(
                SystemDescriptor {
                    id: "independent.rules".into(),
                    state_schema: "1".into(),
                    components: vec![],
                    ports: vec![],
                    requires: vec![],
                },
                |_| Err("Unused test factory".into()),
            )
            .unwrap();
        systems
            .register_game_contribution(GameContribution {
                id: "independent.presentation".into(),
                activation_modules: BTreeSet::from(["independent.rules".into()]),
                begin_tick: |_| Ok(()),
                initial_camera: |_| Camera::default(),
                follow_target: |_| None,
                decorate_snapshot: |_, _| {},
                script_input: |_| serde_json::Value::Null,
                validate_event: |_| Ok(()),
                validate_state: |_| Ok(()),
                service_requests: |_| Ok(vec![]),
                diagnostics: |_| serde_json::Value::Null,
            })
            .unwrap();
        let mut registry = UiRegistry::for_systems(&systems);
        assert!(
            registry
                .register(presentation("different.presentation"))
                .is_err()
        );
        registry
            .register(presentation("independent.presentation"))
            .unwrap();
        let mut duplicate = presentation("independent.presentation");
        duplicate.hud = |_, _, _, _, _| true;
        assert!(registry.register(duplicate).is_err());
        assert!(
            (registry
                .get(Some("independent.presentation"))
                .unwrap()
                .controls)
                .is_none()
        );
        assert!(registry.get(Some("different.presentation")).is_none());
        // The accepted original callback survives the rejected replacement.
        let context = egui::Context::default();
        context
            .run_ui(egui::RawInput::default(), |ui| {
                let state: GameState = serde_json::from_value(serde_json::json!({
                    "scene":"empty","time":0.,"tick":0,"entities":{}
                }))
                .unwrap();
                assert!(!(registry
                    .get(Some("independent.presentation"))
                    .unwrap()
                    .hud)(
                    ui, Rect::NOTHING, &state, Camera::default(), None,
                ));
            })
            .drop_without_applying_deltas();
    }
}
