use crate::{COLLIDER, simulation::Space};
use module_engine_core::{
    EcsWorld, MAX_PHYSICS_OVERLAPS, PhysicsHit, PhysicsPose, PhysicsQueries, PhysicsQuery,
    PhysicsQueryError, PhysicsQueryFilter, PhysicsQueryResponse, PhysicsQueryResult, PhysicsShape,
};
use rapier3d::{math::glamx::EulerRot, parry::query::ShapeCastOptions, prelude::*};

const MAX_EXTENT: f32 = 1.0e7;

pub(crate) fn bounded(values: [f32; 3], bound: f32) -> bool {
    values.into_iter().all(|value| value.abs() <= bound)
}

pub(crate) fn valid_pose(pose: PhysicsPose) -> bool {
    bounded(pose.position, MAX_EXTENT) && bounded(pose.rotation, MAX_EXTENT)
}

pub(crate) fn to_rapier_pose(pose: PhysicsPose) -> Pose {
    let [x, y, z] = pose.rotation;
    Pose::from_parts(
        Vector::from_array(pose.position),
        Rotation::from_euler(EulerRot::ZYX, z, y, x),
    )
}

pub(crate) fn from_rapier_pose(pose: &Pose) -> PhysicsPose {
    let (z, y, x) = pose.rotation.to_euler(EulerRot::ZYX);
    PhysicsPose {
        position: pose.translation.to_array(),
        rotation: [x, y, z],
    }
}

pub(crate) fn unit_vector(values: [f32; 3]) -> Option<Vector> {
    let vector = Vector::from_array(values);
    let largest = vector.abs().max_element();
    (largest.is_finite() && largest > 0.0)
        .then(|| vector / largest)
        .and_then(Vector::try_normalize)
}

pub(crate) fn shared_shape(shape: PhysicsShape) -> Result<SharedShape, PhysicsQueryError> {
    let valid_size = |value: f32| (0.001..=10_000.0).contains(&value);
    match shape {
        PhysicsShape::Ball { radius } if valid_size(radius) => Ok(SharedShape::ball(radius)),
        PhysicsShape::Box {
            half_extents: [x, y, z],
        } if [x, y, z].into_iter().all(valid_size) => Ok(SharedShape::cuboid(x, y, z)),
        PhysicsShape::CapsuleY {
            half_height,
            radius,
        } if valid_size(half_height) && valid_size(radius) => {
            Ok(SharedShape::capsule_y(half_height, radius))
        }
        _ => Err(PhysicsQueryError::InvalidInput(
            "shape dimensions must be within 0.001–10000 world units".into(),
        )),
    }
}

fn cast_direction(values: [f32; 3], distance: f32) -> Result<Vector, PhysicsQueryError> {
    if !(0.0..=MAX_EXTENT).contains(&distance) {
        return Err(PhysicsQueryError::InvalidInput(
            "invalid cast distance".into(),
        ));
    }
    unit_vector(values).ok_or_else(|| {
        PhysicsQueryError::InvalidInput("direction must be finite and nonzero".into())
    })
}

fn checked_pose(pose: PhysicsPose) -> Result<Pose, PhysicsQueryError> {
    if !valid_pose(pose) {
        return Err(PhysicsQueryError::InvalidInput("invalid query pose".into()));
    }
    Ok(to_rapier_pose(pose))
}

impl PhysicsQueries for Space {
    fn query(&self, world: &EcsWorld, query: &PhysicsQuery) -> PhysicsQueryResponse {
        let PhysicsQueryFilter {
            exclude_object,
            include_sensors,
        } = *match query {
            PhysicsQuery::Ray { filter, .. }
            | PhysicsQuery::Cast { filter, .. }
            | PhysicsQuery::Overlap { filter, .. } => filter,
        };
        let in_current_world = |_: ColliderHandle, collider: &Collider| {
            let id = collider.user_data as u64;
            let lifetime = (collider.user_data >> 64) as u64;
            exclude_object != Some(id)
                && world.lifetime(id) == Some(lifetime)
                && world.component(id, COLLIDER).is_some()
        };
        let mut filter = QueryFilter::default().predicate(&in_current_world);
        if !include_sensors {
            filter = filter.exclude_sensors();
        }
        let pipeline = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            filter,
        );
        let hit =
            |handle: ColliderHandle, direction: Vector, origin: Vector, distance: f32| PhysicsHit {
                object: self.colliders[handle].user_data as u64,
                distance,
                position: (origin + direction * distance).to_array(),
            };
        match *query {
            PhysicsQuery::Ray {
                origin,
                direction,
                max_distance,
                ..
            } => {
                let pos = checked_pose(PhysicsPose {
                    position: origin,
                    rotation: [0.0; 3],
                })?;
                let direction = cast_direction(direction, max_distance)?;
                let ray = Ray::new(pos.translation, direction);
                Ok(PhysicsQueryResult::Hit(
                    pipeline
                        .cast_ray(&ray, max_distance, true)
                        .map(|(handle, distance)| {
                            hit(handle, direction, pos.translation, distance)
                        }),
                ))
            }
            PhysicsQuery::Cast {
                shape,
                pose,
                direction,
                max_distance,
                ..
            } => {
                let shape = shared_shape(shape)?;
                let pos = checked_pose(pose)?;
                let direction = cast_direction(direction, max_distance)?;
                let options = ShapeCastOptions::with_max_time_of_impact(max_distance);
                Ok(PhysicsQueryResult::Hit(
                    pipeline
                        .cast_shape(&pos, direction, shape.as_ref(), options)
                        .map(|(handle, result)| {
                            hit(handle, direction, pos.translation, result.time_of_impact)
                        }),
                ))
            }
            PhysicsQuery::Overlap { shape, pose, .. } => {
                let shape = shared_shape(shape)?;
                let mut objects: Vec<_> = pipeline
                    .intersect_shape(checked_pose(pose)?, shape.as_ref())
                    .take(MAX_PHYSICS_OVERLAPS + 1)
                    .map(|(_, collider)| collider.user_data as u64)
                    .collect();
                if objects.len() > MAX_PHYSICS_OVERLAPS {
                    return Err(PhysicsQueryError::TooManyHits);
                }
                objects.sort_unstable();
                Ok(PhysicsQueryResult::Overlap(objects))
            }
        }
    }
}
