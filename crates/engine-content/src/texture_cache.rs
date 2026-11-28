//! Derived texture cache. The original PNG/JPEG remains the source and fallback.
use module_engine_core::{
    Material, PACKAGE_TEXTURE_DIR, TextureBlockChain, TextureEncoding, TextureImage,
    texture_block_mip_bytes, texture_mip_extents,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path, sync::Arc};

const AUTHOR_TEXTURE_DIR: &str = ".work/texture-bc";
const SIGNATURE: &[u8; 8] = b"MEBC0001";
const HEADER_BYTES: usize = SIGNATURE.len() + 32;

fn cache_name(image: &TextureImage, encoding: TextureEncoding) -> String {
    let mut digest = Sha256::new();
    let role = match encoding {
        TextureEncoding::Srgb => b"sRGB",
        TextureEncoding::SrgbOpaque => b"opaq",
        TextureEncoding::Linear => b"data",
        TextureEncoding::Normal => b"norm",
    };
    digest.update(SIGNATURE);
    digest.update(role);
    digest.update(image.width.to_le_bytes());
    digest.update(image.height.to_le_bytes());
    digest.update(&image.rgba8);
    format!("{:x}.bc", digest.finalize())
}

fn cached(
    root: &Path,
    name: &str,
    image: &TextureImage,
    encoding: TextureEncoding,
) -> Option<TextureBlockChain> {
    let expected = texture_mip_extents(image.width, image.height)
        .map(|[w, h]| texture_block_mip_bytes(w, h))
        .sum::<u64>();
    let sealed = root.join(".spirv").is_dir();
    for directory in [PACKAGE_TEXTURE_DIR, AUTHOR_TEXTURE_DIR]
        .into_iter()
        .filter(|directory| !sealed || *directory == PACKAGE_TEXTURE_DIR)
    {
        let path = root.join(directory).join(name);
        if let Ok(file) = path.canonicalize()
            && file.starts_with(root)
            && let Ok(file) = File::open(file)
            && file
                .metadata()
                .is_ok_and(|meta| meta.len() == expected + HEADER_BYTES as u64)
        {
            let mut bytes = Vec::with_capacity(expected as usize + HEADER_BYTES);
            if file
                .take(expected + HEADER_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .is_ok()
                && bytes.len() == expected as usize + HEADER_BYTES
                && bytes.starts_with(SIGNATURE)
                && Sha256::digest(&bytes[HEADER_BYTES..])[..]
                    == bytes[SIGNATURE.len()..HEADER_BYTES]
            {
                return Some(TextureBlockChain {
                    encoding,
                    bytes: bytes.split_off(HEADER_BYTES),
                });
            }
        }
    }
    None
}

pub(super) fn for_each_map(
    material: &mut Material,
    mut f: impl FnMut(&mut Arc<TextureImage>, TextureEncoding),
) {
    let encoding = material.base_color_encoding();
    if let Some(texture) = &mut material.base_color_texture {
        f(texture, encoding);
    }
    if let Some(surface) = &mut material.pbr {
        for (texture, encoding) in [
            (
                &mut surface.metallic_roughness_texture,
                TextureEncoding::Linear,
            ),
            (&mut surface.normal_texture, TextureEncoding::Normal),
            (&mut surface.occlusion_texture, TextureEncoding::Linear),
            (&mut surface.emissive_texture, TextureEncoding::SrgbOpaque),
        ] {
            if let Some(texture) = texture {
                f(texture, encoding);
            }
        }
    }
}

pub(super) fn attach<'a>(root: &Path, materials: impl IntoIterator<Item = &'a mut Material>) {
    if !root.join(PACKAGE_TEXTURE_DIR).is_dir() && !root.join(AUTHOR_TEXTURE_DIR).is_dir() {
        return;
    }
    // Keep original Arcs live while pointer identities are used as cache keys.
    let mut resolved = BTreeMap::new();
    for material in materials {
        for_each_map(material, |texture, encoding| {
            let key = (Arc::as_ptr(texture) as usize, encoding);
            let pair = resolved
                .entry(key)
                .or_insert_with(|| (texture.clone(), resolve(root, texture.clone(), encoding)));
            *texture = pair.1.clone();
        });
    }
}

pub(super) fn resolve(
    root: &Path,
    mut texture: Arc<TextureImage>,
    encoding: TextureEncoding,
) -> Arc<TextureImage> {
    if texture
        .blocks
        .as_ref()
        .is_some_and(|compressed| compressed.encoding == encoding)
    {
        return texture;
    }
    let blocks = (root.join(PACKAGE_TEXTURE_DIR).is_dir()
        || root.join(AUTHOR_TEXTURE_DIR).is_dir())
    .then(|| cached(root, &cache_name(&texture, encoding), &texture, encoding))
    .flatten();
    if texture.blocks.is_some() || blocks.is_some() {
        Arc::make_mut(&mut texture).blocks = blocks;
    }
    texture
}

#[cfg(feature = "texture-bake")]
pub(super) use bake::{bake_author, bake_delivery};

#[cfg(feature = "texture-bake")]
mod bake {
    use super::*;
    use std::{
        collections::BTreeSet,
        fs::{self, OpenOptions},
        io::Write,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    pub fn bake_author<'a>(
        root: &Path,
        maps: impl IntoIterator<Item = (&'a TextureImage, TextureEncoding)>,
    ) -> Result<(), String> {
        write_maps(root, root, AUTHOR_TEXTURE_DIR, maps)
    }

    pub fn bake_delivery<'a>(
        source: &Path,
        destination: &Path,
        maps: impl IntoIterator<Item = (&'a TextureImage, TextureEncoding)>,
    ) -> Result<(), String> {
        write_maps(source, destination, PACKAGE_TEXTURE_DIR, maps)
    }

    fn write_maps<'a>(
        source: &Path,
        destination: &Path,
        directory: &str,
        maps: impl IntoIterator<Item = (&'a TextureImage, TextureEncoding)>,
    ) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        let mut target = None;
        for (texture, encoding) in maps {
            let key = cache_name(texture, encoding);
            if !seen.insert(key.clone()) || cached(destination, &key, texture, encoding).is_some() {
                continue;
            }
            if target.is_none() {
                target = Some(owned_directory(destination, directory)?);
            }
            let blocks =
                cached(source, &key, texture, encoding).unwrap_or_else(|| TextureBlockChain {
                    encoding,
                    bytes: encode(texture, encoding),
                });
            publish(target.as_ref().unwrap(), &key, &blocks.bytes)?;
        }
        Ok(())
    }

    fn owned_directory(root: &Path, relative: &str) -> Result<PathBuf, String> {
        let mut path = root.to_path_buf();
        for part in relative.split('/') {
            path.push(part);
            match fs::create_dir(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.to_string()),
            }
            path = path.canonicalize().map_err(|error| error.to_string())?;
            if !path.starts_with(root) || !path.is_dir() {
                return Err("Texture cache directory leaves its project".into());
            }
        }
        Ok(path)
    }

    fn publish(directory: &Path, name: &str, blocks: &[u8]) -> Result<(), String> {
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let temp = directory.join(format!(".bc-{}-{id}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let written = file
            .write_all(SIGNATURE)
            .and_then(|_| file.write_all(&Sha256::digest(blocks)))
            .and_then(|_| file.write_all(blocks))
            .and_then(|_| file.sync_all());
        drop(file);
        let result = written.and_then(|_| fs::rename(&temp, directory.join(name)));
        if let Err(error) = result {
            let _ = fs::remove_file(&temp);
            return Err(format!("Texture cache: {error}"));
        }
        Ok(())
    }

    fn encode(image: &TextureImage, encoding: TextureEncoding) -> Vec<u8> {
        let mut mips = Vec::new();
        let mut pixels = std::borrow::Cow::Borrowed(image.rgba8.as_slice());
        let alpha = encoding == TextureEncoding::Srgb
            && image.rgba8.chunks_exact(4).any(|rgba| rgba[3] != 255);
        let settings = if alpha {
            intel_tex_2::bc7::alpha_fast_settings()
        } else {
            intel_tex_2::bc7::opaque_fast_settings()
        };
        for [width, height] in texture_mip_extents(image.width, image.height) {
            let padded_w = width.next_multiple_of(4);
            let padded_h = height.next_multiple_of(4);
            let channels = if encoding == TextureEncoding::Normal {
                2
            } else {
                4
            };
            let mut padded = Vec::with_capacity((padded_w * padded_h * channels) as usize);
            for y in 0..padded_h {
                let row = y.min(height - 1) * width;
                for x in 0..padded_w {
                    let start = ((row + x.min(width - 1)) * 4) as usize;
                    padded.extend_from_slice(&pixels[start..start + channels as usize]);
                    if encoding == TextureEncoding::SrgbOpaque {
                        *padded.last_mut().unwrap() = 255;
                    }
                }
            }
            let stride = padded_w * channels;
            let blocks = if encoding == TextureEncoding::Normal {
                intel_tex_2::bc5::compress_blocks(&intel_tex_2::RgSurface {
                    width: padded_w,
                    height: padded_h,
                    stride,
                    data: &padded,
                })
            } else {
                intel_tex_2::bc7::compress_blocks(
                    &settings,
                    &intel_tex_2::RgbaSurface {
                        width: padded_w,
                        height: padded_h,
                        stride,
                        data: &padded,
                    },
                )
            };
            mips.extend(blocks);
            if width != 1 || height != 1 {
                pixels = std::borrow::Cow::Owned(downsample(&pixels, width, height, encoding));
            }
        }
        mips
    }

    fn downsample(input: &[u8], width: u32, height: u32, encoding: TextureEncoding) -> Vec<u8> {
        let [out_w, out_h] = [(width / 2).max(1), (height / 2).max(1)];
        let decode: [f32; 256] = std::array::from_fn(|i| {
            let v = i as f32 / 255.;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        });
        let quantize = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
        let srgb = |v: f32| {
            if v <= 0.0031308 {
                v * 12.92
            } else {
                1.055 * v.powf(1. / 2.4) - 0.055
            }
        };
        let mut output = Vec::with_capacity((out_w * out_h * 4) as usize);
        for y in 0..out_h {
            for x in 0..out_w {
                let xs = x * width / out_w..(x + 1) * width / out_w;
                let ys = y * height / out_h..(y + 1) * height / out_h;
                let count = xs.len() * ys.len();
                let mut sum = [0.; 4];
                for row in ys {
                    for column in xs.clone() {
                        let offset = ((row * width + column) * 4) as usize;
                        let rgba = &input[offset..offset + 4];
                        let alpha = if encoding == TextureEncoding::SrgbOpaque {
                            1.
                        } else {
                            f32::from(rgba[3]) / 255.
                        };
                        let sample = match encoding {
                            TextureEncoding::Srgb | TextureEncoding::SrgbOpaque => [
                                decode[rgba[0] as usize] * alpha,
                                decode[rgba[1] as usize] * alpha,
                                decode[rgba[2] as usize] * alpha,
                                alpha,
                            ],
                            TextureEncoding::Linear => {
                                std::array::from_fn(|i| f32::from(rgba[i]) / 255.)
                            }
                            TextureEncoding::Normal => {
                                let nx = f32::from(rgba[0]) / 127.5 - 1.;
                                let ny = f32::from(rgba[1]) / 127.5 - 1.;
                                [nx, ny, (1. - nx * nx - ny * ny).max(0.).sqrt(), 1.]
                            }
                        };
                        for (sum, value) in sum.iter_mut().zip(sample) {
                            *sum += value;
                        }
                    }
                }
                let result = match encoding {
                    TextureEncoding::Srgb | TextureEncoding::SrgbOpaque => {
                        let scale = sum[3].max(f32::MIN_POSITIVE).recip();
                        [
                            srgb(sum[0] * scale),
                            srgb(sum[1] * scale),
                            srgb(sum[2] * scale),
                            sum[3] / count as f32,
                        ]
                    }
                    TextureEncoding::Linear => sum.map(|value| value / count as f32),
                    TextureEncoding::Normal => {
                        let n = glam::Vec3::new(sum[0], sum[1], sum[2])
                            .try_normalize()
                            .unwrap_or(glam::Vec3::Z);
                        [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, 1.]
                    }
                };
                output.extend(result.map(quantize));
            }
        }
        output
    }

    #[cfg(test)]
    #[test]
    fn odd_sized_block_chains_are_role_bound_and_reject_corrupt_payload() {
        let root = std::env::temp_dir().join(format!("module-bc-cache-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let image = TextureImage {
            width: 5,
            height: 7,
            rgba8: [180, 143, 240, 85].repeat(35),
            blocks: None,
        };
        let formats = [
            TextureEncoding::Srgb,
            TextureEncoding::SrgbOpaque,
            TextureEncoding::Linear,
            TextureEncoding::Normal,
        ];
        bake_author(&root, formats.map(|encoding| (&image, encoding))).unwrap();
        let source = Arc::new(image.clone());
        let mut materials = [
            module_engine_core::AlphaMode::Opaque,
            module_engine_core::AlphaMode::Blend,
        ]
        .map(|alpha_mode| Material {
            base_color_texture: Some(source.clone()),
            pbr: Some(module_engine_core::MetallicRoughness {
                alpha_mode,
                ..Default::default()
            }),
            ..Default::default()
        });
        attach(&root, &mut materials);
        let [opaque, blend] = materials.map(|material| material.base_color_texture.unwrap());
        assert_eq!(
            opaque.blocks.as_ref().unwrap().encoding,
            TextureEncoding::SrgbOpaque
        );
        assert_eq!(
            blend.blocks.as_ref().unwrap().encoding,
            TextureEncoding::Srgb
        );
        assert_ne!(
            opaque.blocks.as_ref().unwrap().bytes,
            blend.blocks.as_ref().unwrap().bytes
        );
        assert_eq!(
            resolve(&root, opaque, TextureEncoding::Srgb).blocks,
            blend.blocks
        );
        for encoding in formats {
            let file = cache_name(&image, encoding);
            let restored = resolve(&root, Arc::new(image.clone()), encoding);
            let blocks = restored.blocks.as_ref().unwrap();
            assert_eq!(blocks.encoding, encoding);
            assert_eq!(blocks.bytes.len(), 96);
            let cache_file = root.join(AUTHOR_TEXTURE_DIR).join(file);
            let mut corrupt = fs::read(&cache_file).unwrap();
            *corrupt.last_mut().unwrap() ^= 1;
            fs::write(cache_file, corrupt).unwrap();
            assert!(
                resolve(&root, Arc::new(image.clone()), encoding)
                    .blocks
                    .is_none()
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(test)]
    #[test]
    fn opaque_and_emissive_mips_ignore_source_alpha() {
        let opaque = [160, 42, 210, 255].repeat(4);
        let transparent = [160, 42, 210, 0].repeat(4);
        let make_image = |pixels| TextureImage {
            width: 2,
            height: 2,
            rgba8: pixels,
            blocks: None,
        };
        assert_eq!(
            encode(
                &make_image(transparent.clone()),
                TextureEncoding::SrgbOpaque
            ),
            encode(&make_image(opaque.clone()), TextureEncoding::SrgbOpaque),
        );
        assert_eq!(
            downsample(&transparent, 2, 2, TextureEncoding::SrgbOpaque),
            [160, 42, 210, 255]
        );
        assert_eq!(
            downsample(&opaque, 2, 2, TextureEncoding::SrgbOpaque),
            [160, 42, 210, 255]
        );
        assert_eq!(
            downsample(&transparent, 2, 2, TextureEncoding::Srgb),
            [0, 0, 0, 0]
        );
    }
}
