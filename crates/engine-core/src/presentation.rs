//! Renderer-neutral interpolation; gameplay storage and clocks are never modified.
use crate::{SceneSnapshot, matrix_from_rows, matrix_to_rows};
use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use std::collections::{BTreeMap, BTreeSet};

/// Emit this null-payload event in a committed tick that teleports objects.
/// Hosts snap presentation history; ordinary movement is never classified by distance.
pub const PRESENTATION_RESET_EVENT: &str = "PresentationReset";

/// Process-local identity and hierarchy used only to match adjacent presentation poses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationIdentity {
    pub lifetime: u64,
    pub parent: Option<u64>,
}

/// Blend safe affine SRT models before imported primitives are attached. Current
/// geometry, materials, colors and item ordering remain authoritative. Duplicate
/// IDs, changed lifetimes/ancestors, singular, reflected-axis changes and sheared
/// matrices snap to current. Unknown providers should not call this without a
/// proven identity map. Alpha is clamped; nonfinite alpha snaps to current.
pub fn interpolate_scene_models(
    previous: &SceneSnapshot,
    current: &SceneSnapshot,
    previous_ids: &BTreeMap<u64, PresentationIdentity>,
    current_ids: &BTreeMap<u64, PresentationIdentity>,
    alpha: f64,
) -> SceneSnapshot {
    let alpha = if alpha.is_finite() {
        alpha.clamp(0., 1.) as f32
    } else {
        1.
    };
    let mut result = current.clone();
    let mut old = BTreeMap::new();
    let mut duplicates = BTreeSet::new();
    for item in &previous.items {
        if old.insert(item.id, item).is_some() {
            duplicates.insert(item.id);
        }
    }
    let mut seen = BTreeSet::new();
    for item in &current.items {
        if !seen.insert(item.id) {
            duplicates.insert(item.id);
        }
    }
    for item in &mut result.items {
        let mut id = Some(item.id);
        let mut compatible = true;
        let mut remaining = current_ids.len() + 1;
        while let Some(entity) = id {
            remaining = remaining.saturating_sub(1);
            match (previous_ids.get(&entity), current_ids.get(&entity)) {
                (Some(a), Some(b)) if a == b && remaining > 0 => id = b.parent,
                _ => {
                    compatible = false;
                    break;
                }
            }
        }
        if compatible
            && !duplicates.contains(&item.id)
            && let Some(old) = old.get(&item.id)
            && let Some(model) = interpolate_model(old.model, item.model, alpha)
        {
            item.model = model;
        }
    }
    result
}

type Matrix = [[f32; 4]; 4];
fn decompose(rows: Matrix) -> Option<(Vec3, Quat, Vec3)> {
    let matrix = matrix_from_rows(rows);
    if !matrix.is_finite() || !matrix.row(3).abs_diff_eq(Vec4::W, 1.0e-5) {
        return None;
    }
    let columns = [
        matrix.x_axis.truncate(),
        matrix.y_axis.truncate(),
        matrix.z_axis.truncate(),
    ];
    let mut scale = Vec3::from_array(columns.map(Vec3::length));
    if !scale.is_finite() || scale.min_element() < 1.0e-7 {
        return None;
    }
    let [mut x, y, z] = [
        columns[0] / scale.x,
        columns[1] / scale.y,
        columns[2] / scale.z,
    ];
    if x.dot(y).abs() > 1.0e-4 || x.dot(z).abs() > 1.0e-4 || y.dot(z).abs() > 1.0e-4 {
        return None;
    }
    if Mat3::from_cols(x, y, z).determinant() < 0. {
        scale.x = -scale.x;
        x = -x;
    }
    let rotation = Quat::from_mat3(&Mat3::from_cols(x, y, z)).normalize();
    Some((matrix.w_axis.truncate(), rotation, scale))
}

pub fn interpolate_model(a: Matrix, b: Matrix, alpha: f32) -> Option<Matrix> {
    if a == b {
        return Some(b);
    }
    let (at, aq, as_) = decompose(a)?;
    let (bt, bq, bs) = decompose(b)?;
    if as_.x.is_sign_negative() != bs.x.is_sign_negative() {
        return None;
    }
    if alpha == 0. {
        return Some(a);
    }
    if alpha == 1. {
        return Some(b);
    }
    let result = Mat4::from_scale_rotation_translation(
        as_.lerp(bs, alpha),
        aq.slerp(bq, alpha),
        at.lerp(bt, alpha),
    );
    result.is_finite().then(|| matrix_to_rows(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Material, Mesh, RenderItem, Transform, transform_point};
    fn snapshot(transform: Transform) -> SceneSnapshot {
        SceneSnapshot {
            items: vec![RenderItem {
                id: 1,
                mesh: Mesh::cube(),
                model: transform.matrix(),
                color: [255; 4],
                material: Material::default(),
            }],
            time: 0.,
        }
    }
    fn ids() -> BTreeMap<u64, PresentationIdentity> {
        BTreeMap::from([(
            1,
            PresentationIdentity {
                lifetime: 7,
                parent: None,
            },
        )])
    }
    #[test]
    fn srt_endpoints_midpoint_and_shortest_quaternion_rotation_preserve_scale() {
        let a = snapshot(Transform {
            rotation: [0., 0., 170f32.to_radians()],
            scale: [-2., 3., 4.],
            ..Default::default()
        });
        let b = snapshot(Transform {
            translation: [4., 6., 8.],
            rotation: [0., 0., -170f32.to_radians()],
            scale: [-4., 5., 6.],
        });
        assert_eq!(
            interpolate_scene_models(&a, &b, &ids(), &ids(), 0.).items[0].model,
            a.items[0].model
        );
        assert_eq!(
            interpolate_scene_models(&a, &b, &ids(), &ids(), 1.).items[0].model,
            b.items[0].model
        );
        let middle = interpolate_scene_models(&a, &b, &ids(), &ids(), 0.5);
        let m = middle.items[0].model;
        for (row, value) in [2., 3., 4.].into_iter().enumerate() {
            assert!((m[row][3] - value).abs() < 1e-5);
        }
        let p = transform_point(m, [1., 0., 0.]);
        assert!(
            (p[0] - 5.).abs() < 1e-4,
            "Negative scale and shortest rotation pass through180deg"
        );
        assert!((p[1] - 3.).abs() < 1e-4);
    }
    #[test]
    fn changed_lifetimes_ancestors_duplicates_and_invalid_srt_snap_current() {
        let a = snapshot(Transform::default());
        let b = snapshot(Transform {
            translation: [8., 0., 0.],
            ..Default::default()
        });
        let mut changed = ids();
        changed.get_mut(&1).unwrap().lifetime = 8;
        assert_eq!(interpolate_scene_models(&a, &b, &ids(), &changed, 0.5), b);
        changed = ids();
        changed.get_mut(&1).unwrap().parent = Some(2);
        assert_eq!(interpolate_scene_models(&a, &b, &ids(), &changed, 0.5), b);
        assert_eq!(
            interpolate_scene_models(&a, &b, &BTreeMap::new(), &BTreeMap::new(), 0.5),
            b
        );
        let mut duplicate = b.clone();
        duplicate.items.push(b.items[0].clone());
        assert_eq!(
            interpolate_scene_models(&a, &duplicate, &ids(), &ids(), 0.5),
            duplicate
        );
        for model in [
            [
                [1., 0.5, 0., 8.],
                [0., 1., 0., 0.],
                [0., 0., 1., 0.],
                [0., 0., 0., 1.],
            ],
            Transform {
                scale: [0., 1., 1.],
                ..Default::default()
            }
            .matrix(),
            Transform {
                scale: [-1., 1., 1.],
                ..Default::default()
            }
            .matrix(),
        ] {
            let mut invalid = b.clone();
            invalid.items[0].model = model;
            assert_eq!(
                interpolate_scene_models(&a, &invalid, &ids(), &ids(), 0.5),
                invalid
            );
        }
        assert_eq!(
            interpolate_scene_models(&a, &b, &ids(), &ids(), f64::NAN),
            b
        );
        let mut invalid = b.clone();
        invalid.items[0].model[0][0] = f32::INFINITY;
        assert_eq!(
            interpolate_scene_models(&a, &invalid, &ids(), &ids(), 0.5),
            invalid
        );
    }
}
