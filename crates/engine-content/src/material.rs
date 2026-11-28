use crate::{ContentLibrary, owned_path};
use module_engine_core::{
    MATERIAL_ASSET_EXTENSION, Material, MaterialAsset, TextureEncoding, TextureImage,
};
use serde_json::Value;
use std::{collections::BTreeSet, fs::File, io::Read, path::Path, sync::Arc};

pub(super) struct LoadedMaterial {
    asset: MaterialAsset,
    dependencies: BTreeSet<String>,
    metallic_roughness: Option<Arc<TextureImage>>,
    normal: Option<Arc<TextureImage>>,
    occlusion: Option<Arc<TextureImage>>,
    emissive: Option<Arc<TextureImage>>,
}

impl LoadedMaterial {
    fn apply(&self, material: &mut Material) {
        let asset = &self.asset;
        if let Some(color) = asset.base_color {
            material.base_color = color;
        }
        let surface = material.pbr.get_or_insert_default();
        surface.metallic = asset.metallic.unwrap_or(surface.metallic);
        surface.roughness = asset.roughness.unwrap_or(surface.roughness);
        surface.normal_scale = asset.normal_scale.unwrap_or(surface.normal_scale);
        surface.occlusion_strength = asset
            .occlusion_strength
            .unwrap_or(surface.occlusion_strength);
        surface.emissive = asset.emissive.unwrap_or(surface.emissive);
        surface.alpha_mode = asset.alpha_mode.unwrap_or(surface.alpha_mode);
        surface.alpha_cutoff = asset.alpha_cutoff.unwrap_or(surface.alpha_cutoff);
        surface.double_sided = asset.double_sided.unwrap_or(surface.double_sided);
        if let Some(texture) = &self.metallic_roughness {
            surface.metallic_roughness_texture = Some(texture.clone());
        }
        if let Some(texture) = &self.normal {
            surface.normal_texture = Some(texture.clone());
        }
        if let Some(texture) = &self.occlusion {
            surface.occlusion_texture = Some(texture.clone());
        }
        if let Some(texture) = &self.emissive {
            surface.emissive_texture = Some(texture.clone());
        }
    }
}

impl ContentLibrary {
    fn load_material(&mut self, reference: &str) -> Result<Arc<LoadedMaterial>, String> {
        let path = self.resolve_reference(reference)?;
        if !Path::new(&path)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case(MATERIAL_ASSET_EXTENSION))
        {
            return Err(format!(
                "Expected a .{MATERIAL_ASSET_EXTENSION} asset: {path}"
            ));
        }
        if let Some(material) = self.materials.get(&path) {
            return Ok(material.clone());
        }
        let file = owned_path(&self.root, &path)?;
        let mut bytes = Vec::new();
        File::open(file)
            .and_then(|file| file.take(65_537).read_to_end(&mut bytes))
            .map_err(|e| format!("{path}: {e}"))?;
        let asset = MaterialAsset::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let mut read_texture = |reference: Option<&str>, encoding| {
            reference
                .map(|reference| self.load_material_texture(reference, encoding))
                .transpose()
        };
        let mut dependencies = BTreeSet::from([path.clone()]);
        dependencies.extend(asset.textures().map(|(_, path)| path.to_owned()));
        let material = LoadedMaterial {
            dependencies,
            metallic_roughness: read_texture(
                asset.metallic_roughness_texture.as_deref(),
                TextureEncoding::Linear,
            )?,
            normal: read_texture(asset.normal_texture.as_deref(), TextureEncoding::Normal)?,
            occlusion: read_texture(asset.occlusion_texture.as_deref(), TextureEncoding::Linear)?,
            emissive: read_texture(
                asset.emissive_texture.as_deref(),
                TextureEncoding::SrgbOpaque,
            )?,
            asset,
        };
        if let Some(reference) = material.asset.base_color_texture.as_deref() {
            self.load_texture(reference)?;
        }
        let material = Arc::new(material);
        self.materials.insert(path, material.clone());
        Ok(material)
    }

    /// Apply the project's material binding to an imported primitive or a built-in mesh.
    pub fn apply_material_components(
        &mut self,
        material: &mut Material,
        legacy: Option<&Value>,
        surface: Option<&Value>,
    ) -> Result<(), String> {
        if let Some(color) = color_override(legacy)? {
            material.base_color = color;
        }
        let previous_encoding = material.base_color_encoding();
        let asset = material_reference(surface)
            .map(|path| self.load_material(path))
            .transpose()?;
        if let Some(asset) = &asset {
            asset.apply(material);
        }
        let encoding = material.base_color_encoding();
        let reference = asset
            .as_ref()
            .and_then(|asset| asset.asset.base_color_texture.as_deref())
            .or_else(|| texture_override(legacy));
        if let Some(reference) = reference {
            material.base_color_texture = Some(self.load_material_texture(reference, encoding)?);
        } else if encoding != previous_encoding
            && let Some(texture) = &mut material.base_color_texture
        {
            // A factor-only override can change coverage on an inherited glTF map.
            // Retain the original Arc while its pointer is used as a cache key.
            let pair = self
                .inherited_color_variants
                .entry((Arc::as_ptr(texture) as usize, encoding))
                .or_insert_with(|| {
                    (
                        texture.clone(),
                        crate::texture_cache::resolve(&self.root, texture.clone(), encoding),
                    )
                });
            *texture = pair.1.clone();
        }
        Ok(())
    }

    /// Validate the same bindings used by render extraction and return all files needed for export.
    pub fn material_dependencies(
        &mut self,
        legacy: Option<&Value>,
        surface: Option<&Value>,
    ) -> Result<BTreeSet<String>, String> {
        let mut dependencies = BTreeSet::new();
        color_override(legacy)?;
        if let Some(reference) = texture_override(legacy) {
            self.load_texture(reference)?;
            dependencies.insert(self.resolve_reference(reference)?);
        }
        if let Some(reference) = material_reference(surface) {
            dependencies.extend(self.load_material(reference)?.dependencies.iter().cloned());
        }
        Ok(dependencies)
    }

    #[cfg(feature = "texture-bake")]
    pub fn bake_material_asset(&mut self, bytes: &[u8]) -> Result<(), String> {
        let asset = MaterialAsset::parse(bytes)?;
        let mut maps = Vec::new();
        for (key, reference) in asset.textures() {
            let image = self.load_texture(reference)?;
            let encodings: &[TextureEncoding] = match key {
                "base_color_texture" => match asset.alpha_mode {
                    Some(module_engine_core::AlphaMode::Opaque) => &[TextureEncoding::SrgbOpaque],
                    Some(_) => &[TextureEncoding::Srgb],
                    None => &[TextureEncoding::SrgbOpaque, TextureEncoding::Srgb],
                },
                "emissive_texture" => &[TextureEncoding::SrgbOpaque],
                "normal_texture" => &[TextureEncoding::Normal],
                _ => &[TextureEncoding::Linear],
            };
            maps.extend(encodings.iter().map(|encoding| (image.clone(), *encoding)));
        }
        crate::texture_cache::bake_author(
            &self.root,
            maps.iter()
                .map(|(image, encoding)| (image.as_ref(), *encoding)),
        )
    }

    /// Bake the exact source images already copied into this unpublished Player.
    /// An author cache hit can reuse bytes only for identical pixels and role.
    #[cfg(feature = "texture-bake")]
    pub fn bake_player_textures(
        &mut self,
        author_root: &Path,
        entities: &[module_engine_core::GameEntity],
    ) -> Result<(), String> {
        let mut maps = Vec::new();
        for entity in entities {
            let model = entity
                .components
                .get("mesh")
                .and_then(|value| value.get("path"))
                .and_then(Value::as_str)
                .map(|reference| self.load_model(reference))
                .transpose()?;
            let mut collect = |mut material| {
                self.apply_material_components(
                    &mut material,
                    entity.components.get("material"),
                    entity
                        .components
                        .get(module_engine_core::MATERIAL_ASSET_COMPONENT),
                )?;
                crate::texture_cache::for_each_map(&mut material, |image, encoding| {
                    maps.push((image.clone(), encoding));
                });
                Ok::<(), String>(())
            };
            if let Some(model) = model {
                for primitive in &model.primitives {
                    collect(primitive.material.clone())?;
                }
            } else {
                collect(Material::default())?;
            }
        }
        maps.sort_unstable_by_key(|(image, encoding)| (Arc::as_ptr(image) as usize, *encoding));
        maps.dedup_by_key(|(image, encoding)| (Arc::as_ptr(image) as usize, *encoding));
        crate::texture_cache::bake_delivery(
            author_root,
            &self.root,
            maps.iter()
                .map(|(image, encoding)| (image.as_ref(), *encoding)),
        )
    }
}

#[cfg(feature = "texture-bake")]
pub(super) fn bake_model(root: &Path, model: &crate::LoadedModel) -> Result<(), String> {
    let mut maps = Vec::new();
    for primitive in &model.primitives {
        let mut material = primitive.material.clone();
        crate::texture_cache::for_each_map(&mut material, |image, encoding| {
            maps.push((image.clone(), encoding))
        });
    }
    maps.sort_unstable_by_key(|(image, encoding)| (Arc::as_ptr(image) as usize, *encoding));
    maps.dedup_by_key(|(image, encoding)| (Arc::as_ptr(image) as usize, *encoding));
    crate::texture_cache::bake_author(
        root,
        maps.iter()
            .map(|(image, encoding)| (image.as_ref(), *encoding)),
    )
}

fn color_override(component: Option<&Value>) -> Result<Option<[f32; 4]>, String> {
    component
        .and_then(|component| component.get("base_color"))
        .map(|color| serde_json::from_value(color.clone()))
        .transpose()
        .map_err(|error| format!("Material color: {error}"))
}

fn texture_override(component: Option<&Value>) -> Option<&str> {
    component?.get("base_color_texture")?.as_str()
}

fn material_reference(component: Option<&Value>) -> Option<&str> {
    component?.get("asset")?.as_str()
}
