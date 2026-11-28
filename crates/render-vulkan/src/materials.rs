use super::{Buffer, DeviceLifetime, Image, VulkanError, VulkanRenderer, range};
use crate::capabilities::MATERIAL_IMAGE_COUNT;
use ash::vk;
use module_engine_core::{
    AlphaMode, Material, SceneSnapshot, TextureEncoding, TextureImage, texture_block_mip_bytes,
    texture_mip_extents,
};
use std::{collections::BTreeMap, sync::Arc};

pub(super) const FIRST_MATERIAL_ROW: u32 = 1;
const SRGB: vk::Format = vk::Format::R8G8B8A8_SRGB;
const LINEAR: vk::Format = vk::Format::R8G8B8A8_UNORM;

const DESCRIPTORS: [(vk::DescriptorType, u32); 4] = [
    (vk::DescriptorType::SAMPLED_IMAGE, MATERIAL_IMAGE_COUNT),
    (vk::DescriptorType::SAMPLER, 1),
    (vk::DescriptorType::STORAGE_BUFFER, 1),
    (vk::DescriptorType::STORAGE_BUFFER, 1),
];

type ImageKey = (usize, TextureEncoding);

fn image_key(texture: &Arc<TextureImage>, encoding: TextureEncoding) -> ImageKey {
    (Arc::as_ptr(texture) as usize, encoding)
}

fn surface_maps(material: &Material) -> [(Option<&Arc<TextureImage>>, TextureEncoding); 4] {
    let surface = material.pbr.as_ref();
    [
        (
            surface.and_then(|pbr| pbr.metallic_roughness_texture.as_ref()),
            TextureEncoding::Linear,
        ),
        (
            surface.and_then(|pbr| pbr.normal_texture.as_ref()),
            TextureEncoding::Normal,
        ),
        (
            surface.and_then(|pbr| pbr.occlusion_texture.as_ref()),
            TextureEncoding::Linear,
        ),
        (
            surface.and_then(|pbr| pbr.emissive_texture.as_ref()),
            TextureEncoding::Srgb,
        ),
    ]
}

/// Mirrors the stock HLSL Surface layout. Row zero selects legacy shading.
type SurfaceRow = [u32; 16];
pub(super) type AtlasSurfaceRow = [u32; 32];

fn surface_row(material: &Material, map_ids: [u32; 4]) -> SurfaceRow {
    let mut row = [0; 16];
    if let Some(surface) = &material.pbr {
        row[..4].copy_from_slice(&map_ids);
        row[4..12].copy_from_slice(
            &[
                surface.metallic,
                surface.roughness,
                surface.normal_scale,
                surface.occlusion_strength,
                surface.emissive[0],
                surface.emissive[1],
                surface.emissive[2],
                surface.alpha_cutoff,
            ]
            .map(f32::to_bits),
        );
        row[12] = 1
            | (u32::from(surface.double_sided) << 1)
            | match surface.alpha_mode {
                AlphaMode::Opaque => 0,
                AlphaMode::Mask => 4,
                AlphaMode::Blend => 8,
            };
    }
    row
}

pub(super) fn atlas_surface_rows(
    scene: &SceneSnapshot,
    rects: &BTreeMap<usize, [f32; 4]>,
) -> Vec<AtlasSurfaceRow> {
    let mut rows = vec![[0; 32]];
    for (object, item) in scene
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.material.pbr.is_some())
    {
        rows.resize(object + 2, [0; 32]);
        let mut row = [0; 32];
        let maps = surface_maps(&item.material);
        row[..16].copy_from_slice(&surface_row(
            &item.material,
            maps.map(|(image, _)| u32::from(image.is_some())),
        ));
        for (slot, (image, _)) in maps.into_iter().enumerate() {
            if let Some(image) = image {
                let rect = rects[&(Arc::as_ptr(image) as usize)];
                let start = 16 + 4 * slot;
                row[start..start + 4].copy_from_slice(&rect.map(f32::to_bits));
            }
        }
        rows[object + 1] = row;
    }
    rows
}

pub(super) struct MaterialLayout {
    pub(super) layout: vk::DescriptorSetLayout,
    sampler: vk::Sampler,
    context: Arc<DeviceLifetime>,
}

impl MaterialLayout {
    pub(super) fn new(context: &Arc<DeviceLifetime>, anisotropy: f32) -> Result<Self, VulkanError> {
        let bindings = DESCRIPTORS.map(|(kind, count)| {
            vk::DescriptorSetLayoutBinding::default()
                .descriptor_type(kind)
                .descriptor_count(count)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        });
        let bindings = std::array::from_fn::<_, 4, _>(|i| bindings[i].binding(i as u32));
        let device = &context.device;
        // SAFETY: The bindless path checked the descriptor limits and enabled sampler anisotropy.
        // The bindings live through creation and the owner retains the sampler and layout.
        unsafe {
            let layout = device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .map_err(VulkanError::api("create material descriptor layout"))?;
            let mut result = Self {
                layout,
                sampler: vk::Sampler::null(),
                context: context.clone(),
            };
            result.sampler = device
                .create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(vk::Filter::LINEAR)
                        .min_filter(vk::Filter::LINEAR)
                        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                        .address_mode_u(vk::SamplerAddressMode::REPEAT)
                        .address_mode_v(vk::SamplerAddressMode::REPEAT)
                        .address_mode_w(vk::SamplerAddressMode::REPEAT)
                        .anisotropy_enable(true)
                        .max_anisotropy(anisotropy)
                        .max_lod(vk::LOD_CLAMP_NONE),
                    None,
                )
                .map_err(VulkanError::api("create anisotropic material sampler"))?;
            Ok(result)
        }
    }
}

impl Drop for MaterialLayout {
    fn drop(&mut self) {
        // SAFETY: Descriptor sets and in-flight users retain this layout. It alone owns the sampler
        // and set layout on the live device.
        unsafe {
            self.context.device.destroy_sampler(self.sampler, None);
            self.context
                .device
                .destroy_descriptor_set_layout(self.layout, None);
        }
    }
}

struct MaterialTexture {
    image: Image,
    source: Option<Arc<TextureImage>>,
    blocks: Option<TextureEncoding>,
}

impl MaterialTexture {
    fn extent(&self) -> [u32; 2] {
        self.source.as_ref().map_or([1, 1], |t| [t.width, t.height])
    }

    fn pixels(&self) -> &[u8] {
        self.source.as_ref().map_or(&[255; 4], |texture| {
            if self.blocks.is_some() {
                &texture.blocks.as_ref().unwrap().bytes
            } else {
                &texture.rgba8
            }
        })
    }
}

pub(super) struct MaterialSet {
    pub(super) set: vk::DescriptorSet,
    pool: vk::DescriptorPool,
    rows: Vec<ImageKey>,
    textures: BTreeMap<ImageKey, Arc<MaterialTexture>>,
    white: Arc<MaterialTexture>,
    indices: Arc<Buffer>,
    surface_rows: Vec<SurfaceRow>,
    surfaces: Arc<Buffer>,
    slots: BTreeMap<ImageKey, u32>,
    layout: Arc<MaterialLayout>,
}

impl MaterialSet {
    pub(super) fn image_count(&self) -> usize {
        self.textures.len()
    }

    pub(super) fn block_image_counts(&self) -> [usize; 2] {
        let mut count = [0; 2];
        for texture in self.textures.values() {
            if let Some(encoding) = texture.blocks {
                count[usize::from(encoding == TextureEncoding::Normal)] += 1;
            }
        }
        count
    }

    pub(super) fn index(&self, source: Option<&Arc<TextureImage>>) -> u32 {
        source.map_or(0, |image| {
            self.slots[&image_key(image, TextureEncoding::Srgb)]
        })
    }
}

impl Drop for MaterialSet {
    fn drop(&mut self) {
        // SAFETY: The fresh or retired material set owns its pool. No pending command can still use
        // its descriptors.
        unsafe {
            self.layout
                .context
                .device
                .destroy_descriptor_pool(self.pool, None);
        }
    }
}

#[derive(Default)]
pub(super) struct MaterialUploads {
    regions: Vec<(Arc<MaterialTexture>, u64)>,
    pub(super) bytes: usize,
}

impl MaterialUploads {
    fn add(&mut self, texture: Arc<MaterialTexture>) {
        if texture.blocks.is_some() {
            self.bytes = self.bytes.next_multiple_of(16);
        }
        self.regions.push((texture.clone(), self.bytes as u64));
        self.bytes += texture.pixels().len();
    }

    pub(super) fn image_count(&self) -> usize {
        self.regions.len()
    }

    pub(super) fn stage(&self, renderer: &VulkanRenderer) -> Result<Option<Buffer>, VulkanError> {
        if self.regions.is_empty() {
            return Ok(None);
        }
        let buffer = renderer.buffer(self.bytes as u64, vk::BufferUsageFlags::TRANSFER_SRC)?;
        // SAFETY: The fresh host-visible staging buffer covers the packed upload size. Every source
        // and destination range has the validated RGBA byte count and no GPU submission uses the
        // buffer yet.
        unsafe {
            buffer
                .memory
                .as_ref()
                .unwrap()
                .access(buffer.size, |mapped| {
                    for (texture, offset) in &self.regions {
                        let pixels = texture.pixels();
                        std::ptr::copy_nonoverlapping(
                            pixels.as_ptr(),
                            mapped.add(*offset as usize),
                            pixels.len(),
                        );
                    }
                })?;
        }
        Ok(Some(buffer))
    }

    pub(super) fn record(
        &self,
        device: &ash::Device,
        command: vk::CommandBuffer,
        staging: &Buffer,
    ) {
        for (texture, offset) in &self.regions {
            record_mip_upload(device, command, staging.buffer, *offset, texture);
        }
    }
}

impl VulkanRenderer {
    pub(super) fn prepare_materials(
        &self,
        scene: &SceneSnapshot,
    ) -> Result<Option<(Arc<MaterialSet>, MaterialUploads)>, VulkanError> {
        let rows: Vec<_> = scene
            .items
            .iter()
            .map(|item| {
                item.material
                    .base_color_texture
                    .as_ref()
                    .map_or((0, TextureEncoding::Srgb), |image| {
                        image_key(image, TextureEncoding::Srgb)
                    })
            })
            .collect();
        let previous = self.materials.as_ref();
        if let Some(previous) = previous.filter(|set| {
            set.rows == rows
                && set.surface_rows.len() == 1
                && scene.items.iter().all(|item| item.material.pbr.is_none())
        }) {
            return Ok(Some((previous.clone(), MaterialUploads::default())));
        }
        self.prepare_material_set(scene, previous, rows, None, &[])
    }

    pub(super) fn prepare_resident_materials(
        &self,
        scene: &SceneSnapshot,
        previous: Option<&Arc<MaterialSet>>,
        indices: Arc<Buffer>,
        order: &[usize],
    ) -> Result<Option<(Arc<MaterialSet>, MaterialUploads)>, VulkanError> {
        self.prepare_material_set(scene, previous, Vec::new(), Some(indices), order)
    }

    fn prepare_material_set(
        &self,
        scene: &SceneSnapshot,
        previous: Option<&Arc<MaterialSet>>,
        rows: Vec<ImageKey>,
        resident_indices: Option<Arc<Buffer>>,
        order: &[usize],
    ) -> Result<Option<(Arc<MaterialSet>, MaterialUploads)>, VulkanError> {
        // SAFETY: The physical device belongs to the renderer's live instance.
        let properties = unsafe { self.instance.get_physical_device_properties(self.physical) };
        let limits = properties.limits;
        let byte_count = (rows.len() as u64 + u64::from(FIRST_MATERIAL_ROW)) * 4;
        // The scene ABI carries the exact, one-based object row as a float.
        if scene.items.len() > (1 << 24) || byte_count > u64::from(limits.max_storage_buffer_range)
        {
            return Err(VulkanError::Resource(
                "Scene material index buffer exceeds device limits".into(),
            ));
        }
        let mut sources = BTreeMap::new();
        for item in &scene.items {
            for (source, encoding) in surface_maps(&item.material).into_iter().chain([(
                item.material.base_color_texture.as_ref(),
                TextureEncoding::Srgb,
            )]) {
                if let Some(source) = source {
                    sources.entry(image_key(source, encoding)).or_insert(source);
                }
            }
        }
        if sources.len() >= MATERIAL_IMAGE_COUNT as usize {
            // More tiny textures can still fit in the compatibility atlas.
            return Ok(None);
        }
        let mut uploads = MaterialUploads::default();
        let white = match previous {
            Some(previous) => previous.white.clone(),
            None => {
                let texture = Arc::new(self.material_texture(
                    None,
                    TextureEncoding::Srgb,
                    limits.max_image_dimension2_d,
                )?);
                uploads.add(texture.clone());
                texture
            }
        };
        let mut textures = BTreeMap::new();
        for (key, source) in sources {
            let texture = match previous.and_then(|set| set.textures.get(&key)) {
                Some(texture) => texture.clone(),
                None => {
                    let texture = Arc::new(self.material_texture(
                        Some(source.clone()),
                        key.1,
                        limits.max_image_dimension2_d,
                    )?);
                    uploads.add(texture.clone());
                    texture
                }
            };
            textures.insert(key, texture);
        }
        let mut slots = BTreeMap::new();
        let mut used = [false; MATERIAL_IMAGE_COUNT as usize];
        used[0] = true;
        // Keep image slots across immutable descriptor generations so a texture
        // replacement only changes rows that reference a different image.
        if let Some(previous) = previous {
            for key in textures.keys() {
                if let Some(&index) = previous.slots.get(key) {
                    slots.insert(*key, index);
                    used[index as usize] = true;
                }
            }
        }
        let mut free = used.iter().enumerate().filter(|(_, used)| !**used);
        for key in textures.keys() {
            slots
                .entry(*key)
                .or_insert_with(|| free.next().unwrap().0 as u32);
        }
        let surface_count = scene
            .items
            .iter()
            .enumerate()
            .filter_map(|(ordinal, item)| {
                item.material.pbr.as_ref()?;
                Some(1 + order.get(ordinal).copied().unwrap_or(ordinal))
            })
            .max()
            .unwrap_or(0)
            + 1;
        if surface_count as u64 * size_of::<SurfaceRow>() as u64
            > u64::from(limits.max_storage_buffer_range)
        {
            return Ok(None);
        }
        let mut surface_rows = vec![[0; 16]; surface_count];
        for (ordinal, item) in scene
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.material.pbr.is_some())
        {
            let row = 1 + order.get(ordinal).copied().unwrap_or(ordinal);
            let map_ids = surface_maps(&item.material).map(|(image, encoding)| {
                image.map_or(0, |image| slots[&image_key(image, encoding)])
            });
            surface_rows[row] = surface_row(&item.material, map_ids);
        }
        if let Some(previous) = previous.filter(|previous| {
            previous.rows == rows
                && previous.slots == slots
                && previous.surface_rows == surface_rows
                && resident_indices
                    .as_ref()
                    .is_none_or(|indices| Arc::ptr_eq(indices, &previous.indices))
        }) {
            return Ok(Some((previous.clone(), uploads)));
        }
        let surfaces =
            if let Some(previous) = previous.filter(|set| set.surface_rows == surface_rows) {
                previous.surfaces.clone()
            } else {
                Arc::new(self.write_host_storage(&surface_rows)?)
            };
        let indices = if let Some(indices) = resident_indices {
            indices
        } else {
            let indices = self.buffer(byte_count, vk::BufferUsageFlags::STORAGE_BUFFER)?;
            // SAFETY: This fresh host-visible allocation is aligned for u32
            // and sized for the reserved row plus every item. No GPU work
            // references it while it is being written.
            unsafe {
                indices
                    .memory
                    .as_ref()
                    .unwrap()
                    .access(indices.size, |mapped| {
                        let indices = mapped.cast::<u32>();
                        indices.write(0);
                        for (row, key) in rows.iter().enumerate() {
                            indices
                                .add(FIRST_MATERIAL_ROW as usize + row)
                                .write(slots.get(key).copied().unwrap_or(0));
                        }
                    })?;
            }
            Arc::new(indices)
        };
        let layout = self.material_layout.as_ref().unwrap();
        let sizes = [
            vk::DescriptorPoolSize {
                ty: DESCRIPTORS[0].0,
                descriptor_count: MATERIAL_IMAGE_COUNT,
            },
            vk::DescriptorPoolSize {
                ty: DESCRIPTORS[1].0,
                descriptor_count: 1,
            },
            vk::DescriptorPoolSize {
                ty: DESCRIPTORS[2].0,
                descriptor_count: 2,
            },
        ];
        // SAFETY: The fresh descriptor pool matches the retained layout and is unused by any
        // submission. Descriptor infos point at images, sampler and index buffer kept by the set.
        unsafe {
            let pool = self
                .device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(1)
                        .pool_sizes(&sizes),
                    None,
                )
                .map_err(VulkanError::api("create material descriptor pool"))?;
            let mut result = MaterialSet {
                set: vk::DescriptorSet::null(),
                pool,
                rows,
                textures,
                white,
                indices,
                surface_rows,
                surfaces,
                slots,
                layout: layout.clone(),
            };
            result.set = self
                .device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(pool)
                        .set_layouts(&[layout.layout]),
                )
                .map_err(VulkanError::api("allocate material descriptor set"))?[0];
            let sampled_image = |texture: &MaterialTexture| {
                vk::DescriptorImageInfo::default()
                    .image_view(texture.image.view)
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            };
            let white = sampled_image(&result.white);
            let mut image_infos = vec![white; MATERIAL_IMAGE_COUNT as usize];
            for (key, image) in &result.textures {
                image_infos[result.slots[key] as usize] = sampled_image(image);
            }
            let sampler_info = [vk::DescriptorImageInfo::default().sampler(layout.sampler)];
            let buffer_info = [vk::DescriptorBufferInfo::default()
                .buffer(result.indices.buffer)
                .range(result.indices.size)];
            let surface_info = [vk::DescriptorBufferInfo::default()
                .buffer(result.surfaces.buffer)
                .range(result.surfaces.size)];
            let writes = [
                vk::WriteDescriptorSet::default()
                    .dst_set(result.set)
                    .dst_binding(0)
                    .descriptor_type(DESCRIPTORS[0].0)
                    .image_info(&image_infos),
                vk::WriteDescriptorSet::default()
                    .dst_set(result.set)
                    .dst_binding(1)
                    .descriptor_type(DESCRIPTORS[1].0)
                    .image_info(&sampler_info),
                vk::WriteDescriptorSet::default()
                    .dst_set(result.set)
                    .dst_binding(2)
                    .descriptor_type(DESCRIPTORS[2].0)
                    .buffer_info(&buffer_info),
                vk::WriteDescriptorSet::default()
                    .dst_set(result.set)
                    .dst_binding(3)
                    .descriptor_type(DESCRIPTORS[3].0)
                    .buffer_info(&surface_info),
            ];
            self.device.update_descriptor_sets(&writes, &[]);
            Ok(Some((Arc::new(result), uploads)))
        }
    }

    pub(super) fn write_host_storage<const N: usize>(
        &self,
        rows: &[[u32; N]],
    ) -> Result<Buffer, VulkanError> {
        let bytes = std::mem::size_of_val(rows);
        let buffer = self.buffer(bytes as u64, vk::BufferUsageFlags::STORAGE_BUFFER)?;
        // SAFETY: The new mapped buffer has room for all rows and has not been submitted.
        unsafe {
            buffer
                .memory
                .as_ref()
                .unwrap()
                .access(buffer.size, |mapped| {
                    std::ptr::copy_nonoverlapping(rows.as_ptr().cast::<u8>(), mapped, bytes);
                })?;
        }
        Ok(buffer)
    }

    fn material_texture(
        &self,
        source: Option<Arc<TextureImage>>,
        encoding: TextureEncoding,
        max_dimension: u32,
    ) -> Result<MaterialTexture, VulkanError> {
        let [width, height] = source.as_ref().map_or([1, 1], |t| [t.width, t.height]);
        if let Some(image) = &source {
            let byte_count = (width as usize)
                .checked_mul(height as usize)
                .and_then(|pixels| pixels.checked_mul(4));
            if width == 0 || height == 0 || byte_count != Some(image.rgba8.len()) {
                return Err(VulkanError::Resource(
                    "Malformed neutral texture image".into(),
                ));
            }
        }
        if width > max_dimension || height > max_dimension {
            return Err(VulkanError::Resource(format!(
                "Base-color image {width}×{height} exceeds device dimension {max_dimension}"
            )));
        }
        let blocks = self
            .capabilities
            .material_block_compression
            .then_some(encoding)
            .filter(|&encoding| {
                source
                    .as_ref()
                    .and_then(|texture| texture.blocks.as_ref())
                    .is_some_and(|block_chain| {
                        block_chain.encoding == encoding
                            && block_chain.bytes.len() as u64
                                == texture_mip_extents(width, height)
                                    .map(|[w, h]| texture_block_mip_bytes(w, h))
                                    .sum::<u64>()
                    })
            });
        let format = match blocks {
            Some(TextureEncoding::Srgb) => vk::Format::BC7_SRGB_BLOCK,
            Some(TextureEncoding::Linear) => vk::Format::BC7_UNORM_BLOCK,
            Some(TextureEncoding::Normal) => vk::Format::BC5_UNORM_BLOCK,
            None if encoding == TextureEncoding::Srgb => SRGB,
            None => LINEAR,
        };
        let mut usage = vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST;
        if blocks.is_none() {
            usage |= vk::ImageUsageFlags::TRANSFER_SRC;
        }
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(mip_count(width, height))
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        let image = self.allocate_image(&info, vk::ImageAspectFlags::COLOR)?;
        Ok(MaterialTexture {
            image,
            source,
            blocks,
        })
    }
}

fn mip_count(width: u32, height: u32) -> u32 {
    u32::BITS - width.max(height).leading_zeros()
}

fn record_mip_upload(
    device: &ash::Device,
    command: vk::CommandBuffer,
    staging: vk::Buffer,
    offset: u64,
    texture: &MaterialTexture,
) {
    let image = texture.image.image;
    let [width, height] = texture.extent();
    let levels = mip_count(width, height);
    let color = vk::ImageAspectFlags::COLOR;
    let subresource = |mip_level| {
        vk::ImageSubresourceLayers::default()
            .aspect_mask(color)
            .mip_level(mip_level)
            .layer_count(1)
    };
    let all_levels = range(color).level_count(levels);
    let transfer = vk::PipelineStageFlags2::TRANSFER;
    let write = vk::AccessFlags2::TRANSFER_WRITE;
    let read = vk::AccessFlags2::TRANSFER_READ;
    let dst = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
    let src = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
    let barrier = |levels,
                   old,
                   new,
                   from: (vk::PipelineStageFlags2, vk::AccessFlags2),
                   to: (vk::PipelineStageFlags2, vk::AccessFlags2)| {
        let barriers = [vk::ImageMemoryBarrier2::default()
            .image(image)
            .subresource_range(levels)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .old_layout(old)
            .new_layout(new)
            .src_stage_mask(from.0)
            .src_access_mask(from.1)
            .dst_stage_mask(to.0)
            .dst_access_mask(to.1)];
        // SAFETY: The graphics command buffer is recording outside rendering. These subresources
        // are within the texture and the layouts match the earlier mip transitions.
        unsafe {
            device.cmd_pipeline_barrier2(
                command,
                &vk::DependencyInfo::default().image_memory_barriers(&barriers),
            )
        };
    };
    barrier(
        all_levels,
        vk::ImageLayout::UNDEFINED,
        dst,
        (vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE),
        (transfer, write),
    );
    if texture.blocks.is_some() {
        let mut cursor = offset;
        let regions = texture_mip_extents(width, height)
            .enumerate()
            .map(|(level, [width, height])| {
                let region = vk::BufferImageCopy::default()
                    .buffer_offset(cursor)
                    .image_subresource(subresource(level as u32))
                    .image_extent(vk::Extent3D {
                        width,
                        height,
                        depth: 1,
                    });
                cursor += texture_block_mip_bytes(width, height);
                region
            })
            .collect::<Vec<_>>();
        // SAFETY: The staging slice is aligned to a BC block and includes every
        // mip. All copied image subresources were transitioned to TRANSFER_DST.
        unsafe { device.cmd_copy_buffer_to_image(command, staging, image, dst, &regions) };
        barrier(
            all_levels,
            dst,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            (transfer, write),
            (
                vk::PipelineStageFlags2::FRAGMENT_SHADER,
                vk::AccessFlags2::SHADER_SAMPLED_READ,
            ),
        );
        return;
    }
    let region = vk::BufferImageCopy::default()
        .buffer_offset(offset)
        .image_subresource(subresource(0))
        .image_extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        });
    // SAFETY: The staging range contains the full RGBA base mip. The image has transfer-destination
    // usage and was transitioned to the requested layout.
    unsafe { device.cmd_copy_buffer_to_image(command, staging, image, dst, &[region]) };
    let end = |mip: u32| vk::Offset3D {
        x: (width >> mip).max(1u32) as i32,
        y: (height >> mip).max(1u32) as i32,
        z: 1,
    };
    for level in 0..levels {
        barrier(
            range(color).base_mip_level(level),
            dst,
            src,
            (transfer, write),
            (transfer, read),
        );
        if level + 1 < levels {
            let region = vk::ImageBlit::default()
                .src_subresource(subresource(level))
                .src_offsets([vk::Offset3D::default(), end(level)])
                .dst_subresource(subresource(level + 1))
                .dst_offsets([vk::Offset3D::default(), end(level + 1)]);
            // SAFETY: Linear blitting support was checked when selecting this path. Consecutive
            // mips are disjoint, fit the image, and are in their required transfer layouts.
            unsafe {
                device.cmd_blit_image(
                    command,
                    image,
                    src,
                    image,
                    dst,
                    &[region],
                    vk::Filter::LINEAR,
                )
            };
        }
    }
    barrier(
        all_levels,
        src,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        (transfer, read | write),
        (
            vk::PipelineStageFlags2::FRAGMENT_SHADER,
            vk::AccessFlags2::SHADER_SAMPLED_READ,
        ),
    );
}
