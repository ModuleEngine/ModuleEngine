use crate::{ContentLibrary, owned_path};
use module_engine_core::{MATERIAL_ASSET_EXTENSION, Material, MaterialAsset, TextureImage};
use serde_json::Value;
use std::{collections::BTreeSet, fs::File, io::Read, path::Path, sync::Arc};

pub(super) struct LoadedMaterial {
    asset: MaterialAsset,
    dependencies: BTreeSet<String>,
    base_color: Option<Arc<TextureImage>>,
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
        if let Some(texture) = &self.base_color {
            material.base_color_texture = Some(texture.clone());
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
        let mut read_texture = |reference: Option<&str>| {
            reference
                .map(|reference| self.load_texture(reference))
                .transpose()
        };
        let mut dependencies = BTreeSet::from([path.clone()]);
        dependencies.extend(asset.textures().map(|(_, path)| path.to_owned()));
        let material = Arc::new(LoadedMaterial {
            dependencies,
            base_color: read_texture(asset.base_color_texture.as_deref())?,
            metallic_roughness: read_texture(asset.metallic_roughness_texture.as_deref())?,
            normal: read_texture(asset.normal_texture.as_deref())?,
            occlusion: read_texture(asset.occlusion_texture.as_deref())?,
            emissive: read_texture(asset.emissive_texture.as_deref())?,
            asset,
        });
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
        if let Some(reference) = texture_override(legacy) {
            material.base_color_texture = Some(self.load_texture(reference)?);
        }
        if let Some(reference) = material_reference(surface) {
            self.load_material(reference)?.apply(material);
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
