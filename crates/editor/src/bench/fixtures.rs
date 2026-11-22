use super::s3_ecs::{
    self, COMPONENTS, ENTITIES as S3_ENTITIES, MOVING as S3_MOVING, STATIC as S3_STATIC,
};
use module_engine_content::ContentLibrary;
use module_engine_core::{
    Camera, EcsWorld, Material, Mesh, RenderItem, SceneSnapshot, TextureImage, Transform,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, sync::Arc};

const S3_MESHES: usize = 200;
const S3_TEXTURES: usize = 500;

pub struct Fixture {
    pub scene: SceneSnapshot,
    pub camera: Camera,
    pub inventory: Value,
    pub ecs: Option<EcsWorld>,
}

impl Fixture {
    pub fn load(id: &str, root: &Path) -> Result<Self, String> {
        match id {
            "S3" => stress(),
            "S4" => sponza(root),
            "S5" => dense(),
            _ => Err(format!("Unknown benchmark scene {id}")),
        }
    }

    pub fn use_legacy_s4_textures(&mut self) -> Result<(), String> {
        // Keep the same geometry and alpha adaptation on compatibility devices.
        let mut reduced: BTreeMap<usize, Arc<TextureImage>> = BTreeMap::new();
        for item in &mut self.scene.items {
            if let Some(texture) = &item.material.base_color_texture {
                let key = Arc::as_ptr(texture) as usize;
                let small = match reduced.entry(key) {
                    std::collections::btree_map::Entry::Occupied(entry) => entry.get().clone(),
                    std::collections::btree_map::Entry::Vacant(entry) => entry
                        .insert(Arc::new(downscale(texture, SPONZA_TEXTURE_SIDE)?))
                        .clone(),
                };
                item.material.base_color_texture = Some(small);
            }
        }
        let atlas_height = 1 + reduced.values().map(|texture| texture.height).sum::<u32>();
        if atlas_height > 8192 {
            return Err("Reduced S4 textures still exceed the compatibility atlas".into());
        }
        self.inventory["legacy_atlas_height"] = json!(atlas_height);
        self.inventory["base_color_texture_max_edge"] = json!(SPONZA_TEXTURE_SIDE);
        self.inventory["base_color_texture_reason"] =
            json!("Texture reduction for the compatibility atlas");
        Ok(())
    }

    /// Workload for the present Core storage: query eight columns, update the
    /// motion payload and Transform for 1,000 matches, then extract transforms.
    pub fn tick(&mut self, frame: usize) -> Option<(f64, f64)> {
        let world = self.ecs.as_mut()?;
        let started = std::time::Instant::now();
        let entities = s3_ecs::tick(world, frame);
        let tick_ms = started.elapsed().as_secs_f64() * 1000.;
        let started = std::time::Instant::now();
        for &id in &entities[..S3_MOVING] {
            self.scene.items[id as usize - 1].model = world.object(id).unwrap().transform.matrix();
        }
        self.scene.time = s3_ecs::time(frame);
        Some((tick_ms, started.elapsed().as_secs_f64() * 1000.))
    }
}

fn item(id: usize, mesh: Mesh, transform: Transform, material: Material) -> RenderItem {
    RenderItem {
        id: id as u64,
        mesh,
        model: transform.matrix(),
        color: [255; 4],
        material,
    }
}

fn stress() -> Result<Fixture, String> {
    let meshes: Vec<_> = (0..S3_MESHES)
        .map(|i| {
            let mut mesh = Mesh::cube();
            for p in &mut mesh.vertices {
                p[1] *= 0.55 + i as f32 / 512.;
                if p[1] > 0. {
                    p[0] *= 0.6 + i as f32 / 768.;
                    p[2] *= 0.7;
                }
            }
            mesh.texcoords = Mesh::cube()
                .vertices
                .iter()
                .map(|p| [p[0] * 0.49 + 0.5, p[1] * 0.49 + 0.5])
                .collect();
            mesh
        })
        .collect();
    let textures: Vec<_> = (0..S3_TEXTURES)
        .map(|i| {
            let rgba8 = (0..64)
                .flat_map(|pixel| {
                    let pattern = if (pixel / 8 + pixel % 8) % 2 == 0 {
                        96
                    } else {
                        176
                    };
                    [i as u8, (i >> 8) as u8 * 96 + pattern / 2, pattern, 255]
                })
                .collect();
            Arc::new(TextureImage {
                width: 8,
                height: 8,
                rgba8,
            })
        })
        .collect();
    let mut entities = Vec::new();
    let items: Vec<_> = (0..S3_STATIC + S3_MOVING)
        .map(|i| {
            let object = s3_ecs::object(i);
            let transform = object.transform;
            if i < S3_ENTITIES {
                entities.push(s3_ecs::entity(object));
            }
            item(
                i + 1,
                meshes[i % meshes.len()].clone(),
                transform,
                Material {
                    base_color_texture: Some(textures[i % textures.len()].clone()),
                    ..Default::default()
                },
            )
        })
        .collect();
    let world = EcsWorld::from_entities(entities)?;
    let moving = world
        .entity_ids()
        .filter(|&id| {
            world
                .component(id, COMPONENTS[0])
                .and_then(|v| v["moving"].as_bool())
                == Some(true)
        })
        .count();
    let components_per_entity = world
        .entity_ids()
        .filter_map(|id| world.entity(id).map(|entity| entity.components.len()))
        .collect::<std::collections::BTreeSet<_>>();
    // Count distinct loaded payloads, rather than template IDs or generator inputs.
    let mut mesh_payloads = std::collections::BTreeSet::new();
    let mut texture_payloads = std::collections::BTreeSet::new();
    for item in &items {
        mesh_payloads.insert(serde_json::to_vec(&item.mesh).map_err(|e| e.to_string())?);
        let image = item
            .material
            .base_color_texture
            .as_ref()
            .ok_or("Missing S3 texture")?;
        texture_payloads.insert((image.width, image.height, image.rgba8.as_slice()));
    }
    if items.len() != S3_STATIC + S3_MOVING
        || moving != S3_MOVING
        || mesh_payloads.len() != S3_MESHES
        || texture_payloads.len() != S3_TEXTURES
        || world.len() != S3_ENTITIES
        || components_per_entity != [8].into()
        || world.entities_with(&COMPONENTS).len() != S3_ENTITIES
    {
        return Err("Loaded S3 workload differs from its fixture contract".into());
    }
    Ok(Fixture {
        inventory: json!({
            "static_objects":items.len() - moving,"moving_objects":moving,
            "unique_meshes":mesh_payloads.len(),"unique_textures":texture_payloads.len(),
            "ecs_entities":world.len(),"custom_components_per_entity":components_per_entity,
            "texture_shape":[8,8],"atlas_layout":"legacy vertical atlas",
        }),
        scene: SceneSnapshot {
            items,
            ..Default::default()
        },
        camera: Camera {
            yaw: 0.25,
            pitch: 1.17,
            distance: 196.,
            target: [0.; 3],
        },
        ecs: Some(world),
    })
}

/// Folder for downloaded third-party benchmark models. It is ignored by Git and
/// excluded from the source digest; `scripts/bench.py` fills it from the pinned
/// manifest, and the fixture verifies every byte again before loading.
pub const SPONZA_CACHE: &str = ".tools/bench-assets/sponza";
const SPONZA_MANIFEST: &str = "examples/bench/sponza/manifest.json";
const SPONZA_TRIANGLES: usize = 262_267;
const SPONZA_PRIMITIVES: usize = 103;
/// The legacy renderer stacks base-color textures in one atlas at most 8192 px
/// high. Sponza's 25 base-color textures (up to 1024 px) and Avocado's 2048 px
/// texture fit after reducing every edge to at most 256 px.
const SPONZA_TEXTURE_SIDE: u32 = 256;

/// Verify the pinned Sponza download and return the cache folder.
pub fn verified_sponza(root: &Path) -> Result<std::path::PathBuf, String> {
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(root.join(SPONZA_MANIFEST))
            .map_err(|e| format!("Read {SPONZA_MANIFEST}: {e}"))?,
    )
    .map_err(|e| format!("Parse {SPONZA_MANIFEST}: {e}"))?;
    let cache = root.join(SPONZA_CACHE);
    let files = manifest["files"]
        .as_array()
        .ok_or("Sponza manifest has no files")?;
    for file in files {
        let name = file["path"].as_str().ok_or("Sponza manifest path")?;
        let expected = file["sha256"].as_str().ok_or("Sponza manifest hash")?;
        let bytes = std::fs::read(cache.join(name)).map_err(|e| {
            format!("Sponza file {name} is missing ({e}); run scripts/bench.py to download it")
        })?;
        if module_engine_project::hash_bytes(&bytes) != expected {
            return Err(format!(
                "Sponza file {name} differs from the pinned manifest; delete {SPONZA_CACHE} and run scripts/bench.py"
            ));
        }
    }
    Ok(cache)
}

/// Box-filter a texture so neither edge exceeds `side`. Smaller edges stay.
fn downscale(image: &TextureImage, side: u32) -> Result<TextureImage, String> {
    let (width, height) = (image.width.min(side), image.height.min(side));
    if !image.width.is_multiple_of(width) || !image.height.is_multiple_of(height) {
        return Err(format!(
            "Sponza texture {}×{} cannot be box-filtered to {width}×{height}",
            image.width, image.height
        ));
    }
    let (fx, fy) = (image.width / width, image.height / height);
    let mut rgba8 = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            for channel in 0..4 {
                let mut sum = 0u32;
                for sy in 0..fy {
                    for sx in 0..fx {
                        let index =
                            (((y * fy + sy) * image.width + x * fx + sx) * 4 + channel) as usize;
                        sum += image.rgba8[index] as u32;
                    }
                }
                rgba8.push(((sum + fx * fy / 2) / (fx * fy)) as u8);
            }
        }
    }
    Ok(TextureImage {
        width,
        height,
        rgba8,
    })
}

fn sponza(root: &Path) -> Result<Fixture, String> {
    let cache = verified_sponza(root)?;
    // The stock static importer accepts opaque materials only. Three Sponza
    // materials (leaves, chains, plant) use MASK; draw them opaque by writing a
    // derived glTF beside the verified download. Geometry and images are unchanged.
    let mut document: Value = serde_json::from_slice(
        &std::fs::read(cache.join("Sponza.gltf")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut masked = 0;
    for material in document["materials"]
        .as_array_mut()
        .ok_or("Sponza has no materials")?
    {
        if let Some(object) = material.as_object_mut()
            && object
                .remove("alphaMode")
                .is_some_and(|mode| mode != "OPAQUE")
        {
            object.remove("alphaCutoff");
            masked += 1;
        }
    }
    let derived = serde_json::to_vec(&document).map_err(|e| e.to_string())?;
    let derived_path = cache.join("Sponza.bench.gltf");
    if std::fs::read(&derived_path).ok().as_deref() != Some(derived.as_slice()) {
        std::fs::write(&derived_path, &derived)
            .map_err(|e| format!("Write {}: {e}", derived_path.display()))?;
    }
    let sponza = ContentLibrary::new(cache)?.load_model("Sponza.bench.gltf")?;
    let mut items = Vec::new();
    for primitive in &sponza.primitives {
        items.push(RenderItem {
            id: (items.len() + 1) as u64,
            mesh: primitive.mesh.clone(),
            model: primitive.transform,
            color: [255; 4],
            material: primitive.material.clone(),
        });
    }
    let sponza_triangles: usize = items.iter().map(|i| i.mesh.triangles.len()).sum();
    let sponza_primitives = items.len();
    let mut library = ContentLibrary::new(root.join("examples/bench/khronos"))?;
    let mut models = BTreeMap::new();
    for (file, scale, translation) in [
        ("Avocado.glb", 22., [-2.6, 0.45, -1.2]),
        ("BoxVertexColors.glb", 1.6, [2.6, 0.8, 1.2]),
    ] {
        let model = library.load_model(file)?;
        let objects_before = items.len();
        let matrix = Transform {
            translation,
            scale: [scale; 3],
            ..Default::default()
        }
        .matrix();
        for primitive in &model.primitives {
            items.push(RenderItem {
                id: (items.len() + 1) as u64,
                mesh: primitive.mesh.clone(),
                model: module_engine_core::multiply_matrices(matrix, primitive.transform),
                color: [255; 4],
                material: primitive.material.clone(),
            });
        }
        models.insert(file, items.len() - objects_before);
    }
    let mut images = BTreeMap::new();
    for item in &items {
        if let Some(texture) = &item.material.base_color_texture {
            images.insert(
                Arc::as_ptr(texture) as usize,
                [texture.width, texture.height],
            );
        }
    }
    let mut image_dimensions: Vec<_> = images.values().copied().collect();
    image_dimensions.sort_unstable();
    let original_atlas_height = 1 + image_dimensions.iter().map(|dims| dims[1]).sum::<u32>();
    let max_edge = image_dimensions
        .iter()
        .flatten()
        .copied()
        .max()
        .unwrap_or(0);
    // 16 × 8 grid on two storeys of the nave and side galleries.
    let points: Vec<_> = (0..256)
        .map(|i| {
            let x = (i % 16) as f32 * 1.6 - 12.;
            let z = (i / 16 % 8) as f32 * 2. - 7.;
            let y = if i < 128 { 1.6 } else { 6.4 };
            let rgb = if i % 2 == 0 {
                [1., 0.66, 0.38]
            } else {
                [0.48, 0.72, 1.]
            };
            json!({"id":i + 1,"position":[x,y,z],"linear_rgb":rgb,"intensity_cd":420,"range_m":6})
        })
        .collect();
    if sponza_triangles != SPONZA_TRIANGLES
        || sponza_primitives != SPONZA_PRIMITIVES
        || masked != 3
        || image_dimensions.len() != 26
        || points.len() != 256
        || models.len() != 2
        || models.values().any(|&count| count == 0)
    {
        return Err("Loaded S4 workload differs from its fixture contract".into());
    }
    Ok(Fixture {
        inventory: json!({
            "interior":"Khronos glTF Sample Assets Sponza at e124e8d (pinned download)",
            "sponza_triangles":sponza_triangles,"sponza_primitives":sponza_primitives,
            "base_color_textures":image_dimensions.len(),"legacy_atlas_height":null,
            "base_color_texture_max_edge":max_edge,"base_color_texture_reason":"imported dimensions",
            "original_base_color_dimensions":image_dimensions,"original_vertical_atlas_height":original_atlas_height,
            "alpha_mask_materials_drawn_opaque":masked,
            "imported_glb_primitives":models,
            "point_lights_defined":points.len(),"point_lights_rendered":null,"point_lights":points,
            "sun":{"shader_direction_toward_light":[-0.5,1.,0.7],"binding":"Forward built-in fixed directional light"},
        }),
        scene: SceneSnapshot {
            items,
            ..Default::default()
        },
        // Inside the east end of the nave, looking west along its length.
        camera: Camera {
            yaw: std::f32::consts::FRAC_PI_2,
            pitch: 0.08,
            distance: 13.,
            target: [-3., 3.2, 0.],
        },
        ecs: None,
    })
}

fn dense() -> Result<Fixture, String> {
    const SIDE: usize = 500;
    // Forty non-overlapping indexed patches of a continuous bumpy surface.
    // Keep the source at the lower end of R0.2 and generate directly in memory.
    let mut items = Vec::new();
    for tz in 0..5 {
        for tx in 0..8 {
            let mut vertices = Vec::with_capacity((SIDE + 1).pow(2));
            let mut triangles = Vec::with_capacity(2 * SIDE * SIDE);
            for z in 0..=SIDE {
                for x in 0..=SIDE {
                    let wx = (tx * SIDE + x) as f32 / 50. - 40.;
                    let wz = (tz * SIDE + z) as f32 / 50. - 25.;
                    let y = (wx * 0.19).sin() * (wz * 0.23).cos() * 2.
                        + (wx * 3.2 + wz * 5.1).sin() * 0.09;
                    vertices.push([wx, y, wz]);
                }
            }
            for z in 0..SIDE {
                for x in 0..SIDE {
                    let a = z * (SIDE + 1) + x;
                    let b = a + SIDE + 1;
                    triangles.extend([[a, b, b + 1], [a, b + 1, a + 1]]);
                }
            }
            items.push(item(
                items.len() + 1,
                Mesh {
                    vertices,
                    triangles,
                    texcoords: vec![],
                },
                Transform::default(),
                Material {
                    base_color: [0.32, 0.42, 0.29, 1.],
                    ..Default::default()
                },
            ));
        }
    }
    let triangles: usize = items.iter().map(|i| i.mesh.triangles.len()).sum();
    if !(20_000_000..=50_000_000).contains(&triangles) {
        return Err(format!(
            "Expected 20–50 million loaded S5 triangles, found {triangles}"
        ));
    }
    Ok(Fixture {
        inventory: json!({
            "patches":items.len(),
            "indexed_vertices":items.iter().map(|i| i.mesh.vertices.len()).sum::<usize>(),
            "source_triangles":triangles,
        }),
        scene: SceneSnapshot {
            items,
            ..Default::default()
        },
        camera: Camera {
            yaw: 0.45,
            pitch: 0.62,
            distance: 69.,
            target: [0.; 3],
        },
        ecs: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn bench_s3_fixture_matches_roadmap_contract() {
        let mut fixture = Fixture::load("S3", &root()).unwrap();
        assert_eq!(fixture.scene.items.len(), 11_000);
        assert_eq!(fixture.inventory["moving_objects"], 1_000);
        assert_eq!(fixture.inventory["unique_meshes"], 200);
        assert_eq!(fixture.inventory["unique_textures"], 500);
        assert_eq!(fixture.inventory["ecs_entities"], 2_000);
        let before = fixture.scene.items[0].model;
        assert!(fixture.tick(0).is_some());
        assert_ne!(fixture.scene.items[0].model, before);
        assert_eq!(fixture.scene.items[S3_MOVING].model, {
            let again = Fixture::load("S3", &root()).unwrap();
            again.scene.items[S3_MOVING].model
        });
    }

    #[test]
    fn bench_s5_fixture_has_twenty_million_triangles() {
        let fixture = Fixture::load("S5", &root()).unwrap();
        assert_eq!(fixture.inventory["source_triangles"], 20_000_000);
        assert_eq!(fixture.inventory["patches"], 40);
    }

    #[test]
    fn bench_s4_fixture_loads_pinned_sponza_when_downloaded() {
        if !root().join(SPONZA_CACHE).join("Sponza.gltf").is_file() {
            eprintln!("SKIP: Sponza is not downloaded; run scripts/bench.py");
            return;
        }
        let fixture = Fixture::load("S4", &root()).unwrap();
        assert_eq!(fixture.inventory["sponza_triangles"], SPONZA_TRIANGLES);
        assert_eq!(fixture.inventory["point_lights_defined"], 256);
        assert_eq!(fixture.inventory["alpha_mask_materials_drawn_opaque"], 3);
    }
}
