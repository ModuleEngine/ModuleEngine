//! Optional Rapier physics for the authored game schedule.
mod queries;
mod schema;
mod simulation;

#[cfg(test)]
mod acceptance;
#[cfg(test)]
mod tests;

#[cfg(feature = "debug-draw")]
mod debug_draw;
#[cfg(feature = "debug-draw")]
pub use debug_draw::{DebugLine, DebugLineKind, debug_lines};

use module_engine_core::{
    CapabilityConfiguration, CapabilityKind, CapabilityMetadata, CapabilityOwner, ExecutionGraph,
    ResourceAccess, SystemDescriptor, SystemPort, SystemRegistry,
};

pub const MODULE_ID: &str = "physics.rapier";
pub const BODY: &str = "physics.rigid_body";
pub const COLLIDER: &str = "physics.collider";
pub const JOINT: &str = "physics.joint";
pub const COLLISION_STARTED: &str = "physics.collision_started";
pub const COLLISION_STOPPED: &str = "physics.collision_stopped";

pub fn is_scheduled(graph: &ExecutionGraph) -> bool {
    graph
        .stages
        .iter()
        .flat_map(|stage| &stage.nodes)
        .any(|node| node.module == MODULE_ID)
}

pub fn install(registry: &mut SystemRegistry) -> Result<(), String> {
    install_with(registry, |params| {
        Ok(Box::new(simulation::RapierStep::new(params)?))
    })
}

fn install_with(
    registry: &mut SystemRegistry,
    factory: impl Fn(
        &serde_json::Value,
    ) -> Result<Box<dyn module_engine_core::ExecutableSystem>, String>
    + Send
    + Sync
    + 'static,
) -> Result<(), String> {
    let descriptor = SystemDescriptor {
        id: MODULE_ID.into(),
        state_schema: "physics.rapier/1".into(),
        components: vec![BODY.into(), COLLIDER.into(), JOINT.into()],
        ports: vec![
            SystemPort {
                name: "world".into(),
                data_type: "GameWorld".into(),
                access: ResourceAccess::ReadWrite,
            },
            SystemPort {
                name: "events".into(),
                data_type: "GameEvents".into(),
                access: ResourceAccess::ReadWrite,
            },
        ],
        requires: vec![],
    };
    registry.register(descriptor, factory)?;
    schema::install(registry.components_mut())?;
    registry.register_project_contract(
        MODULE_ID,
        |project, _| {
            let count = project
                .execution
                .stages
                .iter()
                .flat_map(|stage| &stage.nodes)
                .filter(|node| node.module == MODULE_ID)
                .count();
            if count > 1 {
                return Err("The execution graph permits one physics.rapier step".into());
            }
            Ok(())
        },
        |_| Ok(()),
    )?;
    registry.describe_module(
        MODULE_ID,
        CapabilityMetadata::new(
            "Rapier physics",
            "Advance rigid bodies, colliders and joints at 60 Hz. Publish queries and collision events.",
            CapabilityOwner::Engine,
            CapabilityKind::ExecutableSystem,
            env!("CARGO_PKG_NAME"),
        )
        .configured_by([
            CapabilityConfiguration::ExecutionGraph,
            CapabilityConfiguration::ComponentFields,
        ]),
    )
}
