use crate::VulkanError;
use module_engine_core::{
    AlphaMode, Camera, Material, SceneSnapshot, TextureImage, transform_point,
};
use std::{collections::BTreeMap, sync::Arc};

pub(crate) const VERTEX_BYTES: usize = 88;
pub(super) type ResidentVertex = [u32; 7];
pub(super) const RESIDENT_VERTEX_BYTES: usize = size_of::<ResidentVertex>();

pub(super) fn pack_direction(direction: [f32; 3], sign: f32) -> u32 {
    let mut packed = 0;
    for (axis, value) in direction.into_iter().enumerate() {
        let bits = (value.clamp(-1., 1.) * 511.).round() as i32 as u32;
        packed |= (bits & 0x3ff) << (axis * 10);
    }
    packed | (u32::from(sign < 0.) << 30)
}
pub(crate) struct TextureAtlas {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub rects: BTreeMap<usize, [f32; 4]>,
    pub _textures: Vec<Arc<module_engine_core::TextureImage>>,
}

fn texture_sources(material: &Material) -> impl Iterator<Item = &Arc<TextureImage>> {
    material
        .base_color_texture
        .iter()
        .chain(material.pbr.iter().flat_map(|surface| {
            [
                &surface.metallic_roughness_texture,
                &surface.normal_texture,
                &surface.occlusion_texture,
                &surface.emissive_texture,
            ]
            .into_iter()
            .flatten()
        }))
}

pub(crate) fn texture_keys(scene: &SceneSnapshot) -> Vec<usize> {
    let mut keys = scene
        .items
        .iter()
        .flat_map(|i| texture_sources(&i.material))
        .map(|texture| Arc::as_ptr(texture) as usize)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();
    keys
}
pub(crate) fn texture_atlas(scene: &SceneSnapshot) -> Result<TextureAtlas, VulkanError> {
    let mut textures = BTreeMap::new();
    for item in &scene.items {
        for t in texture_sources(&item.material) {
            textures.entry(Arc::as_ptr(t) as usize).or_insert(t.clone());
        }
    }
    for t in textures.values() {
        let bytes = (t.width as usize)
            .checked_mul(t.height as usize)
            .and_then(|p| p.checked_mul(4));
        if t.width == 0 || t.height == 0 || bytes != Some(t.rgba8.len()) {
            return Err(VulkanError::Resource(
                "Malformed neutral texture image".into(),
            ));
        }
    }
    let mut sizes: Vec<_> = textures
        .iter()
        .map(|(&key, image)| (key, image.width, image.height))
        .collect();
    sizes.push((0, 1, 1));
    sizes.sort_unstable_by_key(|&(key, width, height)| (std::cmp::Reverse((height, width)), key));
    let minimum_width = sizes.iter().map(|&(_, w, _)| w).max().unwrap();
    const LIMIT: u32 = 8192;
    let mut best = None;
    for limit in [minimum_width, 2048, 4096, LIMIT]
        .into_iter()
        .filter(|w| (minimum_width..=LIMIT).contains(w))
    {
        let mut rects = BTreeMap::new();
        let (mut x, mut y, mut shelf, mut width) = (0, 0, 0, 0);
        for &(key, w, h) in &sizes {
            if x + w > limit {
                y += shelf;
                x = 0;
                shelf = 0;
            }
            if h > LIMIT || y + h > LIMIT {
                break;
            }
            rects.insert(key, [x as f32, y as f32, w as f32, h as f32]);
            x += w;
            shelf = shelf.max(h);
            width = width.max(x);
        }
        let height = y + shelf;
        if rects.len() == sizes.len()
            && best
                .as_ref()
                .is_none_or(|&(w, h, _)| width * height < w * h)
        {
            best = Some((width, height, rects));
        }
    }
    let (width, height, rects) = best.ok_or_else(|| {
        VulkanError::Resource(
            "Material atlas exceeds 8192 pixels; reduce imported texture sizes".into(),
        )
    })?;
    let mut rgba = vec![255; (width * height * 4) as usize];
    for (key, t) in &textures {
        let [x, y, _, _] = rects[key];
        for row in 0..t.height {
            let source = (row * t.width * 4) as usize;
            let target = (((y as u32 + row) * width + x as u32) * 4) as usize;
            rgba[target..target + t.width as usize * 4]
                .copy_from_slice(&t.rgba8[source..source + t.width as usize * 4]);
        }
    }
    Ok(TextureAtlas {
        width,
        height,
        rgba,
        rects,
        _textures: textures.into_values().collect(),
    })
}
pub(crate) fn legacy_vertices(
    scene: &SceneSnapshot,
    rects: &BTreeMap<usize, [f32; 4]>,
    view: [f32; 3],
) -> (Vec<u8>, usize, Vec<(u32, u32)>) {
    let mut bytes = Vec::with_capacity(
        scene
            .items
            .iter()
            .map(|i| i.mesh.triangles.len() * 3 * VERTEX_BYTES)
            .sum(),
    );
    let mut triangle = 0;
    let mut blended = Vec::new();
    for (object, item) in scene.items.iter().enumerate() {
        let sorted = item
            .material
            .pbr
            .as_ref()
            .is_some_and(|surface| surface.alpha_mode == AlphaMode::Blend);
        let first = triangle;
        let mut depth = [f32::INFINITY, f32::NEG_INFINITY];
        let [x, y, z] = std::array::from_fn(|axis| {
            [
                item.model[0][axis],
                item.model[1][axis],
                item.model[2][axis],
            ]
        });
        let normal_axes = [cross(y, z), cross(z, x), cross(x, y)];
        let handedness = if x
            .into_iter()
            .zip(normal_axes[0])
            .map(|(a, b)| a * b)
            .sum::<f32>()
            < 0.
        {
            -1.
        } else {
            1.
        };
        for indices in &item.mesh.triangles {
            let positions = indices.map(|index| {
                item.mesh
                    .vertices
                    .get(index)
                    .copied()
                    .map(|p| transform_point(item.model, p))
            });
            let [Some(a), Some(b), Some(c)] = positions else {
                continue;
            };
            if !a.into_iter().chain(b).chain(c).all(f32::is_finite) {
                continue;
            }
            if sorted {
                for point in [a, b, c] {
                    let z = point.into_iter().zip(view).map(|(p, f)| p * f).sum::<f32>();
                    depth[0] = depth[0].min(z);
                    depth[1] = depth[1].max(z);
                }
            }
            let face_normal = normalize(cross(sub(b, a), sub(c, a)));
            triangle += 1;
            for (vertex, p) in indices.iter().zip([a, b, c]) {
                let color = std::array::from_fn::<_, 4, _>(|c| {
                    item.color[c] as f32 / 255.0 * item.material.base_color[c]
                });
                let uv = item.mesh.texcoords.get(*vertex).copied().unwrap_or([0.; 2]);
                let normal = item
                    .mesh
                    .normals
                    .get(*vertex)
                    .map_or(face_normal, |&normal| {
                        let n =
                            combine_axes(normal_axes, normal).map(|channel| channel * handedness);
                        if n.into_iter().map(|c| c * c).sum::<f32>() > 0.000_000_000_1 {
                            normalize(n)
                        } else {
                            face_normal
                        }
                    });
                let tangent =
                    item.mesh
                        .tangents
                        .get(*vertex)
                        .map_or([0., 0., 0., 1.], |&[u, v, w, sign]| {
                            let [a, b, c] = combine_axes([x, y, z], [u, v, w]);
                            [a, b, c, sign * handedness]
                        });
                let tangent_sign = if handedness < 0. {
                    2. * tangent[3]
                } else {
                    tangent[3]
                };
                let rect = item
                    .material
                    .base_color_texture
                    .as_ref()
                    .and_then(|t| rects.get(&(Arc::as_ptr(t) as usize)))
                    .copied()
                    .unwrap_or_else(|| rects.get(&0).copied().unwrap_or([0., 0., 1., 1.]));
                let values = [
                    p[0],
                    p[1],
                    p[2],
                    normal[0],
                    normal[1],
                    normal[2],
                    color[0],
                    color[1],
                    color[2],
                    color[3],
                    triangle as f32,
                    (object + 1) as f32,
                    uv[0],
                    uv[1],
                    rect[0],
                    rect[1],
                    rect[2],
                    rect[3],
                    tangent[0],
                    tangent[1],
                    tangent[2],
                    tangent_sign,
                ];
                for v in values {
                    bytes.extend_from_slice(&v.to_ne_bytes());
                }
            }
        }
        if sorted && triangle != first {
            blended.push((
                depth[0].midpoint(depth[1]),
                (first * 3) as u32,
                ((triangle - first) * 3) as u32,
            ));
        }
    }
    blended.sort_by(|a, b| b.0.total_cmp(&a.0));
    (
        bytes,
        triangle,
        blended
            .into_iter()
            .map(|(_, first, count)| (first, count))
            .collect(),
    )
}

fn combine_axes(axes: [[f32; 3]; 3], direction: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|axis| {
        axes[0][axis] * direction[0] + axes[1][axis] * direction[1] + axes[2][axis] * direction[2]
    })
}
pub(crate) fn camera_constants(camera: Camera, width: usize, height: usize) -> [f32; 32] {
    let target = camera.target.map(|v| if v.is_finite() { v } else { 0. });
    let yaw = if camera.yaw.is_finite() {
        camera.yaw
    } else {
        0.55
    };
    let pitch = if camera.pitch.is_finite() {
        camera.pitch.clamp(-1.45, 1.45)
    } else {
        0.3
    };
    let distance = if camera.distance.is_finite() {
        camera.distance.clamp(0.2, 1000.)
    } else {
        8.
    };
    let eye = [
        target[0] + yaw.sin() * pitch.cos() * distance,
        target[1] + pitch.sin() * distance,
        target[2] + yaw.cos() * pitch.cos() * distance,
    ];
    let forward = normalize(sub(target, eye));
    let right = normalize(cross(forward, [0., 1., 0.]));
    let up = cross(right, forward);
    let mut result = [0.; 32];
    result[..4].copy_from_slice(&[eye[0], eye[1], eye[2], 0.08]);
    result[4..8].copy_from_slice(&[right[0], right[1], right[2], width as f32 / height as f32]);
    result[8..12].copy_from_slice(&[up[0], up[1], up[2], 2000.]);
    result[12..16].copy_from_slice(&[
        forward[0],
        forward[1],
        forward[2],
        1.0 / (55.0f32.to_radians() * 0.5).tan(),
    ]);
    result
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = (v.iter().map(|x| x * x).sum::<f32>()).sqrt();
    if length > 0.00001 {
        v.map(|x| x / length)
    } else {
        [0., 1., 0.]
    }
}
