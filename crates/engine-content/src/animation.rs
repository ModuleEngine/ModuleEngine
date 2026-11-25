use crate::ModelPrimitive;
use glam::Quat;
use gltf::{
    accessor::{DataType, Dimensions},
    animation::{Interpolation, Property, util::ReadOutputs},
    mesh::Semantic,
    scene::Transform,
};
use module_engine_core::{identity_matrix, multiply_matrices, transform_point};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Matrix = [[f32; 4]; 4];

pub(super) fn row_major(columns: Matrix) -> Matrix {
    std::array::from_fn(|row| std::array::from_fn(|column| columns[column][row]))
}

/// The source name and duration of one glTF animation. Clips keep file order.
#[derive(Clone, Debug)]
pub struct AnimationClip {
    pub name: Option<String>,
    pub duration: f32,
    channels: Vec<Channel>,
}

#[derive(Clone, Debug)]
pub(super) struct SceneNode {
    pub parent: Option<usize>,
    pub local: Transform,
}

#[derive(Clone, Debug)]
pub(super) struct Skin {
    skeleton: Option<usize>,
    joints: Vec<usize>,
    inverse_binds: Vec<Matrix>,
}

impl Skin {
    pub fn read(
        skin: gltf::Skin<'_>,
        buffers: &[Vec<u8>],
        joint_budget: &mut usize,
    ) -> Result<Self, String> {
        let count = skin.joints().len();
        if count == 0 || count > 4096 {
            return Err("glTF skin must have between one and 4096 joints".into());
        }
        *joint_budget = joint_budget
            .checked_sub(count)
            .ok_or("glTF exceeds 32,768 skin joint bindings")?;
        let joints = skin.joints().map(|joint| joint.index()).collect::<Vec<_>>();
        if joints.iter().collect::<BTreeSet<_>>().len() != count {
            return Err("glTF skin repeats a joint".into());
        }
        let inverse_binds = if let Some(accessor) = skin.inverse_bind_matrices() {
            if accessor.data_type() != DataType::F32
                || accessor.dimensions() != Dimensions::Mat4
                || accessor.normalized()
                || accessor.count() < count
            {
                return Err("Invalid glTF inverse bind matrix accessor".into());
            }
            skin.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice))
                .read_inverse_bind_matrices()
                .ok_or("Unreadable glTF inverse bind matrices")?
                .take(count)
                .map(row_major)
                .collect()
        } else {
            vec![identity_matrix(); count]
        };
        if inverse_binds.len() != count
            || inverse_binds
                .iter()
                .any(|m| m[3] != [0., 0., 0., 1.] || !m.iter().flatten().all(|v| v.is_finite()))
        {
            return Err("glTF inverse bind matrices must be finite and affine".into());
        }
        Ok(Self {
            skeleton: skin.skeleton().map(|node| node.index()),
            joints,
            inverse_binds,
        })
    }

    fn resolve_nodes(
        &mut self,
        node_indices: &BTreeMap<usize, usize>,
        nodes: &[SceneNode],
    ) -> Result<(), String> {
        let resolve = |id: usize| {
            node_indices
                .get(&id)
                .copied()
                .ok_or_else(|| format!("glTF joint or skeleton node {id} is outside the scene"))
        };
        for joint in &mut self.joints {
            *joint = resolve(*joint)?;
        }
        let ancestors = |node| std::iter::successors(Some(node), |&i: &usize| nodes[i].parent);
        let root = match self.skeleton {
            Some(id) => resolve(id)?,
            None => ancestors(self.joints[0]).last().unwrap(),
        };
        if self
            .joints
            .iter()
            .any(|&joint| !ancestors(joint).any(|node| node == root))
        {
            return Err("glTF skin joints must share a skeleton root".into());
        }
        Ok(())
    }

    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }

    fn palette(&self, globals: &[Matrix]) -> Result<Vec<Matrix>, String> {
        self.joints
            .iter()
            .zip(&self.inverse_binds)
            .map(|(&joint, &inverse)| {
                let matrix = multiply_matrices(globals[joint], inverse);
                if !matrix.iter().flatten().all(|v| v.is_finite()) {
                    return Err("glTF skin produced a nonfinite joint matrix".into());
                }
                Ok(matrix)
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
struct Influences {
    joints: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
}

#[derive(Clone, Debug)]
pub(super) struct PrimitiveSkin {
    pub skin: usize,
    positions: Vec<[f32; 3]>,
    influences: Vec<Influences>,
}

impl PrimitiveSkin {
    pub fn read(
        skin: usize,
        joint_count: usize,
        primitive: &gltf::Primitive<'_>,
        positions: &[[f32; 3]],
        buffers: &[Vec<u8>],
        influence_budget: &mut usize,
    ) -> Result<Self, String> {
        let sets = primitive
            .attributes()
            .filter_map(|(semantic, _)| match semantic {
                Semantic::Joints(set) | Semantic::Weights(set) => Some(set),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if sets.is_empty() {
            return Err("Skinned glTF primitive requires JOINTS_0 and WEIGHTS_0".into());
        }
        *influence_budget = positions
            .len()
            .checked_mul(4)
            .and_then(|count| count.checked_mul(sets.len()))
            .and_then(|count| influence_budget.checked_sub(count))
            .ok_or("glTF exceeds eight million vertex skin influences")?;
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice));
        let mut influences = Vec::with_capacity(sets.len());
        let mut totals = vec![0.; positions.len()];
        for (expected, set) in sets.into_iter().enumerate() {
            if expected != set as usize {
                return Err(
                    "glTF skin influence sets must start at zero and be consecutive".into(),
                );
            }
            let joints = primitive
                .get(&Semantic::Joints(set))
                .ok_or_else(|| format!("Skinned glTF primitive is missing JOINTS_{set}"))?;
            let weights = primitive
                .get(&Semantic::Weights(set))
                .ok_or_else(|| format!("Skinned glTF primitive is missing WEIGHTS_{set}"))?;
            if joints.dimensions() != Dimensions::Vec4
                || !matches!(joints.data_type(), DataType::U8 | DataType::U16)
                || joints.normalized()
                || joints.count() != positions.len()
                || weights.dimensions() != Dimensions::Vec4
                || weights.count() != positions.len()
                || !match weights.data_type() {
                    DataType::F32 => !weights.normalized(),
                    DataType::U8 | DataType::U16 => weights.normalized(),
                    _ => false,
                }
            {
                return Err(format!(
                    "Invalid glTF JOINTS_{set} or WEIGHTS_{set} accessor"
                ));
            }
            let joints = reader
                .read_joints(set)
                .ok_or("Unreadable glTF vertex joint indices")?
                .into_u16()
                .collect::<Vec<_>>();
            let weights = reader
                .read_weights(set)
                .ok_or("Unreadable glTF vertex weights")?
                .into_f32()
                .collect::<Vec<_>>();
            if joints.len() != positions.len()
                || weights.len() != positions.len()
                || joints
                    .iter()
                    .flatten()
                    .any(|&joint| joint as usize >= joint_count)
                || weights.iter().flatten().any(|w| !(0. ..=1.).contains(w))
            {
                return Err("glTF vertex joint indices or weights are out of range".into());
            }
            for (total, weights) in totals.iter_mut().zip(&weights) {
                *total += weights.iter().sum::<f32>();
            }
            influences.push(Influences { joints, weights });
        }
        if totals.contains(&0.) {
            return Err("glTF skin vertex has zero total weight".into());
        }
        for set in &mut influences {
            for (weights, total) in set.weights.iter_mut().zip(&totals) {
                for weight in weights {
                    *weight /= total;
                }
            }
        }
        Ok(Self {
            skin,
            positions: positions.to_vec(),
            influences,
        })
    }

    fn deform(&self, palette: &[Matrix]) -> Result<Vec<[f32; 3]>, String> {
        self.positions
            .iter()
            .enumerate()
            .map(|(i, &position)| {
                let mut result = [0.; 3];
                for set in &self.influences {
                    for (joint, weight) in set.joints[i].into_iter().zip(set.weights[i]) {
                        if weight > 0. {
                            let point = transform_point(palette[joint as usize], position);
                            for (axis, value) in result.iter_mut().enumerate() {
                                *value += point[axis] * weight;
                            }
                        }
                    }
                }
                if !result.iter().all(|v| v.is_finite()) {
                    return Err("glTF skin produced a nonfinite vertex".into());
                }
                Ok(result)
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub(super) struct PrimitiveNode {
    pub node: usize,
    pub skin: Option<PrimitiveSkin>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Rig {
    pub nodes: Vec<SceneNode>,
    pub skins: BTreeMap<usize, Skin>,
    pub primitives: Vec<PrimitiveNode>,
    pub clips: Vec<AnimationClip>,
}

impl Rig {
    pub fn resolve(
        &mut self,
        document: &gltf::Document,
        node_indices: &BTreeMap<usize, usize>,
        buffers: &[Vec<u8>],
    ) -> Result<(), String> {
        for skin in self.skins.values_mut() {
            skin.resolve_nodes(node_indices, &self.nodes)?;
        }
        let mut value_budget = 8_000_000;
        for animation in document.animations() {
            let mut clip = AnimationClip {
                name: animation.name().map(str::to_owned),
                duration: 0.,
                channels: Vec::new(),
            };
            let mut seen = BTreeSet::new();
            for channel in animation.channels() {
                let target = channel.target();
                let node = target.node();
                let property = target.property();
                if !seen.insert((node.index(), property as u8)) {
                    return Err("glTF animation repeats a node property".into());
                }
                if matches!(node.transform(), Transform::Matrix { .. }) {
                    return Err("Animated glTF node must use TRS".into());
                }
                let resolved = node_indices.get(&node.index()).copied();
                let track = Channel::read(
                    &channel,
                    property,
                    buffers,
                    &mut value_budget,
                    resolved.unwrap_or_default(),
                )
                .map_err(|error| format!("glTF animation {}: {error}", animation.index()))?;
                clip.duration = clip.duration.max(*track.times.last().unwrap());
                if resolved.is_some() {
                    clip.channels.push(track);
                }
            }
            if seen.is_empty() {
                return Err("glTF animation has no channels".into());
            }
            self.clips.push(clip);
        }
        Ok(())
    }

    pub fn default_clip(&self) -> Option<usize> {
        self.clips.iter().position(|clip| !clip.channels.is_empty())
    }

    pub fn bake_rest(&self, primitives: &mut [ModelPrimitive]) -> Result<(), String> {
        if self.skins.is_empty() {
            return Ok(());
        }
        let globals = self.global_matrices(None)?;
        let palettes = self.palettes(&globals)?;
        for (primitive, binding) in primitives.iter_mut().zip(&self.primitives) {
            if let Some(skin) = &binding.skin {
                // Joint palettes already transform into scene space. The mesh
                // node's transform is ignored for its own skinned geometry.
                primitive.transform = identity_matrix();
                primitive.mesh.vertices = skin.deform(&palettes[&skin.skin])?;
            }
        }
        Ok(())
    }

    pub fn sample(
        &self,
        primitives: &[ModelPrimitive],
        clip: &AnimationClip,
        time: f32,
    ) -> Result<Vec<ModelPrimitive>, String> {
        if !time.is_finite() {
            return Err("glTF animation time must be finite".into());
        }
        if clip.channels.is_empty() {
            return Ok(primitives.to_vec());
        }
        let time = if clip.duration > 0. {
            time.rem_euclid(clip.duration)
        } else {
            0.
        };
        let globals = self.global_matrices(Some((clip, time)))?;
        let palettes = self.palettes(&globals)?;
        primitives
            .iter()
            .zip(&self.primitives)
            .map(|(primitive, binding)| {
                let (vertices, transform) = match &binding.skin {
                    Some(skin) => (skin.deform(&palettes[&skin.skin])?, identity_matrix()),
                    None => (primitive.mesh.vertices.clone(), globals[binding.node]),
                };
                Ok(ModelPrimitive {
                    mesh: module_engine_core::Mesh {
                        vertices,
                        triangles: primitive.mesh.triangles.clone(),
                        texcoords: primitive.mesh.texcoords.clone(),
                    },
                    material: primitive.material.clone(),
                    transform,
                })
            })
            .collect()
    }

    fn global_matrices(
        &self,
        sample: Option<(&AnimationClip, f32)>,
    ) -> Result<Vec<Matrix>, String> {
        let mut locals = self
            .nodes
            .iter()
            .map(|node| node.local.clone())
            .collect::<Vec<_>>();
        if let Some((clip, time)) = sample {
            for channel in &clip.channels {
                channel.apply(&mut locals[channel.node], time)?;
            }
        }
        let mut globals = Vec::with_capacity(locals.len());
        for (node, local) in self.nodes.iter().zip(locals) {
            let parent = node
                .parent
                .map_or_else(identity_matrix, |slot| globals[slot]);
            let matrix = multiply_matrices(parent, row_major(local.matrix()));
            if !matrix.iter().flatten().all(|value| value.is_finite()) {
                return Err("glTF animation produced a nonfinite node transform".into());
            }
            globals.push(matrix);
        }
        Ok(globals)
    }

    fn palettes(&self, globals: &[Matrix]) -> Result<BTreeMap<usize, Vec<Matrix>>, String> {
        self.skins
            .iter()
            .map(|(&id, skin)| skin.palette(globals).map(|palette| (id, palette)))
            .collect()
    }
}

#[derive(Clone, Debug)]
enum ChannelValues {
    Translation(Vec<[f32; 3]>),
    Rotation(Vec<[f32; 4]>),
    Scale(Vec<[f32; 3]>),
}

#[derive(Clone, Debug)]
struct Channel {
    node: usize,
    times: Vec<f32>,
    values: ChannelValues,
    interpolation: Interpolation,
}

impl Channel {
    fn read(
        channel: &gltf::animation::Channel<'_>,
        property: Property,
        buffers: &[Vec<u8>],
        value_budget: &mut usize,
        node: usize,
    ) -> Result<Self, String> {
        let sampler = channel.sampler();
        let input = sampler.input();
        let output = sampler.output();
        let interpolation = sampler.interpolation();
        let cubic = interpolation == Interpolation::CubicSpline;
        let step = if cubic { 3 } else { 1 };
        if input.data_type() != DataType::F32
            || input.dimensions() != Dimensions::Scalar
            || input.normalized()
            || input.count() < if cubic { 2 } else { 1 }
            || output.count() != input.count() * step
        {
            return Err("incompatible keyframe input and output accessors".into());
        }
        let valid_output = match property {
            Property::Translation | Property::Scale => {
                output.dimensions() == Dimensions::Vec3
                    && output.data_type() == DataType::F32
                    && !output.normalized()
            }
            Property::Rotation => {
                output.dimensions() == Dimensions::Vec4
                    && match output.data_type() {
                        DataType::F32 => !output.normalized(),
                        DataType::I8 | DataType::U8 | DataType::I16 | DataType::U16 => {
                            output.normalized()
                        }
                        _ => false,
                    }
            }
            Property::MorphTargetWeights => {
                return Err("glTF morph target animation is unsupported".into());
            }
        };
        if !valid_output {
            return Err("invalid keyframe output type".into());
        }
        *value_budget = value_budget
            .checked_sub(input.count() + output.count() * output.dimensions().multiplicity())
            .ok_or("glTF exceeds eight million animation sample components")?;

        let reader = channel.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice));
        let times = reader
            .read_inputs()
            .ok_or("unreadable keyframe times")?
            .collect::<Vec<_>>();
        if times.len() != input.count()
            || times.iter().any(|t| !t.is_finite() || *t < 0.)
            || times.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(
                "keyframe times must be finite and strictly increasing from zero or later".into(),
            );
        }
        let values = match reader.read_outputs().ok_or("unreadable keyframe outputs")? {
            ReadOutputs::Translations(values) => {
                ChannelValues::Translation(checked_values(values, output.count())?)
            }
            ReadOutputs::Scales(values) => {
                ChannelValues::Scale(checked_values(values, output.count())?)
            }
            ReadOutputs::Rotations(values) => {
                let values = checked_values(values.into_f32(), output.count())?;
                for &key in values.iter().skip(usize::from(cubic)).step_by(step) {
                    rotation(key)?;
                }
                ChannelValues::Rotation(values)
            }
            ReadOutputs::MorphTargetWeights(_) => {
                return Err("glTF morph target animation is unsupported".into());
            }
        };
        Ok(Self {
            node,
            times,
            values,
            interpolation,
        })
    }

    fn apply(&self, transform: &mut Transform, time: f32) -> Result<(), String> {
        let Transform::Decomposed {
            translation,
            rotation: orientation,
            scale,
        } = transform
        else {
            return Err("Animated glTF node must use TRS".into());
        };
        match &self.values {
            ChannelValues::Translation(values) => {
                *translation = self.sample(values, time, linear);
            }
            ChannelValues::Scale(values) => {
                *scale = self.sample(values, time, linear);
            }
            ChannelValues::Rotation(values) => {
                *orientation = rotation(self.sample(values, time, |a, b, t| {
                    // All stored rotation keys were validated on import.
                    Quat::from_array(a)
                        .normalize()
                        .slerp(Quat::from_array(b).normalize(), t)
                        .to_array()
                }))?
                .to_array();
            }
        }
        Ok(())
    }

    fn sample<const N: usize>(
        &self,
        values: &[[f32; N]],
        time: f32,
        lerp: impl FnOnce([f32; N], [f32; N], f32) -> [f32; N],
    ) -> [f32; N] {
        let key = |i| match self.interpolation {
            Interpolation::CubicSpline => values[3 * i + 1],
            _ => values[i],
        };
        let next = self.times.partition_point(|&t| t <= time);
        if next == 0 || next == self.times.len() {
            return key(next.saturating_sub(1));
        }
        let previous = next - 1;
        let a = key(previous);
        if time == self.times[previous] || self.interpolation == Interpolation::Step {
            return a;
        }
        let b = key(next);
        let interval = self.times[next] - self.times[previous];
        let t = (time - self.times[previous]) / interval;
        if self.interpolation == Interpolation::Linear {
            return lerp(a, b, t);
        }
        let out_tangent = values[previous * 3 + 2];
        let in_tangent = values[next * 3];
        let t = f64::from(t);
        let t2 = t * t;
        let t3 = t2 * t;
        std::array::from_fn(|axis| {
            ((2. * t3 - 3. * t2 + 1.) * f64::from(a[axis])
                + (t3 - 2. * t2 + t) * f64::from(interval) * f64::from(out_tangent[axis])
                + (-2. * t3 + 3. * t2) * f64::from(b[axis])
                + (t3 - t2) * f64::from(interval) * f64::from(in_tangent[axis])) as f32
        })
    }
}

fn checked_values<const N: usize>(
    values: impl Iterator<Item = [f32; N]>,
    count: usize,
) -> Result<Vec<[f32; N]>, String> {
    let values = values.collect::<Vec<_>>();
    if values.len() != count || values.iter().flatten().any(|v| !v.is_finite()) {
        return Err("invalid or nonfinite keyframe values".into());
    }
    Ok(values)
}

fn linear<const N: usize>(a: [f32; N], b: [f32; N], t: f32) -> [f32; N] {
    std::array::from_fn(|i| a[i] * (1. - t) + b[i] * t)
}

fn rotation(value: [f32; 4]) -> Result<Quat, String> {
    let q = Quat::from_array(value);
    let len_sq = q.length_squared();
    if !len_sq.is_finite() || len_sq <= f32::EPSILON * f32::EPSILON {
        return Err("invalid glTF animation quaternion".into());
    }
    Ok(q.normalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyframe_sampling_respects_step_cubic_tangents_and_shortest_rotation() {
        let mut channel = Channel {
            node: 0,
            times: vec![1., 3.],
            values: ChannelValues::Translation(Vec::new()),
            interpolation: Interpolation::Step,
        };
        let keys = [[1.], [3.]];
        assert_eq!(channel.sample(&keys, 0., linear), [1.]);
        assert_eq!(channel.sample(&keys, 2.99, linear), [1.]);
        assert_eq!(channel.sample(&keys, 3., linear), [3.]);

        channel.interpolation = Interpolation::CubicSpline;
        // Unit output tangent over the two-second interval. At t=0.5,
        // the Hermite out-tangent basis is 1/8 and all other terms are zero.
        let keys = [[0.], [0.], [1.], [0.], [0.], [0.]];
        assert_eq!(channel.sample(&keys, 2., linear), [0.25]);

        channel.interpolation = Interpolation::Linear;
        channel.values = ChannelValues::Rotation(vec![[0., 0., 0., 1.], [0., 0., 0., -1.]]);
        let mut local = Transform::Decomposed {
            translation: [0.; 3],
            rotation: [0., 0., 0., 1.],
            scale: [1.; 3],
        };
        channel.apply(&mut local, 2.).unwrap();
        assert_eq!(row_major(local.matrix()), identity_matrix());
    }
}
