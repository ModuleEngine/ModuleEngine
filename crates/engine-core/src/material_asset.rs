use crate::AlphaMode;
use serde::Deserialize;

pub const MATERIAL_ASSET_EXTENSION: &str = "material";
pub const MATERIAL_ASSET_COMPONENT: &str = "surface_material";
pub const PACKAGE_TEXTURE_DIR: &str = ".textures";

/// Sparse overrides for an imported or built-in surface. Texture paths are
/// project-relative. An omitted field or null inherits the underlying value.
#[derive(Clone, Debug, Deserialize)]
pub struct MaterialAsset {
    schema_version: u32,
    pub base_color: Option<[f32; 4]>,
    pub metallic: Option<f32>,
    pub roughness: Option<f32>,
    pub normal_scale: Option<f32>,
    pub occlusion_strength: Option<f32>,
    pub emissive: Option<[f32; 3]>,
    pub alpha_mode: Option<AlphaMode>,
    pub alpha_cutoff: Option<f32>,
    pub double_sided: Option<bool>,
    pub base_color_texture: Option<String>,
    pub metallic_roughness_texture: Option<String>,
    pub normal_texture: Option<String>,
    pub occlusion_texture: Option<String>,
    pub emissive_texture: Option<String>,
}

impl MaterialAsset {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 65_536 || bytes.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{') {
            return Err("Material asset must be a JSON object of at most 64 KiB".into());
        }
        let material: Self = serde_json::from_slice(bytes).map_err(|e| format!("Material: {e}"))?;
        if material.schema_version != 1 {
            return Err("Unsupported material asset version".into());
        }
        for (field, value) in [
            ("metallic", material.metallic),
            ("roughness", material.roughness),
            ("occlusion_strength", material.occlusion_strength),
        ] {
            if value.is_some_and(|v| !(0. ..=1.).contains(&v)) {
                return Err(format!("Material {field} must be in 0..=1"));
            }
        }
        if material
            .base_color
            .into_iter()
            .flatten()
            .chain(material.emissive.into_iter().flatten())
            .any(|v| !(0. ..=1.).contains(&v))
        {
            return Err("Material color factors must be in 0..=1".into());
        }
        if material.normal_scale.is_some_and(|v| !v.is_finite())
            || material
                .alpha_cutoff
                .is_some_and(|v| !v.is_finite() || v < 0.)
        {
            return Err("Material normal scale or alpha cutoff is out of range".into());
        }
        for (field, path) in material.textures() {
            let extension = path
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase());
            if path.contains([':', '\\'])
                || path.split('/').any(|part| matches!(part, "" | "." | ".."))
                || !matches!(extension.as_deref(), Some("png" | "jpg" | "jpeg"))
            {
                return Err(format!(
                    "Material {field} requires a project-relative PNG or JPEG"
                ));
            }
        }
        Ok(material)
    }

    pub fn textures(&self) -> impl Iterator<Item = (&'static str, &str)> {
        [
            ("base_color_texture", self.base_color_texture.as_deref()),
            (
                "metallic_roughness_texture",
                self.metallic_roughness_texture.as_deref(),
            ),
            ("normal_texture", self.normal_texture.as_deref()),
            ("occlusion_texture", self.occlusion_texture.as_deref()),
            ("emissive_texture", self.emissive_texture.as_deref()),
        ]
        .into_iter()
        .filter_map(|(field, path)| Some((field, path?)))
    }
}
