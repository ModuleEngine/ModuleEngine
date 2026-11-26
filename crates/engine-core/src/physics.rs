//! Portable, scene-local physics queries shared by native systems and Rust workers.
use crate::{EcsWorld, ScriptContext};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PhysicsShape {
    Ball { radius: f32 },
    Box { half_extents: [f32; 3] },
    CapsuleY { half_height: f32, radius: f32 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsPose {
    pub position: [f32; 3],
    /// XYZ angles in radians, with the same Rz * Ry * Rx convention as Transform.
    pub rotation: [f32; 3],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PhysicsQueryFilter {
    pub exclude_object: Option<u64>,
    pub include_sensors: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PhysicsQuery {
    Ray {
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
        #[serde(default)]
        filter: PhysicsQueryFilter,
    },
    Cast {
        shape: PhysicsShape,
        pose: PhysicsPose,
        direction: [f32; 3],
        max_distance: f32,
        #[serde(default)]
        filter: PhysicsQueryFilter,
    },
    Overlap {
        shape: PhysicsShape,
        pose: PhysicsPose,
        #[serde(default)]
        filter: PhysicsQueryFilter,
    },
}

impl PhysicsQuery {
    pub(crate) fn validate_wire(&self) -> Result<(), PhysicsQueryError> {
        let finite = |values: &[f32]| values.iter().all(|value| value.is_finite());
        let shape_finite = |shape: &PhysicsShape| match shape {
            PhysicsShape::Ball { radius } => radius.is_finite(),
            PhysicsShape::Box { half_extents } => finite(half_extents),
            PhysicsShape::CapsuleY {
                half_height,
                radius,
            } => half_height.is_finite() && radius.is_finite(),
        };
        let pose_finite = |pose: &PhysicsPose| finite(&pose.position) && finite(&pose.rotation);
        let valid = match self {
            Self::Ray {
                origin,
                direction,
                max_distance,
                ..
            } => finite(origin) && finite(direction) && max_distance.is_finite(),
            Self::Cast {
                shape,
                pose,
                direction,
                max_distance,
                ..
            } => {
                shape_finite(shape)
                    && pose_finite(pose)
                    && finite(direction)
                    && max_distance.is_finite()
            }
            Self::Overlap { shape, pose, .. } => shape_finite(shape) && pose_finite(pose),
        };
        if !valid {
            return Err(PhysicsQueryError::InvalidInput(
                "query parameters must be finite".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicsHit {
    pub object: u64,
    /// World units along the normalized direction, zero for an initial overlap.
    pub distance: f32,
    /// Ray position or swept shape's origin at the reported distance.
    pub position: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PhysicsQueryResult {
    Hit(Option<PhysicsHit>),
    /// Unique collider object IDs in ascending order.
    Overlap(Vec<u64>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum PhysicsQueryError {
    #[error("Physics queries require a preceding physics step in this tick")]
    Unavailable,
    #[error("Invalid physics query: {0}")]
    InvalidInput(String),
    #[error("Physics overlap exceeds {MAX_PHYSICS_OVERLAPS} objects")]
    TooManyHits,
    #[error("Physics query transport: {0}")]
    Transport(String),
}

pub const MAX_PHYSICS_OVERLAPS: usize = 4096;
pub type PhysicsQueryResponse = Result<PhysicsQueryResult, PhysicsQueryError>;

/// A read-only view produced by the explicitly scheduled physics system. The
/// current ECS view lets it discard removed or replaced collider incarnations.
pub trait PhysicsQueries: Send + Sync {
    fn query(&self, world: &EcsWorld, query: &PhysicsQuery) -> PhysicsQueryResponse;
}

type ScriptQueries = Box<dyn FnMut(&PhysicsQuery) -> PhysicsQueryResponse>;
thread_local! {
    static SCRIPT_PHYSICS: RefCell<Option<ScriptQueries>> = const { RefCell::new(None) };
}

struct ScriptQueryScope(Option<ScriptQueries>);

impl Drop for ScriptQueryScope {
    fn drop(&mut self) {
        SCRIPT_PHYSICS.replace(self.0.take());
    }
}

/// Installs transport for one generated worker invocation on its main thread.
#[doc(hidden)]
pub fn with_script_physics_queries<T>(
    query: impl FnMut(&PhysicsQuery) -> PhysicsQueryResponse + 'static,
    invoke: impl FnOnce() -> T,
) -> T {
    let _scope = ScriptQueryScope(SCRIPT_PHYSICS.replace(Some(Box::new(query))));
    invoke()
}

impl ScriptContext {
    pub fn physics_queries_available(&self) -> bool {
        SCRIPT_PHYSICS.with_borrow(Option::is_some)
    }

    pub fn query_physics(&self, query: &PhysicsQuery) -> PhysicsQueryResponse {
        SCRIPT_PHYSICS.with_borrow_mut(|source| {
            let source = source.as_mut().ok_or(PhysicsQueryError::Unavailable)?;
            query.validate_wire()?;
            source(query)
        })
    }
}
