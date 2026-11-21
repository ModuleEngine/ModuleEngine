use crate::{ArpgEvent, GameStateExt, GameStatus, legacy_input};
use module_engine_core::{
    Camera, GameContribution, GameProject, GameState, HostServiceRequest, SystemNode,
};
use std::collections::BTreeSet;

pub const AUDIO_SYSTEM: &str = "game.audio";
pub const AUDIO_SERVICE: &str = "audio.wav";
pub const PLAYBACK_ENABLED: &str = "playback_enabled";
pub const SOUND_EVENT: &str = "Sound";
pub const SOUND_CUE_FIELD: &str = "cue";

#[derive(Clone, Copy)]
pub enum SoundCue {
    Attack,
    Hit,
    Death,
    Victory,
}

impl SoundCue {
    pub const ALL: [Self; 4] = [Self::Attack, Self::Hit, Self::Death, Self::Victory];

    pub fn name(self) -> &'static str {
        match self {
            Self::Attack => "attack",
            Self::Hit => "hit",
            Self::Death => "death",
            Self::Victory => "victory",
        }
    }

    pub fn event(self) -> &'static str {
        match self {
            Self::Attack => "Attack",
            Self::Hit => "Hit",
            Self::Death => "Death",
            Self::Victory => "Victory",
        }
    }

    pub(crate) fn from_event(event: ArpgEvent) -> Option<(Self, Option<u64>)> {
        match event {
            ArpgEvent::Attack { attacker } => Some((Self::Attack, Some(attacker))),
            ArpgEvent::Hit { target, .. } => Some((Self::Hit, Some(target))),
            ArpgEvent::Death { object } => Some((Self::Death, Some(object))),
            ArpgEvent::Victory => Some((Self::Victory, None)),
            _ => None,
        }
    }
}

pub fn audio_nodes(project: &GameProject) -> impl Iterator<Item = &SystemNode> {
    project
        .execution
        .stages
        .iter()
        .flat_map(|stage| &stage.nodes)
        .filter(|node| node.module == AUDIO_SYSTEM)
}

pub fn playback_enabled(params: &serde_json::Value) -> bool {
    params.get(PLAYBACK_ENABLED) != Some(&serde_json::Value::Bool(false))
}

pub fn audio_playback_enabled(project: &GameProject) -> bool {
    audio_nodes(project).any(|node| playback_enabled(&node.params))
}

pub fn has_audio_request(project: &GameProject) -> bool {
    !project.audio_cues.is_empty() && audio_playback_enabled(project)
}

pub fn audio_cue_event(cue: &str) -> &'static str {
    SoundCue::ALL
        .into_iter()
        .find(|known| known.name() == cue)
        .map_or(SOUND_EVENT, SoundCue::event)
}

pub fn host_contribution() -> GameContribution {
    GameContribution {
        id: "arpg.presentation".into(),
        activation_modules: [
            "game.input",
            "game.ai",
            "game.movement",
            "game.collision",
            "game.combat",
            "game.health",
            "game.portals",
            AUDIO_SYSTEM,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>(),
        begin_tick: crate::state::begin_tick,
        initial_camera,
        follow_target,
        decorate_snapshot: apply_hit_flashes,
        script_input: legacy_input,
        validate_event: crate::validate_event,
        validate_state: |game| crate::state::read_frame(game).map(|_| ()),
        service_requests,
        diagnostics,
    }
}
/// Removing the authored audio node disables this service without touching
/// archived cues. Events may still exist without requesting a playback endpoint.
pub fn audio_request(project: &GameProject) -> Option<HostServiceRequest> {
    has_audio_request(project).then(|| HostServiceRequest {
        id: AUDIO_SERVICE.into(),
        asset_references: project.audio_cues.values().cloned().collect(),
        config: serde_json::json!({
            "cues":project.audio_cues,
            "event":SOUND_EVENT,
            "cue_field":SOUND_CUE_FIELD,
        }),
    })
}

pub fn service_requests(project: &GameProject) -> Result<Vec<HostServiceRequest>, String> {
    Ok(audio_request(project).into_iter().collect())
}
pub fn initial_camera(_: &GameProject) -> Camera {
    Camera {
        yaw: 0.,
        pitch: 0.95,
        distance: 21.,
        target: [0.; 3],
    }
}
pub fn follow_target(state: &GameState) -> Option<[f32; 3]> {
    state
        .player()
        .map(|player| player.object.transform.translation)
}
pub fn diagnostics(state: &GameState) -> serde_json::Value {
    serde_json::json!({
        "status": format!("{:?}", state.arpg().status),
        "active_enemies": state.living_enemies(),
        "player_health":state.player().and_then(|player| player.components.get("health").and_then(|value|value.get("current")).and_then(serde_json::Value::as_f64))
    })
}

pub fn apply_hit_flashes(snapshot: &mut module_engine_core::SceneSnapshot, state: &GameState) {
    let frame = state.arpg();
    if frame.status != GameStatus::Playing || frame.hit_visuals.is_empty() {
        return;
    }
    for item in &mut snapshot.items {
        let intensity = frame
            .hit_visuals
            .iter()
            .filter(|hit| {
                hit.target == item.id
                    && hit
                        .target_lifetime
                        .is_none_or(|lifetime| state.world.lifetime(item.id) == Some(lifetime))
            })
            .map(|hit| {
                let age = state.time - hit.started_at;
                if age < 0. || state.time >= hit.expires_at {
                    0.
                } else {
                    (1. - age / 0.16).clamp(0., 1.) as f32 * 0.95
                }
            })
            .fold(0_f32, f32::max);
        if intensity <= 0. {
            continue;
        }
        for (channel, white) in [255., 248., 205.].into_iter().enumerate() {
            item.color[channel] =
                (item.color[channel] as f32 * (1. - intensity) + white * intensity).round() as u8;
            item.material.base_color[channel] +=
                (1. - item.material.base_color[channel]) * intensity;
        }
    }
}

#[cfg(test)]
mod service_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn audio_service_discovery_uses_its_scheduled_node_and_preserves_raw_references() {
        let mut project = crate::demo_project();
        project.audio_cues =
            std::collections::BTreeMap::from([("hit".into(), "asset://tone".into())]);
        let before = project.clone();
        let requests = (host_contribution().service_requests)(&project).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].id, "audio.wav");
        assert_eq!(
            requests[0].asset_references,
            BTreeSet::from(["asset://tone".into()])
        );
        assert_eq!(
            requests[0].config,
            json!({"cues":{"hit":"asset://tone"},"event":"Sound","cue_field":"cue"})
        );
        assert_eq!(
            project, before,
            "Discovery must not resolve/read or rewrite author data"
        );
        for stage in &mut project.execution.stages {
            stage.nodes.retain(|node| node.module != "game.audio");
        }
        assert!(
            (host_contribution().service_requests)(&project)
                .unwrap()
                .is_empty()
        );
        assert_eq!(project.audio_cues, before.audio_cues);
        let mut empty = before;
        empty.audio_cues.clear();
        assert!(
            (host_contribution().service_requests)(&empty)
                .unwrap()
                .is_empty()
        );
    }
}
