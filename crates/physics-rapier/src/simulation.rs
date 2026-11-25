use crate::{
    BODY, COLLIDER, COLLISION_STARTED, COLLISION_STOPPED, JOINT,
    queries::{bounded, from_rapier_pose, shared_shape, to_rapier_pose, unit_vector, valid_pose},
    schema::{BodyKind, BodySpec, ColliderSpec, JointKind, JointSpec},
};
use module_engine_core::{EcsWorld, ExecutableSystem, GameEvent, PhysicsPose, SystemContext};
use rapier3d::prelude::*;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};

const DT: f32 = 1.0 / 60.0;

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    gravity: [f32; 3],
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            gravity: [0.0, -9.81, 0.0],
        }
    }
}

pub(crate) struct RapierStep {
    gravity: Vector,
    pipeline: PhysicsPipeline,
    ccd: CCDSolver,
    accepted: Arc<Space>,
    staged: Option<Arc<Space>>,
    before_commit: Option<Arc<Space>>,
}

impl RapierStep {
    pub fn new(params: &Value) -> Result<Self, String> {
        let settings = if params.is_null() {
            Settings::default()
        } else {
            Settings::deserialize(params).map_err(|error| format!("Rapier settings: {error}"))?
        };
        if !bounded(settings.gravity, 10_000.0) {
            return Err("Rapier gravity must be finite and within ±10000".into());
        }
        Ok(Self {
            gravity: Vector::from_array(settings.gravity),
            pipeline: PhysicsPipeline::new(),
            ccd: CCDSolver::new(),
            accepted: Arc::new(Space::default()),
            staged: None,
            before_commit: None,
        })
    }
}

impl ExecutableSystem for RapierStep {
    fn step(&mut self, ctx: &mut SystemContext<'_>, _: &mut Value) -> Result<(), String> {
        if ctx.dt != DT {
            return Err("Rapier requires the 60 Hz game tick".into());
        }
        self.before_commit = None;
        let mut space = (*self.accepted).clone();
        let removed = space.sync(&ctx.world("world")?.world)?;
        let collisions = Collisions::default();
        let params = IntegrationParameters {
            dt: DT,
            max_ccd_substeps: 2,
            ..Default::default()
        };
        self.pipeline.step(
            self.gravity,
            &params,
            &mut space.islands,
            &mut space.broad_phase,
            &mut space.narrow_phase,
            &mut space.bodies,
            &mut space.colliders,
            &mut space.impulse_joints,
            &mut space.multibody_joints,
            &mut space.soft_bodies,
            &mut self.ccd,
            &(),
            &collisions,
        );

        space.write_transforms(&mut ctx.world_mut("world")?.world)?;
        let events = space.collision_events(collisions, removed)?;
        ctx.events_resource_mut("events")?.extend(events);
        let space = Arc::new(space);
        ctx.publish_physics_queries(space.clone())?;
        self.staged = Some(space);
        Ok(())
    }

    fn commit_tick(&mut self) -> Result<(), String> {
        if let Some(space) = self.staged.take() {
            self.before_commit = Some(std::mem::replace(&mut self.accepted, space));
        }
        Ok(())
    }

    fn rollback_tick(&mut self) -> Result<(), String> {
        self.staged = None;
        if let Some(previous) = self.before_commit.take() {
            self.accepted = previous;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct BodyRow {
    handle: RigidBodyHandle,
    lifetime: u64,
    spec: BodySpec,
    pose: PhysicsPose,
}

#[derive(Clone, Copy)]
struct ColliderRow {
    handle: ColliderHandle,
    lifetime: u64,
    parent: Option<RigidBodyHandle>,
    spec: ColliderSpec,
    pose: PhysicsPose,
}

#[derive(Clone, Copy)]
struct JointRow {
    handle: ImpulseJointHandle,
    owner: RigidBodyHandle,
    target: RigidBodyHandle,
    spec: JointSpec,
}

/// Rapier's derived scene state. Bevy still owns object identity, config and
/// transforms. Rapier's pipelines are scratch, outside the checkpoint.
#[derive(Clone, Default)]
pub(crate) struct Space {
    pub islands: IslandManager,
    pub broad_phase: BroadPhaseBvh,
    pub narrow_phase: NarrowPhase,
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,
    pub impulse_joints: ImpulseJointSet,
    pub multibody_joints: MultibodyJointSet,
    pub soft_bodies: SoftBodySet,
    body_rows: BTreeMap<u64, BodyRow>,
    collider_rows: BTreeMap<u64, ColliderRow>,
    joint_rows: BTreeMap<u64, JointRow>,
}

type RemovedColliders = HashMap<ColliderHandle, u64>;

fn component_rows<T: DeserializeOwned>(
    world: &EcsWorld,
    component: &str,
) -> Result<BTreeMap<u64, (u64, T)>, String> {
    world
        .entities_with(&[component])
        .into_iter()
        .map(|id| {
            let payload = world
                .component(id, component)
                .expect("component query membership");
            let spec = T::deserialize(payload)
                .map_err(|error| format!("Object {id}, {component}: {error}"))?;
            Ok((
                id,
                (world.lifetime(id).expect("queried runtime entity"), spec),
            ))
        })
        .collect()
}

fn object_pose(world: &EcsWorld, id: u64) -> Result<PhysicsPose, String> {
    let object = world
        .object(id)
        .ok_or_else(|| format!("Missing physics object {id}"))?;
    if object.parent.is_some() {
        return Err(format!("Physics object {id} must be a scene root"));
    }
    let pose = PhysicsPose {
        position: object.transform.translation,
        rotation: object.transform.rotation,
    };
    if !valid_pose(pose) {
        return Err(format!("Physics object {id} has an out-of-range pose"));
    }
    Ok(pose)
}

impl Space {
    pub fn sync(&mut self, world: &EcsWorld) -> Result<RemovedColliders, String> {
        // Always update in stable object-ID order, including removal and reuse.
        self.sync_bodies(world)?;
        let removed = self.sync_colliders(world)?;
        self.sync_joints(world)?;
        Ok(removed)
    }

    fn sync_bodies(&mut self, world: &EcsWorld) -> Result<(), String> {
        let input = component_rows::<BodySpec>(world, BODY)?;
        self.body_rows.retain(|id, row| {
            if input.get(id) == Some(&(row.lifetime, row.spec)) {
                true
            } else {
                self.bodies.remove(
                    row.handle,
                    &mut self.islands,
                    &mut self.colliders,
                    &mut self.impulse_joints,
                    &mut self.multibody_joints,
                    &mut self.soft_bodies,
                    false,
                );
                false
            }
        });
        for (id, (lifetime, spec)) in input {
            let pose = object_pose(world, id)?;
            if let Some(row) = self.body_rows.get_mut(&id) {
                if pose != row.pose {
                    let body = &mut self.bodies[row.handle];
                    match spec.kind {
                        BodyKind::Kinematic => {
                            body.set_next_kinematic_position(to_rapier_pose(pose))
                        }
                        BodyKind::Fixed | BodyKind::Dynamic => {
                            body.set_position(to_rapier_pose(pose), true)
                        }
                    }
                    row.pose = pose;
                }
                continue;
            }
            if !(0.001..=10_000.0).contains(&spec.mass)
                || !(-100.0..=100.0).contains(&spec.gravity_scale)
                || !bounded(spec.linear_velocity, 10_000.0)
                || !bounded(spec.angular_velocity, 10_000.0)
            {
                return Err(format!(
                    "Physics rigid body {id} has out-of-range properties"
                ));
            }
            let builder = match spec.kind {
                BodyKind::Fixed => RigidBodyBuilder::fixed(),
                BodyKind::Dynamic => RigidBodyBuilder::dynamic(),
                BodyKind::Kinematic => RigidBodyBuilder::kinematic_position_based(),
            };
            let handle = self.bodies.insert(
                builder
                    .pose(to_rapier_pose(pose))
                    .additional_mass(spec.mass)
                    .gravity_scale(spec.gravity_scale)
                    .linvel(Vector::from_array(spec.linear_velocity))
                    .angvel(Vector::from_array(spec.angular_velocity))
                    .ccd_enabled(spec.ccd),
            );
            self.body_rows.insert(
                id,
                BodyRow {
                    handle,
                    lifetime,
                    spec,
                    pose,
                },
            );
        }
        Ok(())
    }

    fn sync_colliders(&mut self, world: &EcsWorld) -> Result<RemovedColliders, String> {
        let input = component_rows::<ColliderSpec>(world, COLLIDER)?;
        let mut removed = RemovedColliders::new();
        self.collider_rows.retain(|id, row| {
            let parent = self.body_rows.get(id).map(|body| body.handle);
            if input.get(id) == Some(&(row.lifetime, row.spec)) && row.parent == parent {
                true
            } else {
                removed.insert(row.handle, *id);
                self.colliders.remove(
                    row.handle,
                    &mut self.islands,
                    &mut self.bodies,
                    &mut self.soft_bodies,
                    true,
                );
                false
            }
        });
        for (id, (lifetime, spec)) in input {
            let pose = object_pose(world, id)?;
            let parent = self.body_rows.get(&id).map(|body| body.handle);
            let local = Pose::translation(spec.offset[0], spec.offset[1], spec.offset[2]);
            let collider_pose = if parent.is_some() {
                local
            } else {
                to_rapier_pose(pose) * local
            };
            if let Some(row) = self.collider_rows.get_mut(&id) {
                if pose != row.pose && parent.is_none() {
                    self.colliders[row.handle].set_position(collider_pose);
                    row.pose = pose;
                }
                continue;
            }
            if !bounded(spec.offset, 10_000.0)
                || !(0.0..=10_000.0).contains(&spec.density)
                || !(0.0..=16.0).contains(&spec.friction)
                || !(0.0..=1.0).contains(&spec.restitution)
            {
                return Err(format!("Physics collider {id} has out-of-range properties"));
            }
            let shape =
                shared_shape(spec.shape()).map_err(|error| format!("Collider {id}: {error}"))?;
            let collider = ColliderBuilder::new(shape)
                .position(collider_pose)
                .sensor(spec.sensor)
                .density(spec.density)
                .friction(spec.friction)
                .restitution(spec.restitution)
                .user_data((u128::from(lifetime) << 64) | u128::from(id))
                .active_collision_types(
                    ActiveCollisionTypes::default()
                        | ActiveCollisionTypes::KINEMATIC_FIXED
                        | ActiveCollisionTypes::KINEMATIC_KINEMATIC,
                )
                .active_events(ActiveEvents::COLLISION_EVENTS);
            let handle = if let Some(parent) = parent {
                self.colliders
                    .insert_with_parent(collider, parent, &mut self.bodies)
            } else {
                self.colliders.insert(collider)
            };
            self.collider_rows.insert(
                id,
                ColliderRow {
                    handle,
                    lifetime,
                    parent,
                    spec,
                    pose,
                },
            );
        }
        Ok(removed)
    }

    fn sync_joints(&mut self, world: &EcsWorld) -> Result<(), String> {
        let input = component_rows::<JointSpec>(world, JOINT)?;
        self.joint_rows.retain(|id, row| {
            let keep = input.get(id).is_some_and(|(_, spec)| {
                *spec == row.spec
                    && self
                        .body_rows
                        .get(id)
                        .is_some_and(|body| body.handle == row.owner)
                    && spec
                        .target
                        .and_then(|id| self.body_rows.get(&id))
                        .is_some_and(|body| body.handle == row.target)
            });
            if !keep {
                self.impulse_joints.remove(row.handle, true);
            }
            keep
        });
        for (id, (_, spec)) in input {
            spec.validate(id)?;
            if self.joint_rows.contains_key(&id) {
                continue;
            }
            let Some(target_id) = spec.target else {
                continue;
            };
            let owner = self
                .body_rows
                .get(&id)
                .ok_or_else(|| format!("Physics joint on {id} requires its own rigid body"))?
                .handle;
            let target = self
                .body_rows
                .get(&target_id)
                .ok_or_else(|| format!("Physics joint on {id} targets missing body {target_id}"))?
                .handle;
            let joint =
                build_joint(spec).ok_or_else(|| format!("Invalid physics joint on {id}"))?;
            let handle = self.impulse_joints.insert(owner, target, joint, true);
            self.joint_rows.insert(
                id,
                JointRow {
                    handle,
                    owner,
                    target,
                    spec,
                },
            );
        }
        Ok(())
    }

    fn write_transforms(&mut self, world: &mut EcsWorld) -> Result<(), String> {
        for (id, row) in &mut self.body_rows {
            if row.spec.kind != BodyKind::Dynamic {
                continue;
            }
            let pose = from_rapier_pose(self.bodies[row.handle].position());
            if !valid_pose(pose) {
                return Err(format!("Physics body {id} produced an out-of-range pose"));
            }
            if pose != row.pose {
                let transform = world
                    .transform_mut(*id)
                    .ok_or_else(|| format!("Missing dynamic physics object {id}"))?;
                transform.translation = pose.position;
                transform.rotation = pose.rotation;
                row.pose = pose;
            }
        }
        Ok(())
    }

    fn collision_events(
        &self,
        collisions: Collisions,
        removed: RemovedColliders,
    ) -> Result<Vec<GameEvent>, String> {
        let object = |handle| {
            self.colliders
                .get(handle)
                .map(|collider| collider.user_data as u64)
                .or_else(|| removed.get(&handle).copied())
                .ok_or_else(|| format!("Physics event referenced an unknown collider {handle:?}"))
        };
        let mut edges = collisions
            .0
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|event| {
                let a = object(event.collider1())?;
                let b = object(event.collider2())?;
                Ok((a.min(b), a.max(b), event))
            })
            .collect::<Result<Vec<_>, String>>()?;
        // Stable sorting preserves the sequence of edges for each pair.
        edges.sort_by_key(|(a, b, _)| (*a, *b));
        Ok(edges
            .into_iter()
            .map(|(a, b, event)| {
                GameEvent::new(
                    if event.started() {
                        COLLISION_STARTED
                    } else {
                        COLLISION_STOPPED
                    },
                    json!({"a": a, "b": b, "sensor": event.sensor()}),
                )
            })
            .collect())
    }
}

fn build_joint(spec: JointSpec) -> Option<GenericJoint> {
    let mut joint: GenericJoint = match spec.kind {
        JointKind::Fixed => FixedJointBuilder::new().into(),
        JointKind::Spherical => SphericalJointBuilder::new().into(),
        JointKind::Revolute => RevoluteJointBuilder::new(unit_vector(spec.axis)?).into(),
        JointKind::Prismatic => PrismaticJointBuilder::new(unit_vector(spec.axis)?).into(),
    };
    if !bounded(spec.anchor, 10_000.0) || !bounded(spec.target_anchor, 10_000.0) {
        return None;
    }
    joint.set_local_anchor1(Vector::from_array(spec.anchor));
    joint.set_local_anchor2(Vector::from_array(spec.target_anchor));
    joint.set_contacts_enabled(spec.contacts_enabled);
    Some(joint)
}

#[derive(Default)]
struct Collisions(Mutex<Vec<CollisionEvent>>);

impl EventHandler for Collisions {
    fn handle_collision_event(
        &self,
        _: &RigidBodySet,
        _: &ColliderSet,
        event: CollisionEvent,
        _: Option<&ContactPair>,
    ) {
        self.0.lock().unwrap().push(event);
    }

    fn handle_contact_force_event(
        &self,
        _: f32,
        _: &RigidBodySet,
        _: &ColliderSet,
        _: &ContactPair,
        _: f32,
    ) {
    }

    fn handle_soft_body_tear_event(&self, _: &SoftBodySet, _: &SoftBodyTearEvent) {}
}
