use crate::{BODY, COLLIDER, JOINT, MODULE_ID};
use module_engine_core::{
    ComponentRegistry, ComponentSchema, GameEntity, PhysicsShape, PropertySchema, PropertyScope,
    PropertyType,
};
use serde::{Deserialize, Serialize};

const MAX_LENGTH: f64 = 10_000.0;
const MIN_SIZE: f64 = 0.001;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BodyKind {
    Fixed,
    #[default]
    Dynamic,
    Kinematic,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct BodySpec {
    pub kind: BodyKind,
    pub mass: f32,
    pub gravity_scale: f32,
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub ccd: bool,
}

impl Default for BodySpec {
    fn default() -> Self {
        Self {
            kind: BodyKind::Dynamic,
            mass: 1.0,
            gravity_scale: 1.0,
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            ccd: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ColliderKind {
    Ball,
    #[default]
    Box,
    CapsuleY,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ColliderSpec {
    pub kind: ColliderKind,
    pub half_extents: [f32; 3],
    pub radius: f32,
    pub half_height: f32,
    pub offset: [f32; 3],
    pub sensor: bool,
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
}

impl Default for ColliderSpec {
    fn default() -> Self {
        Self {
            kind: ColliderKind::Box,
            half_extents: [0.5; 3],
            radius: 0.5,
            half_height: 0.5,
            offset: [0.0; 3],
            sensor: false,
            density: 1.0,
            friction: 0.5,
            restitution: 0.0,
        }
    }
}

impl ColliderSpec {
    pub fn shape(self) -> PhysicsShape {
        match self.kind {
            ColliderKind::Ball => PhysicsShape::Ball {
                radius: self.radius,
            },
            ColliderKind::Box => PhysicsShape::Box {
                half_extents: self.half_extents,
            },
            ColliderKind::CapsuleY => PhysicsShape::CapsuleY {
                half_height: self.half_height,
                radius: self.radius,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JointKind {
    #[default]
    Fixed,
    Spherical,
    Revolute,
    Prismatic,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct JointSpec {
    pub kind: JointKind,
    pub target: Option<u64>,
    pub anchor: [f32; 3],
    pub target_anchor: [f32; 3],
    pub axis: [f32; 3],
    pub contacts_enabled: bool,
}

impl Default for JointSpec {
    fn default() -> Self {
        Self {
            kind: JointKind::Fixed,
            target: None,
            anchor: [0.0; 3],
            target_anchor: [0.0; 3],
            axis: [0.0, 1.0, 0.0],
            contacts_enabled: false,
        }
    }
}

impl JointSpec {
    pub fn validate(self, id: u64) -> Result<(), String> {
        if self.target == Some(id) {
            return Err(format!("Physics joint on object {id} targets itself"));
        }
        if matches!(self.kind, JointKind::Revolute | JointKind::Prismatic) && self.axis == [0.0; 3]
        {
            return Err(format!(
                "Physics joint on object {id} requires a nonzero axis"
            ));
        }
        Ok(())
    }
}

fn schema<T: Default + Serialize>(id: &str, label: &str) -> ComponentSchema {
    ComponentSchema {
        id: id.into(),
        owner_module: MODULE_ID.into(),
        version: "1".into(),
        label: label.into(),
        defaults: serde_json::to_value(T::default()).expect("physics defaults are portable"),
        fields: vec![],
        requires_modules: vec![],
        requires_components: vec![],
    }
}

fn field(schema: &mut ComponentSchema, path: &str, label: &str, value_type: PropertyType) {
    let pointer = format!("/{}", path.replace('.', "/"));
    let default = schema
        .defaults
        .pointer(&pointer)
        .expect("physics fields match the component defaults")
        .clone();
    schema.fields.push(PropertySchema {
        id: format!("{}.{path}", schema.id),
        label: label.into(),
        scope: PropertyScope::Component {
            component: schema.id.clone(),
        },
        value_type,
        default,
        requires_modules: vec![],
        requires_components: vec![],
    });
}

fn choices(values: &[&str]) -> PropertyType {
    PropertyType::Enum {
        choices: values.iter().map(|value| (*value).into()).collect(),
    }
}

fn number(schema: &mut ComponentSchema, path: &str, label: &str, min: f64, max: f64) {
    field(schema, path, label, PropertyType::Number { min, max });
}

fn vector(schema: &mut ComponentSchema, path: &str, label: &str, min: f64, max: f64) {
    for (index, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
        number(
            schema,
            &format!("{path}.{index}"),
            &format!("{label} {axis}"),
            min,
            max,
        );
    }
}

fn root(entity: &GameEntity) -> Result<(), String> {
    if entity.object.parent.is_some() {
        return Err(format!(
            "Physics object {} must be a scene root",
            entity.object.id
        ));
    }
    Ok(())
}

pub(crate) fn install(registry: &mut ComponentRegistry) -> Result<(), String> {
    let mut body = schema::<BodySpec>(BODY, "Rigid body");
    field(
        &mut body,
        "kind",
        "Body type",
        choices(&["fixed", "dynamic", "kinematic"]),
    );
    number(&mut body, "mass", "Additional mass", MIN_SIZE, MAX_LENGTH);
    number(&mut body, "gravity_scale", "Gravity scale", -100.0, 100.0);
    vector(
        &mut body,
        "linear_velocity",
        "Initial velocity",
        -MAX_LENGTH,
        MAX_LENGTH,
    );
    vector(
        &mut body,
        "angular_velocity",
        "Initial angular velocity",
        -MAX_LENGTH,
        MAX_LENGTH,
    );
    field(
        &mut body,
        "ccd",
        "Continuous collision detection",
        PropertyType::Boolean,
    );
    registry.register_with_validator(body, |entity, _| root(entity))?;

    let mut collider = schema::<ColliderSpec>(COLLIDER, "Collider");
    field(
        &mut collider,
        "kind",
        "Shape",
        choices(&["ball", "box", "capsule_y"]),
    );
    vector(
        &mut collider,
        "half_extents",
        "Half extent",
        MIN_SIZE,
        MAX_LENGTH,
    );
    number(&mut collider, "radius", "Radius", MIN_SIZE, MAX_LENGTH);
    number(
        &mut collider,
        "half_height",
        "Capsule half segment",
        MIN_SIZE,
        MAX_LENGTH,
    );
    vector(
        &mut collider,
        "offset",
        "Local offset",
        -MAX_LENGTH,
        MAX_LENGTH,
    );
    field(&mut collider, "sensor", "Sensor", PropertyType::Boolean);
    number(&mut collider, "density", "Density", 0.0, MAX_LENGTH);
    number(&mut collider, "friction", "Friction", 0.0, 16.0);
    number(&mut collider, "restitution", "Restitution", 0.0, 1.0);
    registry.register_with_validator(collider, |entity, _| root(entity))?;

    let mut joint = schema::<JointSpec>(JOINT, "Joint");
    joint.requires_components = vec![BODY.into()];
    field(
        &mut joint,
        "kind",
        "Joint type",
        choices(&["fixed", "spherical", "revolute", "prismatic"]),
    );
    field(
        &mut joint,
        "target",
        "Target body",
        PropertyType::ObjectReference {
            required_components: vec![BODY.into()],
            optional: true,
        },
    );
    vector(
        &mut joint,
        "anchor",
        "Local anchor",
        -MAX_LENGTH,
        MAX_LENGTH,
    );
    vector(
        &mut joint,
        "target_anchor",
        "Target local anchor",
        -MAX_LENGTH,
        MAX_LENGTH,
    );
    vector(&mut joint, "axis", "Local axis", -1.0, 1.0);
    field(
        &mut joint,
        "contacts_enabled",
        "Enable pair contacts",
        PropertyType::Boolean,
    );
    registry.register_with_validator(joint, |entity, _| {
        let spec = JointSpec::deserialize(&entity.components[JOINT])
            .map_err(|error| format!("Invalid physics joint on {}: {error}", entity.object.id))?;
        spec.validate(entity.object.id)
    })
}
