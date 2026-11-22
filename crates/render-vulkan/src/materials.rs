use super::{Buffer, DeviceLifetime, Image, VulkanError, VulkanRenderer, range};
use crate::capabilities::MATERIAL_IMAGE_COUNT;
use ash::vk;
use module_engine_core::{SceneSnapshot, TextureImage};
use std::{collections::BTreeMap, sync::Arc};

pub(super) const FIRST_MATERIAL_ROW: u32 = 1;
const FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;

const DESCRIPTORS: [(vk::DescriptorType, u32); 3] = [
    (vk::DescriptorType::SAMPLED_IMAGE, MATERIAL_IMAGE_COUNT),
    (vk::DescriptorType::SAMPLER, 1),
    (vk::DescriptorType::STORAGE_BUFFER, 1),
];

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
        let bindings = std::array::from_fn::<_, 3, _>(|i| bindings[i].binding(i as u32));
        let device = &context.device;
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
}

impl MaterialTexture {
    fn extent(&self) -> [u32; 2] {
        self.source.as_ref().map_or([1, 1], |t| [t.width, t.height])
    }

    fn pixels(&self) -> &[u8] {
        self.source
            .as_ref()
            .map_or(&[255; 4], |t| t.rgba8.as_slice())
    }
}

pub(super) struct MaterialSet {
    pub(super) set: vk::DescriptorSet,
    pool: vk::DescriptorPool,
    rows: Vec<usize>,
    textures: BTreeMap<usize, Arc<MaterialTexture>>,
    white: Arc<MaterialTexture>,
    _indices: Buffer,
    layout: Arc<MaterialLayout>,
}

impl MaterialSet {
    pub(super) fn image_count(&self) -> usize {
        self.textures.len()
    }
}

impl Drop for MaterialSet {
    fn drop(&mut self) {
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
        layout: &Arc<MaterialLayout>,
    ) -> Result<Option<(Arc<MaterialSet>, MaterialUploads)>, VulkanError> {
        let image_key = |image: &Arc<TextureImage>| Arc::as_ptr(image) as usize;
        let rows: Vec<_> = scene
            .items
            .iter()
            .map(|item| {
                item.material
                    .base_color_texture
                    .as_ref()
                    .map_or(0, image_key)
            })
            .collect();
        let previous = self.materials.as_ref();
        if let Some(previous) = previous.filter(|set| set.rows == rows) {
            return Ok(Some((previous.clone(), MaterialUploads::default())));
        }
        let properties = unsafe { self.instance.get_physical_device_properties(self.physical) };
        let limits = properties.limits;
        let byte_count = (rows.len() as u64 + u64::from(FIRST_MATERIAL_ROW)) * 4;
        // ids.y is part of the existing scene ABI and carries an exact integer
        // as a float. A storage-buffer index must not alias a different object.
        if rows.len() > (1 << 24) || byte_count > u64::from(limits.max_storage_buffer_range) {
            return Err(VulkanError::Resource(
                "Scene material index buffer exceeds device limits".into(),
            ));
        }
        let mut sources = BTreeMap::new();
        for source in scene
            .items
            .iter()
            .filter_map(|item| item.material.base_color_texture.as_ref())
        {
            sources.entry(image_key(source)).or_insert(source);
        }
        if sources.len() >= MATERIAL_IMAGE_COUNT as usize {
            // More tiny textures can still fit in the compatibility atlas.
            return Ok(None);
        }
        let mut uploads = MaterialUploads::default();
        let white = match previous {
            Some(previous) => previous.white.clone(),
            None => {
                let texture = Arc::new(self.material_texture(None, limits.max_image_dimension2_d)?);
                uploads.add(texture.clone());
                texture
            }
        };
        let mut textures = BTreeMap::new();
        for (key, source) in sources {
            let texture = match previous.and_then(|set| set.textures.get(&key)) {
                Some(texture) => texture.clone(),
                None => {
                    let texture = Arc::new(
                        self.material_texture(Some(source.clone()), limits.max_image_dimension2_d)?,
                    );
                    uploads.add(texture.clone());
                    texture
                }
            };
            textures.insert(key, texture);
        }
        let texture_indices: BTreeMap<_, _> = textures
            .keys()
            .enumerate()
            .map(|(index, key)| (*key, index as u32 + 1))
            .collect();
        let indices = self.buffer(byte_count, vk::BufferUsageFlags::STORAGE_BUFFER)?;
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
                            .write(texture_indices.get(key).copied().unwrap_or(0));
                    }
                })?;
        }
        let sizes = DESCRIPTORS.map(|(ty, count)| vk::DescriptorPoolSize {
            ty,
            descriptor_count: count,
        });
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
                _indices: indices,
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
            let mut image_infos = Vec::with_capacity(MATERIAL_IMAGE_COUNT as usize);
            image_infos.push(white);
            image_infos.extend(result.textures.values().map(|image| sampled_image(image)));
            image_infos.resize(MATERIAL_IMAGE_COUNT as usize, white);
            let sampler_info = [vk::DescriptorImageInfo::default().sampler(layout.sampler)];
            let buffer_info = [vk::DescriptorBufferInfo::default()
                .buffer(result._indices.buffer)
                .range(byte_count)];
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
            ];
            self.device.update_descriptor_sets(&writes, &[]);
            Ok(Some((Arc::new(result), uploads)))
        }
    }

    fn material_texture(
        &self,
        source: Option<Arc<TextureImage>>,
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
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(FORMAT)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(mip_count(width, height))
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(
                vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST,
            )
            .initial_layout(vk::ImageLayout::UNDEFINED);
        let image = self.allocate_image(&info, vk::ImageAspectFlags::COLOR)?;
        Ok(MaterialTexture { image, source })
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
    let region = vk::BufferImageCopy::default()
        .buffer_offset(offset)
        .image_subresource(subresource(0))
        .image_extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        });
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
