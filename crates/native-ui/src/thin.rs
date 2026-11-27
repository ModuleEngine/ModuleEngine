use super::{
    VkResultExt,
    gpu::{Frames, HostBuffer},
    painter::upload,
};
use ash::vk;
use egui::{
    ImageData, Rect, TextureFilter, TextureId, TextureOptions, TextureWrapMode,
    epaint::{ClippedPrimitive, ImageDelta, Primitive, Vertex},
};
use std::{
    collections::HashMap,
    mem::{offset_of, size_of},
    path::Path,
};

pub(super) struct ThinPainter {
    textures: HashMap<TextureId, Texture>,
    mesh_buffers: Option<(HostBuffer, HostBuffer)>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    pipeline: Pipeline,
}

impl ThinPainter {
    pub fn new(frames: &Frames, root: &Path) -> Result<Self, String> {
        Ok(Self {
            textures: HashMap::new(),
            mesh_buffers: None,
            vertices: Vec::new(),
            indices: Vec::new(),
            pipeline: Pipeline::new(frames, root)?,
        })
    }

    pub fn free_texture(&mut self, id: TextureId) {
        self.textures.remove(&id);
    }

    pub fn set_texture(
        &mut self,
        frames: &Frames,
        id: TextureId,
        delta: &ImageDelta,
    ) -> Result<(), String> {
        let ImageData::Color(image) = &delta.image;
        if let Some(offset) = delta.pos {
            let texture = self
                .textures
                .get_mut(&id)
                .ok_or("Texture patch arrived before allocation")?;
            if !offset.iter().zip(image.size).zip(texture.size).all(
                |((&offset, size), dimension)| {
                    offset.checked_add(size).is_some_and(|end| end <= dimension)
                },
            ) {
                return Err("Texture patch exceeds its allocated image".into());
            }
            texture.set_options(delta.options)?;
            texture.upload(
                frames,
                offset,
                image.size,
                bytemuck::cast_slice(&image.pixels),
                true,
            )
        } else {
            let texture = Texture::new(frames, &self.pipeline, image.size, delta.options)?;
            texture.upload(
                frames,
                [0, 0],
                image.size,
                bytemuck::cast_slice(&image.pixels),
                false,
            )?;
            self.textures.insert(id, texture);
            Ok(())
        }
    }

    pub fn record(
        &mut self,
        frames: &Frames,
        cmd: vk::CommandBuffer,
        extent: vk::Extent2D,
        scale: f32,
        primitives: &[ClippedPrimitive],
    ) -> Result<(), String> {
        self.vertices.clear();
        self.indices.clear();
        for item in primitives {
            let Primitive::Mesh(mesh) = &item.primitive else {
                return Err("R3.1 accepts egui meshes only".into());
            };
            self.vertices.extend_from_slice(&mesh.vertices);
            self.indices.extend_from_slice(&mesh.indices);
        }
        if self.indices.is_empty() {
            return Ok(());
        }
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&self.vertices);
        let index_bytes: &[u8] = bytemuck::cast_slice(&self.indices);
        let buffers = &mut self.mesh_buffers;
        if buffers
            .as_ref()
            .is_none_or(|(v, i)| v.capacity < vertex_bytes.len() || i.capacity < index_bytes.len())
        {
            *buffers = Some((
                frames.host_buffer(
                    vertex_bytes.len().next_power_of_two(),
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                )?,
                frames.host_buffer(
                    index_bytes.len().next_power_of_two(),
                    vk::BufferUsageFlags::INDEX_BUFFER,
                )?,
            ));
        }
        let (vertices, indices) = buffers.as_ref().expect("mesh buffers reserved");
        vertices.write(vertex_bytes)?;
        indices.write(index_bytes)?;
        let screen = [extent.width as f32 / scale, extent.height as f32 / scale];
        let device = &frames.device;
        // Rendering is active. The sole frame slot retains both draw buffers,
        // and textures cannot be replaced until its fence completes.
        unsafe {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline.raw);
            device.cmd_set_viewport(
                cmd,
                0,
                &[vk::Viewport::default()
                    .width(extent.width as f32)
                    .height(extent.height as f32)
                    .max_depth(1.)],
            );
            device.cmd_push_constants(
                cmd,
                self.pipeline.layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                bytemuck::bytes_of(&screen),
            );
            device.cmd_bind_vertex_buffers(cmd, 0, &[vertices.raw], &[0]);
            device.cmd_bind_index_buffer(cmd, indices.raw, 0, vk::IndexType::UINT32);
            let mut first_index = 0;
            let mut base_vertex = 0;
            let mut bound_texture = None;
            for item in primitives {
                let Primitive::Mesh(mesh) = &item.primitive else {
                    unreachable!("primitive types checked before uploading");
                };
                let count = mesh.indices.len() as u32;
                if let Some(scissor) = scissor(item.clip_rect, extent, scale).filter(|_| count != 0)
                {
                    if bound_texture != Some(mesh.texture_id) {
                        let texture = self
                            .textures
                            .get(&mesh.texture_id)
                            .ok_or_else(|| format!("Unknown egui texture {:?}", mesh.texture_id))?;
                        device.cmd_bind_descriptor_sets(
                            cmd,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.pipeline.layout,
                            0,
                            &[texture.set],
                            &[],
                        );
                        bound_texture = Some(mesh.texture_id);
                    }
                    device.cmd_set_scissor(cmd, 0, &[scissor]);
                    device.cmd_draw_indexed(cmd, count, 1, first_index, base_vertex, 0);
                }
                first_index += count;
                base_vertex += mesh.vertices.len() as i32;
            }
        }
        Ok(())
    }
}

fn scissor(rect: Rect, extent: vk::Extent2D, scale: f32) -> Option<vk::Rect2D> {
    let (width, height) = (extent.width as f32, extent.height as f32);
    let x0 = (rect.min.x * scale).round().clamp(0., width) as i32;
    let y0 = (rect.min.y * scale).round().clamp(0., height) as i32;
    let x1 = (rect.max.x * scale).round().clamp(x0 as f32, width) as i32;
    let y1 = (rect.max.y * scale).round().clamp(y0 as f32, height) as i32;
    (x1 > x0 && y1 > y0).then_some(vk::Rect2D {
        offset: vk::Offset2D { x: x0, y: y0 },
        extent: vk::Extent2D {
            width: (x1 - x0) as u32,
            height: (y1 - y0) as u32,
        },
    })
}

struct Pipeline {
    device: ash::Device,
    set_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    layout: vk::PipelineLayout,
    raw: vk::Pipeline,
}

impl Pipeline {
    fn new(frames: &Frames, root: &Path) -> Result<Self, String> {
        let device = &frames.device;
        let mut pipeline = Self {
            device: device.clone(),
            set_layout: vk::DescriptorSetLayout::null(),
            pool: vk::DescriptorPool::null(),
            layout: vk::PipelineLayout::null(),
            raw: vk::Pipeline::null(),
        };
        let types = [
            vk::DescriptorType::SAMPLED_IMAGE,
            vk::DescriptorType::SAMPLER,
        ];
        let bindings = types.map(|ty| {
            vk::DescriptorSetLayoutBinding::default()
                .descriptor_type(ty)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        });
        let bindings = [bindings[0].binding(0), bindings[1].binding(1)];
        const TEXTURES: u32 = 256;
        let sizes = types.map(|ty| vk::DescriptorPoolSize {
            ty,
            descriptor_count: TEXTURES,
        });
        let push = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX)
            .size(8)];
        // Created handles are retained by Pipeline, including on partial construction.
        unsafe {
            pipeline.set_layout = device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .at("create thin painter descriptor layout")?;
            pipeline.pool = device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
                        .max_sets(TEXTURES)
                        .pool_sizes(&sizes),
                    None,
                )
                .at("create thin painter descriptor pool")?;
            pipeline.layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&[pipeline.set_layout])
                        .push_constant_ranges(&push),
                    None,
                )
                .at("create thin painter pipeline layout")?;
        }
        pipeline.raw = pipeline.build(frames.format, root)?;
        Ok(pipeline)
    }

    fn build(&self, format: vk::Format, root: &Path) -> Result<vk::Pipeline, String> {
        let device = &self.device;
        let source = include_str!("thin.hlsl");
        let vertex = module_engine_vulkan::compile_hlsl_at(root, source, "vertex_main", "vs_6_0")
            .map_err(|e| e.to_string())?;
        let fragment =
            module_engine_vulkan::compile_hlsl_at(root, source, "fragment_main", "ps_6_0")
                .map_err(|e| e.to_string())?;
        // The shader words were compiled for Vulkan 1.3 and live until their modules are built.
        unsafe {
            let vertex = Shader::new(device, &vertex.words)?;
            let fragment = Shader::new(device, &fragment.words)?;
            let stages = [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .module(vertex.raw)
                    .name(c"vertex_main"),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .module(fragment.raw)
                    .name(c"fragment_main"),
            ];
            let binding =
                [vk::VertexInputBindingDescription::default().stride(size_of::<Vertex>() as u32)];
            let attributes = [
                vk::VertexInputAttributeDescription::default()
                    .location(0)
                    .format(vk::Format::R32G32_SFLOAT)
                    .offset(offset_of!(Vertex, pos) as u32),
                vk::VertexInputAttributeDescription::default()
                    .location(1)
                    .format(vk::Format::R32G32_SFLOAT)
                    .offset(offset_of!(Vertex, uv) as u32),
                vk::VertexInputAttributeDescription::default()
                    .location(2)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .offset(offset_of!(Vertex, color) as u32),
            ];
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
                .vertex_binding_descriptions(&binding)
                .vertex_attribute_descriptions(&attributes);
            let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let viewport = vk::PipelineViewportStateCreateInfo::default()
                .viewport_count(1)
                .scissor_count(1);
            let raster = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL)
                .cull_mode(vk::CullModeFlags::NONE)
                .line_width(1.);
            let samples = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let attachments = [vk::PipelineColorBlendAttachmentState::default()
                .blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::ONE)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .src_alpha_blend_factor(vk::BlendFactor::ONE)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_write_mask(vk::ColorComponentFlags::RGBA)];
            let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);
            let dynamic = vk::PipelineDynamicStateCreateInfo::default()
                .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]);
            let formats = [format];
            let mut rendering =
                vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&formats);
            let info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages)
                .vertex_input_state(&vertex_input)
                .input_assembly_state(&assembly)
                .viewport_state(&viewport)
                .rasterization_state(&raster)
                .multisample_state(&samples)
                .color_blend_state(&blend)
                .dynamic_state(&dynamic)
                .layout(self.layout)
                .push_next(&mut rendering);
            match device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None) {
                Ok(created) => Ok(created[0]),
                Err((partial, error)) => {
                    for handle in partial {
                        device.destroy_pipeline(handle, None);
                    }
                    Err(format!("Create thin painter pipeline: {error:?}"))
                }
            }
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        // Textures and draw work have been retired before pipeline teardown.
        unsafe {
            self.device.destroy_pipeline(self.raw, None);
            self.device.destroy_pipeline_layout(self.layout, None);
            self.device.destroy_descriptor_pool(self.pool, None);
            self.device
                .destroy_descriptor_set_layout(self.set_layout, None);
        }
    }
}

struct Shader<'a> {
    device: &'a ash::Device,
    raw: vk::ShaderModule,
}

impl<'a> Shader<'a> {
    unsafe fn new(device: &'a ash::Device, words: &[u32]) -> Result<Self, String> {
        // The caller supplies SPIR-V words compiled for the device's Vulkan environment.
        let raw = unsafe {
            device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(words), None)
        }
        .at("create thin painter shader module")?;
        Ok(Self { device, raw })
    }
}

impl Drop for Shader<'_> {
    fn drop(&mut self) {
        // The module is needed only through graphics pipeline creation.
        unsafe { self.device.destroy_shader_module(self.raw, None) };
    }
}

struct Texture {
    device: ash::Device,
    pool: vk::DescriptorPool,
    size: [usize; 2],
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    sampler: vk::Sampler,
    options: TextureOptions,
    set: vk::DescriptorSet,
}

impl Texture {
    fn new(
        frames: &Frames,
        pipeline: &Pipeline,
        size: [usize; 2],
        options: TextureOptions,
    ) -> Result<Self, String> {
        let device = &frames.device;
        let mut texture = Self {
            device: device.clone(),
            pool: pipeline.pool,
            size,
            image: vk::Image::null(),
            memory: vk::DeviceMemory::null(),
            view: vk::ImageView::null(),
            sampler: vk::Sampler::null(),
            options,
            set: vk::DescriptorSet::null(),
        };
        let format = vk::Format::R8G8B8A8_UNORM;
        if size.contains(&0) || size.iter().any(|&n| n > 8192) {
            return Err("Thin painter texture dimensions exceed the spike limits".into());
        }
        // The device and descriptor pool outlive all textures. Partial handles
        // are initialized to null and are released by Texture.
        unsafe {
            texture.image = device
                .create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(format)
                        .extent(vk::Extent3D {
                            width: size[0] as u32,
                            height: size[1] as u32,
                            depth: 1,
                        })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED),
                    None,
                )
                .at("create thin painter texture image")?;
            texture.memory = frames.allocate(
                device.get_image_memory_requirements(texture.image),
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;
            device
                .bind_image_memory(texture.image, texture.memory, 0)
                .at("bind thin painter texture")?;
            texture.view = device
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(texture.image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .level_count(1)
                                .layer_count(1),
                        ),
                    None,
                )
                .at("create thin painter texture view")?;
            texture.set = device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(pipeline.pool)
                        .set_layouts(&[pipeline.set_layout]),
                )
                .at("allocate thin painter texture descriptor set")?[0];
        }
        texture.set_options(options)?;
        Ok(texture)
    }

    fn set_options(&mut self, options: TextureOptions) -> Result<(), String> {
        if self.sampler != vk::Sampler::null() && self.options == options {
            return Ok(());
        }
        let filter = |filter| match filter {
            TextureFilter::Nearest => vk::Filter::NEAREST,
            TextureFilter::Linear => vk::Filter::LINEAR,
        };
        let wrap = match options.wrap_mode {
            TextureWrapMode::ClampToEdge => vk::SamplerAddressMode::CLAMP_TO_EDGE,
            TextureWrapMode::Repeat => vk::SamplerAddressMode::REPEAT,
            TextureWrapMode::MirroredRepeat => vk::SamplerAddressMode::MIRRORED_REPEAT,
        };
        // Either new allocation or the spike's only frame has retired before the
        // descriptor update and replacement. The image has exactly one mip.
        unsafe {
            let sampler = self
                .device
                .create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(filter(options.magnification))
                        .min_filter(filter(options.minification))
                        .address_mode_u(wrap)
                        .address_mode_v(wrap)
                        .address_mode_w(wrap),
                    None,
                )
                .at("create thin painter sampler")?;
            let image_info = [vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(self.view)];
            let sampler_info = [vk::DescriptorImageInfo::default().sampler(sampler)];
            self.device.update_descriptor_sets(
                &[
                    vk::WriteDescriptorSet::default()
                        .dst_set(self.set)
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                        .image_info(&image_info),
                    vk::WriteDescriptorSet::default()
                        .dst_set(self.set)
                        .dst_binding(1)
                        .descriptor_type(vk::DescriptorType::SAMPLER)
                        .image_info(&sampler_info),
                ],
                &[],
            );
            self.device.destroy_sampler(self.sampler, None);
            self.sampler = sampler;
        }
        self.options = options;
        Ok(())
    }

    fn upload(
        &self,
        frames: &Frames,
        offset: [usize; 2],
        size: [usize; 2],
        bytes: &[u8],
        patch: bool,
    ) -> Result<(), String> {
        if bytes.len() != size[0] * size[1] * 4 || bytes.is_empty() {
            return Err("egui texture upload has invalid pixel bytes".into());
        }
        let buffer = frames.host_buffer(bytes.len(), vk::BufferUsageFlags::TRANSFER_SRC)?;
        buffer.write(bytes)?;
        let sample = (
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::AccessFlags::SHADER_READ,
        );
        let copy = (
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::TRANSFER_WRITE,
        );
        let old = if patch {
            sample
        } else {
            (
                vk::ImageLayout::UNDEFINED,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::AccessFlags::empty(),
            )
        };
        upload(frames, |command| {
            frames.transition(command, self.image, old, copy);
            // Dimensions and patch bounds were checked and upload retires before buffer drop.
            unsafe {
                frames.device.cmd_copy_buffer_to_image(
                    command,
                    buffer.raw,
                    self.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[vk::BufferImageCopy::default()
                        .image_subresource(
                            vk::ImageSubresourceLayers::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .layer_count(1),
                        )
                        .image_offset(
                            vk::Offset3D::default()
                                .x(offset[0] as i32)
                                .y(offset[1] as i32),
                        )
                        .image_extent(
                            vk::Extent3D::default()
                                .width(size[0] as u32)
                                .height(size[1] as u32)
                                .depth(1),
                        )],
                );
            }
            frames.transition(command, self.image, copy, sample);
        })
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        // The sole frame fence has retired all uses or the image is still being constructed.
        unsafe {
            if self.set != vk::DescriptorSet::null()
                && let Err(error) = self.device.free_descriptor_sets(self.pool, &[self.set])
            {
                eprintln!("Free thin painter texture descriptor set: {error:?}");
            }
            self.device.destroy_sampler(self.sampler, None);
            self.device.destroy_image_view(self.view, None);
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None);
        }
    }
}
