//! Optional ARPG rules and presentation installed through the common contracts.
use module_engine_core::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

mod components;
mod events;
mod presentation;
mod project;
mod queries;
mod state;
mod systems;
mod verification;

pub use components::*;
pub use events::*;
pub use presentation::*;
pub use project::*;
pub use queries::*;
pub use state::*;
pub use verification::*;

#[cfg(feature = "ui")]
pub mod ui;

// Existing package regressions explicitly reuse the example's core-only data.
// Production ARPG has no dependency on the concrete example project.
#[cfg(test)]
#[path = "../../example-two-arenas/src/content.rs"]
mod two_arenas_fixture;

#[cfg(test)]
pub(crate) fn demo_project() -> GameProject {
    two_arenas_fixture::build_project(gameplay_schedule())
}

#[cfg(test)]
pub(crate) trait ProjectExt {
    fn demo() -> Self;
}
#[cfg(test)]
impl ProjectExt for GameProject {
    fn demo() -> Self {
        demo_project()
    }
}

/// One native installation; UI hosts select the package's optional callbacks.
pub fn install(registry: &mut SystemRegistry) -> Result<(), String> {
    register_arpg_component_schemas(registry.components_mut())?;
    systems::register_arpg_systems(registry);
    install_project_contracts(registry)?;
    registry.register_game_contribution(host_contribution())?;
    for (id, label, purpose) in [
        (
            "game.input",
            "ARPG player intentions",
            "Translate player movement and aim into the ARPG's actor intentions.",
        ),
        (
            "game.ai",
            "ARPG enemy behavior",
            "Choose pursuit and attack intentions using the ARPG enemy rules.",
        ),
        (
            "game.movement",
            "ARPG actor movement",
            "Move ARPG actors and publish previous positions for the collision step.",
        ),
        (
            "game.collision",
            "ARPG obstacle collision",
            "Resolve the ARPG actors against its authored obstacles; not a general rigid-body physics service.",
        ),
        (
            "game.combat",
            "ARPG melee combat",
            "Apply the game's attack areas, hit timing and damage rules.",
        ),
        (
            "game.health",
            "ARPG health and outcome",
            "Apply ARPG health, death and configured victory/defeat rules.",
        ),
        (
            "game.portals",
            "ARPG scene transitions",
            "Apply the game's portal conditions and carry selected actors into another scene.",
        ),
        (
            AUDIO_SYSTEM,
            "ARPG sound events",
            "Translate ARPG gameplay events into game-selected sound cue requests; native playback is a separate service.",
        ),
    ] {
        registry.describe_module(
            id,
            CapabilityMetadata::new(
                label,
                purpose,
                CapabilityOwner::Gameplay,
                CapabilityKind::ExecutableSystem,
                env!("CARGO_PKG_NAME"),
            )
            .configured_by([
                CapabilityConfiguration::ExecutionGraph,
                if id == AUDIO_SYSTEM {
                    CapabilityConfiguration::ProjectSettings
                } else {
                    CapabilityConfiguration::ComponentFields
                },
                CapabilityConfiguration::NativeSource,
            ]),
        )?;
    }
    registry.describe_contribution("arpg.presentation", CapabilityMetadata::new(
        "ARPG host integration", "Adapt the ARPG camera, scene, input, events and optional service requests to a host; UI callbacks are separately compiled.",
        CapabilityOwner::Gameplay, CapabilityKind::HostContribution, env!("CARGO_PKG_NAME"),
    ).configured_by([CapabilityConfiguration::NativeSource]))?;
    Ok(())
}

/// Compare every portable session field, ignoring only process-local ARPG
/// incarnation tokens through the typed frame equality contract.
pub fn states_equivalent(left: &GameState, right: &GameState) -> bool {
    let (Ok(left_frame), Ok(right_frame)) = (state::read_frame(left), state::read_frame(right))
    else {
        return false;
    };
    let mut left_data = left.module_data.clone();
    let mut right_data = right.module_data.clone();
    left_data.remove(FRAME_KEY);
    right_data.remove(FRAME_KEY);
    left.scene == right.scene
        && left.time == right.time
        && left.tick == right.tick
        && left.world == right.world
        && left.control == right.control
        && left_data == right_data
        && left_frame == right_frame
}

pub fn registry() -> SystemRegistry {
    let mut registry = SystemRegistry::builtin();
    install(&mut registry).expect("Valid source ARPG contracts");
    registry
}

pub fn session(project: GameProject, kind: WorldKind) -> Result<GameSession, String> {
    GameSession::new_with_registry(project, kind, registry())
}
