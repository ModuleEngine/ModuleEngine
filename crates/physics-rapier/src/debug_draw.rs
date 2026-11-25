use crate::simulation::Space;
use module_engine_core::EcsWorld;
use rapier3d::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugLineKind {
    Body,
    Collider,
    Sensor,
    Joint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebugLine {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub kind: DebugLineKind,
}

#[derive(Default)]
struct Lines(Vec<DebugLine>);

impl DebugRenderBackend for Lines {
    fn draw_line(
        &mut self,
        object: DebugRenderObject<'_>,
        start: Vector,
        end: Vector,
        _: DebugColor,
    ) {
        let kind = match object {
            DebugRenderObject::Collider(_, collider) if collider.is_sensor() => {
                DebugLineKind::Sensor
            }
            DebugRenderObject::Collider(..) => DebugLineKind::Collider,
            DebugRenderObject::ImpulseJoint(..) => DebugLineKind::Joint,
            DebugRenderObject::RigidBody(..) => DebugLineKind::Body,
            _ => return,
        };
        self.0.push(DebugLine {
            start: start.to_array(),
            end: end.to_array(),
            kind,
        });
    }
}

/// Convert the accepted author-preview ECS to Rapier lines without stepping time.
pub fn debug_lines(world: &EcsWorld) -> Result<Vec<DebugLine>, String> {
    let mut space = Space::default();
    space.sync(world)?;
    space
        .bodies
        .propagate_modified_body_positions_to_colliders(&mut space.colliders);
    let mut pipeline = DebugRenderPipeline::new(
        DebugRenderStyle::default(),
        DebugRenderMode::RIGID_BODY_AXES
            | DebugRenderMode::COLLIDER_SHAPES
            | DebugRenderMode::IMPULSE_JOINTS,
    );
    let mut lines = Lines::default();
    pipeline.render(
        &mut lines,
        &space.bodies,
        &space.colliders,
        &space.impulse_joints,
        &space.multibody_joints,
        &space.narrow_phase,
        &space.soft_bodies,
    );
    Ok(lines.0)
}
