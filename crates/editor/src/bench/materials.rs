use super::{SIZE, write};
use module_engine_core::{
    Camera, Material, Mesh, RenderItem, RenderOverrides, SceneSnapshot, TextureImage, Transform,
};
use module_engine_vulkan::{RenderPath, VulkanRenderer, default_graph};
use serde_json::json;
use std::{collections::BTreeSet, path::Path, sync::Arc};

pub(super) fn check(root: &Path, folder: &Path) -> Result<(), String> {
    const COLUMNS: usize = 40;
    const ROWS: usize = 25;
    const SIDE: u32 = 16;
    const SPACING: usize = 32;
    let count = COLUMNS * ROWS;
    let camera = Camera {
        yaw: 0.,
        pitch: 0.,
        distance: 16.,
        target: [0.; 3],
    };
    let tile_world = SPACING as f32 * 2.0 * camera.distance * (55.0f32.to_radians() * 0.5).tan()
        / SIZE[1] as f32;
    let edge = tile_world * 0.46;
    let mesh = Mesh {
        vertices: vec![
            [-edge, -edge, 0.],
            [edge, -edge, 0.],
            [edge, edge, 0.],
            [-edge, edge, 0.],
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        texcoords: vec![[0., 1.], [1., 1.], [1., 0.], [0., 0.]],
        ..Mesh::default()
    };
    let linear = |byte: u8| {
        let v = f32::from(byte) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let mut scene = SceneSnapshot::default();
    let mut reference = SceneSnapshot::default();
    let mut sample_xy = Vec::with_capacity(count);
    for index in 0..count {
        let row = index / COLUMNS;
        let column = index % COLUMNS;
        let color = [
            ((index & 15) * 16 + 8) as u8,
            ((index >> 4 & 15) * 16 + 8) as u8,
            ((index >> 8) * 64 + 24) as u8,
            255,
        ];
        let transform = Transform {
            translation: [
                (column as f32 + 0.5 - COLUMNS as f32 * 0.5) * tile_world,
                (ROWS as f32 * 0.5 - row as f32 - 0.5) * tile_world,
                0.,
            ],
            ..Default::default()
        };
        let mut item = RenderItem {
            id: index as u64 + 1,
            mesh: mesh.clone(),
            model: transform.matrix(),
            color: [255; 4],
            material: Material {
                base_color: [linear(color[0]), linear(color[1]), linear(color[2]), 1.],
                ..Material::default()
            },
        };
        reference.items.push(item.clone());
        item.material = Material {
            base_color_texture: Some(Arc::new(TextureImage {
                width: SIDE,
                height: SIDE,
                rgba8: color.repeat((SIDE * SIDE) as usize),
                blocks: None,
            })),
            ..Material::default()
        };
        scene.items.push(item);
        sample_xy.push([
            (SIZE[0] - COLUMNS * SPACING) / 2 + column * SPACING + SPACING / 2,
            (SIZE[1] - ROWS * SPACING) / 2 + row * SPACING + SPACING / 2,
        ]);
    }
    let mut renderer = VulkanRenderer::new_at(root.to_owned()).map_err(|e| e.to_string())?;
    let capabilities = renderer.capabilities();
    if !capabilities.bindless_materials {
        return Err(format!(
            "{} cannot run the R2 indexed-material check",
            renderer.device_name()
        ));
    }
    let graph = default_graph(RenderPath::Forward);
    let mut render = |scene| {
        renderer
            .render(
                scene,
                camera,
                SIZE[0],
                SIZE[1],
                &graph,
                &Default::default(),
                &RenderOverrides::default(),
            )
            .map_err(|e| e.to_string())
    };
    let expected = render(&reference)?;
    let actual = render(&scene)?;
    let work = renderer
        .last_frame_work()
        .ok_or("Missing Vulkan resource counts")?;
    if work.material_images != count
        || work.material_uploaded_images != count
        || work.atlas_upload_bytes != 0
    {
        return Err(format!(
            "Unexpected texture image and upload counts: {work:?}"
        ));
    }
    let mut unique = BTreeSet::new();
    let mut max_error = 0;
    for (index, [x, y]) in sample_xy.iter().copied().enumerate() {
        let offset = (y * SIZE[0] + x) * 4;
        let pixel = &actual.rgba[offset..offset + 3];
        let reference = &expected.rgba[offset..offset + 3];
        unique.insert(pixel);
        let error = pixel
            .iter()
            .zip(reference)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        max_error = max_error.max(error);
        if error > 1 {
            return Err(format!(
                "Material {index} at ({x},{y}): {pixel:?}, expected {reference:?}"
            ));
        }
    }
    if unique.len() != count {
        return Err(format!(
            "Only {} distinct materials appeared in Vulkan pixels",
            unique.len()
        ));
    }
    crate::write_png(&folder.join("materials.png"), &actual)?;
    write(
        &folder.join("materials.json"),
        &json!({
            "adapter": renderer.device_name(),
            "capabilities": capabilities,
            "render_pixels": SIZE,
            "camera": camera,
            "texture_dimensions": [SIDE, SIDE],
            "legacy_vertical_atlas_height": 1 + SIDE as usize * count,
            "descriptor_indices_checked": [1, count],
            "pixels_checked": count,
            "distinct_rgb_values": unique.len(),
            "max_channel_error": max_error,
            "resource_work": work,
        }),
    )
}
