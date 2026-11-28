use crate::{EcsWorld, transform_point};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const LIGHT_COMPONENT: &str = "light";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LightKind {
    Directional,
    Point,
    Spot,
}

/// World-space light for a completed render snapshot. The direction follows
/// the entity's local -Z axis. Colors are linear, distance is in world units.
/// Directional intensity is illuminance (lux); local intensity is candela.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct SceneLight {
    pub id: u64,
    pub kind: LightKind,
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub inner_angle_cos: f32,
    pub outer_angle_cos: f32,
}

#[derive(Deserialize)]
pub(crate) struct LightSettings {
    kind: LightKind,
    color: [f32; 3],
    intensity: f32,
    range: f32,
    inner_angle_degrees: f32,
    outer_angle_degrees: f32,
}

impl LightSettings {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self
            .color
            .iter()
            .any(|&channel| !(0. ..=1.).contains(&channel))
            || !(0. ..=1_000_000.).contains(&self.intensity)
            || !(0.001..=10_000.).contains(&self.range)
            || !(0. ..=89.).contains(&self.inner_angle_degrees)
            || !(0. ..=89.).contains(&self.outer_angle_degrees)
            || self.outer_angle_degrees <= self.inner_angle_degrees
        {
            return Err("Light requires finite color, intensity and range and 0 ≤ inner < outer ≤ 89 degrees".into());
        }
        Ok(())
    }

    fn scene_light(self, id: u64, model: [[f32; 4]; 4]) -> SceneLight {
        let direction = [-model[0][2], -model[1][2], -model[2][2]];
        let length_sq = direction.into_iter().map(|v| v * v).sum::<f32>();
        let direction = if length_sq > 0. && length_sq.is_finite() {
            direction.map(|v| v / length_sq.sqrt())
        } else {
            [0., 0., -1.]
        };
        SceneLight {
            id,
            kind: self.kind,
            position: transform_point(model, [0.; 3]),
            direction,
            color: self.color,
            intensity: self.intensity,
            range: self.range,
            inner_angle_cos: self.inner_angle_degrees.to_radians().cos(),
            outer_angle_cos: self.outer_angle_degrees.to_radians().cos(),
        }
    }
}

impl EcsWorld {
    pub fn has_lights(&self) -> bool {
        self.may_have_component(LIGHT_COMPONENT)
    }

    pub fn is_light_only(&self, id: u64) -> bool {
        self.component(id, LIGHT_COMPONENT).is_some() && self.component(id, "mesh").is_none()
    }

    /// Extract the canonical column in stable ID order. Interpolated world
    /// matrices take precedence over the accepted pose for retained rendering.
    pub fn render_lights(&self, interpolated: &BTreeMap<u64, [[f32; 4]; 4]>) -> Vec<SceneLight> {
        let mut models = BTreeMap::new();
        self.entities_with(&[LIGHT_COMPONENT])
            .into_iter()
            .filter_map(|id| {
                let light = self
                    .decode_component::<LightSettings>(id, LIGHT_COMPONENT)
                    .ok()??;
                light.validate().ok()?;
                let model = interpolated
                    .get(&id)
                    .copied()
                    .unwrap_or_else(|| self.render_model(id, &mut models));
                Some(light.scene_light(id, model))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Transform, WorldObject, WorldProvider};
    use serde_json::json;

    #[test]
    fn accepted_light_components_follow_hierarchy_without_proxy_meshes() {
        let parent = WorldObject {
            id: 1,
            parent: None,
            color: [255; 4],
            transform: Transform {
                translation: [3., 2., 1.],
                ..Default::default()
            },
        };
        let child = WorldObject {
            id: 2,
            parent: Some(1),
            transform: Transform {
                translation: [1., 0., 0.],
                ..Default::default()
            },
            ..parent.clone()
        };
        let mut world = EcsWorld::from_objects(vec![parent, child]).unwrap();
        world
            .insert_component(
                2,
                LIGHT_COMPONENT,
                json!({"kind":"spot","color":[1.,0.5,0.],"intensity":42.,"range":6.,
                       "inner_angle_degrees":15.,"outer_angle_degrees":30.}),
            )
            .unwrap();
        let snapshot = world.snapshot();
        assert_eq!(
            snapshot
                .items
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![1]
        );
        let [light] = snapshot.lights.as_slice() else {
            panic!("expected one spotlight");
        };
        assert_eq!(light.kind, LightKind::Spot);
        assert_eq!(light.position, [4., 2., 1.]);
        assert_eq!(light.direction, [0., 0., -1.]);
        assert!(light.inner_angle_cos > light.outer_angle_cos);
        world.remove_component(2, LIGHT_COMPONENT).unwrap();
        assert!(world.snapshot().lights.is_empty());
        assert_eq!(world.snapshot().items.len(), 2);
    }
}
