use super::{
    Buffer, Image, OwnedShaderModule, VulkanError, VulkanRenderer, compile_hlsl_at, resident,
    transition,
};
use ash::vk;
use std::{ffi::CString, path::Path, sync::Arc};

const CULL: &str = include_str!("../../../assets/gpu/scene_culling.hlsl");
const HIZ: &str = include_str!("../../../assets/gpu/hiz.hlsl");
type Shader = (&'static str, &'static str, &'static str);
const CLASSIFY: Shader = (CULL, "classify", "cs_6_0");
const SCAN: Shader = (CULL, "scanGroups", "cs_6_0");
const COMPACT: Shader = (CULL, "compact", "cs_6_0");
const OCCLUSION: Shader = (CULL, "classifyOcclusion", "cs_6_0");
const REDUCE_DEPTH: Shader = (HIZ, "reduceDepth", "cs_6_0");
const REDUCE_MIP: Shader = (HIZ, "reduceMip", "cs_6_0");
const SEED_DEPTH: Shader = (resident::VERTEX_SHADER, "depthVS", "vs_6_0");
pub(crate) const SHADERS: [Shader; 7] = [
    CLASSIFY,
    SCAN,
    COMPACT,
    OCCLUSION,
    REDUCE_DEPTH,
    REDUCE_MIP,
    SEED_DEPTH,
];

const GROUP: u32 = 128;
const COUNT_BYTES: u64 = 12;
const MAX_FRUSTUM_GROUPS: u32 = 65_535; // Required Vulkan minimum for X dispatches.
const STORAGE: vk::DescriptorType = vk::DescriptorType::STORAGE_BUFFER;
const SAMPLED: vk::DescriptorType = vk::DescriptorType::SAMPLED_IMAGE;
const COMPUTE: vk::ShaderStageFlags = vk::ShaderStageFlags::COMPUTE;
const VERTEX: vk::ShaderStageFlags = vk::ShaderStageFlags::VERTEX;

/// Limit the stock resident geometry path for diagnostics. The actual path
/// can fall back to frustum-only or to the R2.3 submission path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneCulling {
    Off,
    Frustum,
    #[default]
    HiZ,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct CullingStats {
    pub frustum_candidates: u32,
    pub occluders: u32,
    pub visible: u32,
}

struct Layout {
    device: ash::Device,
    set: vk::DescriptorSetLayout,
    raw: vk::PipelineLayout,
    pipelines: Vec<vk::Pipeline>,
}

impl Layout {
    fn new(
        device: &ash::Device,
        descriptors: &[(u32, vk::DescriptorType)],
        stage: vk::ShaderStageFlags,
        push_bytes: u32,
    ) -> Result<Self, VulkanError> {
        let bindings: Vec<_> = descriptors
            .iter()
            .map(|&(binding, ty)| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(binding)
                    .descriptor_type(ty)
                    .descriptor_count(1)
                    .stage_flags(stage)
            })
            .collect();
        // SAFETY: Layout creation consumes these arrays synchronously. The
        // owner destroys all pipelines before their two layouts.
        unsafe {
            let set = device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .map_err(VulkanError::api("create scene culling descriptor layout"))?;
            let mut layout = Self {
                device: device.clone(),
                set,
                raw: vk::PipelineLayout::null(),
                pipelines: Vec::new(),
            };
            let push = [vk::PushConstantRange::default()
                .stage_flags(stage)
                .size(push_bytes)];
            layout.raw = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&[set])
                        .push_constant_ranges(&push),
                    None,
                )
                .map_err(VulkanError::api("create scene culling pipeline layout"))?;
            Ok(layout)
        }
    }

    fn compute(&mut self, root: &Path, shader: Shader) -> Result<vk::Pipeline, VulkanError> {
        let (module, name) = shader_module(&self.device, root, shader)?;
        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(COMPUTE)
            .module(module.1)
            .name(&name);
        // SAFETY: The module, shader entry and compatible layout remain live
        // during creation. Layout owns the successful pipeline.
        let result = unsafe {
            self.device.create_compute_pipelines(
                vk::PipelineCache::null(),
                &[vk::ComputePipelineCreateInfo::default()
                    .stage(stage)
                    .layout(self.raw)],
                None,
            )
        };
        drop(module);
        self.keep_pipeline(result)
    }

    fn depth(&mut self, root: &Path) -> Result<vk::Pipeline, VulkanError> {
        let (module, name) = shader_module(&self.device, root, SEED_DEPTH)?;
        let stages = [vk::PipelineShaderStageCreateInfo::default()
            .stage(VERTEX)
            .module(module.1)
            .name(&name)];
        let mut rendering = vk::PipelineRenderingCreateInfo::default()
            .depth_attachment_format(vk::Format::D32_SFLOAT);
        let vertices = vk::PipelineVertexInputStateCreateInfo::default();
        let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let raster = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .line_width(1.0);
        let samples = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let depth = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS);
        let dynamic = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]);
        let create = vk::GraphicsPipelineCreateInfo::default()
            .push_next(&mut rendering)
            .layout(self.raw)
            .stages(&stages)
            .vertex_input_state(&vertices)
            .input_assembly_state(&assembly)
            .viewport_state(&viewport)
            .rasterization_state(&raster)
            .multisample_state(&samples)
            .depth_stencil_state(&depth)
            .dynamic_state(&dynamic);
        // SAFETY: This pipeline writes only D32 depth. The creation structs
        // and shader module remain live through the call.
        let result = unsafe {
            self.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[create], None)
        };
        drop(module);
        self.keep_pipeline(result)
    }

    fn keep_pipeline(
        &mut self,
        result: Result<Vec<vk::Pipeline>, (Vec<vk::Pipeline>, vk::Result)>,
    ) -> Result<vk::Pipeline, VulkanError> {
        match result {
            Ok(pipelines) => {
                let pipeline = pipelines[0];
                self.pipelines.push(pipeline);
                Ok(pipeline)
            }
            Err((partial, source)) => {
                for pipeline in partial {
                    // SAFETY: A partially created pipeline was never recorded.
                    unsafe { self.device.destroy_pipeline(pipeline, None) };
                }
                Err(VulkanError::api("create scene culling pipeline")(source))
            }
        }
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        // SAFETY: The recorded flight keeps these handles until completion.
        unsafe {
            for pipeline in self.pipelines.drain(..) {
                self.device.destroy_pipeline(pipeline, None);
            }
            self.device.destroy_pipeline_layout(self.raw, None);
            self.device.destroy_descriptor_set_layout(self.set, None);
        }
    }
}

fn shader_module<'a>(
    device: &'a ash::Device,
    root: &Path,
    (source, entry, profile): Shader,
) -> Result<(OwnedShaderModule<'a>, CString), VulkanError> {
    let artifact = compile_hlsl_at(root, source, entry, profile)?;
    // SAFETY: DXC produced SPIR-V for the selected shader and target. The
    // creation info borrows the words for the duration of this call only.
    let module = unsafe {
        device.create_shader_module(
            &vk::ShaderModuleCreateInfo::default().code(&artifact.words),
            None,
        )
    }
    .map_err(VulkanError::api("create scene culling shader module"))?;
    Ok((
        OwnedShaderModule(device, module),
        CString::new(entry).unwrap(),
    ))
}

pub(super) struct Pipelines {
    layout: Layout,
    classify: vk::Pipeline,
    scan: vk::Pipeline,
    compact: vk::Pipeline,
    hiz: Option<HiZPipelines>,
    _context: Arc<crate::presentation::DeviceLifetime>,
}

struct HiZPipelines {
    test: vk::Pipeline,
    reduction: Layout,
    reduce_depth: vk::Pipeline,
    reduce_mip: vk::Pipeline,
    seed_layout: Layout,
    seed: vk::Pipeline,
}

impl Pipelines {
    pub(super) fn new(renderer: &VulkanRenderer) -> Result<Self, VulkanError> {
        let device = &renderer.device;
        let root = &renderer.resource_root;
        let storage = [0, 1, 2, 3, 4, 5].map(|binding| (binding, STORAGE));
        let mut layout = Layout::new(device, &storage, COMPUTE, 112)?;
        let classify = layout.compute(root, CLASSIFY)?;
        let scan = layout.compute(root, SCAN)?;
        let compact = layout.compute(root, COMPACT)?;
        let hiz = if renderer.capabilities.hiz_occlusion_culling {
            let test = layout.compute(root, OCCLUSION)?;
            let mut reduction = Layout::new(device, &[(0, SAMPLED), (1, STORAGE)], COMPUTE, 24)?;
            let reduce_depth = reduction.compute(root, REDUCE_DEPTH)?;
            let reduce_mip = reduction.compute(root, REDUCE_MIP)?;
            let mut seed_layout = Layout::new(device, &[(1, STORAGE), (3, STORAGE)], VERTEX, 128)?;
            let seed = seed_layout.depth(root)?;
            Some(HiZPipelines {
                test,
                reduction,
                reduce_depth,
                reduce_mip,
                seed_layout,
                seed,
            })
        } else {
            None
        };
        Ok(Self {
            layout,
            classify,
            scan,
            compact,
            hiz,
            _context: renderer.context.clone(),
        })
    }
}

struct Scratch {
    buffer: Buffer,
    capacity: u32,
    with_bounds: bool,
}

impl Scratch {
    fn offsets(capacity: u32) -> [u32; 4] {
        let groups = COUNT_BYTES as u32 + 4 * capacity;
        let commands = groups + 4 * capacity.div_ceil(GROUP);
        let records = commands + 20 * capacity;
        let screen_bounds = records + 12 * capacity;
        [groups, commands, records, screen_bounds]
    }

    fn size(capacity: u32, with_bounds: bool) -> u64 {
        u64::from(Self::offsets(capacity)[3] + if with_bounds { 20 * capacity } else { 0 })
    }
}

#[derive(Clone, Copy)]
struct Level {
    size: [u32; 2],
    offset: u32,
}

impl Level {
    fn texels(self) -> u32 {
        self.size[0] * self.size[1]
    }
}

struct Pyramid {
    depth: Image,
    buffer: Buffer,
    levels: Vec<Level>,
}

impl Pyramid {
    fn new(renderer: &VulkanRenderer, size: [u32; 2]) -> Result<Option<Self>, VulkanError> {
        let usage = vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED;
        // SAFETY: This query targets the renderer's selected adapter and
        // the exact format and usage of the single-sample seed image.
        let props = unsafe {
            renderer
                .instance
                .get_physical_device_image_format_properties(
                    renderer.physical,
                    vk::Format::D32_SFLOAT,
                    vk::ImageType::TYPE_2D,
                    vk::ImageTiling::OPTIMAL,
                    usage,
                    vk::ImageCreateFlags::empty(),
                )
        }
        .map_err(VulkanError::api("query Hi-Z depth image limits"))?;
        let [width, height] = size;
        if width > props.max_extent.width || height > props.max_extent.height {
            return Ok(None);
        }
        let mut levels = Vec::new();
        let mut level = Level { size, offset: 0 };
        loop {
            level.size = level.size.map(|v| v.div_ceil(2));
            levels.push(level);
            if level.size == [1, 1] {
                break;
            }
            level.offset += level.texels();
        }
        let bytes = u64::from(level.offset + level.texels()) * 4;
        if bytes > renderer.storage_buffer_limit {
            return Ok(None);
        }
        let depth = renderer.allocate_image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::D32_SFLOAT)
                .extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(usage),
            vk::ImageAspectFlags::DEPTH,
        )?;
        let buffer = renderer.device_buffer(bytes, vk::BufferUsageFlags::STORAGE_BUFFER)?;
        Ok(Some(Self {
            depth,
            buffer,
            levels,
        }))
    }
}

pub(super) struct Frame {
    scratch: Scratch,
    pyramid: Option<Pyramid>,
    counters: Option<Buffer>,
}

impl Frame {
    pub(super) fn prepare(
        renderer: &VulkanRenderer,
        previous: Option<Self>,
        size: [u32; 2],
        objects: usize,
    ) -> Result<Option<Self>, VulkanError> {
        if renderer.culling_mode == SceneCulling::Off
            || objects == 0
            || objects > renderer.max_indirect_draws as usize
            || objects > (MAX_FRUSTUM_GROUPS * GROUP) as usize
            || Scratch::size(objects as u32, false) > renderer.storage_buffer_limit
        {
            return Ok(None);
        }
        let objects = objects as u32;
        let mut want_hiz = renderer.culling_mode == SceneCulling::HiZ
            && renderer.capabilities.hiz_occlusion_culling
            && Scratch::size(objects, true) <= renderer.storage_buffer_limit;
        let (old_scratch, old_pyramid, old_counters) = previous
            .map(|previous| (Some(previous.scratch), previous.pyramid, previous.counters))
            .unwrap_or_default();
        // Caller only supplies slot reuse with the same viewport and frame key.
        let pyramid = if want_hiz {
            match old_pyramid {
                some @ Some(_) => some,
                None => Pyramid::new(renderer, size)?,
            }
        } else {
            None
        };
        want_hiz = pyramid.is_some();
        let scratch = match old_scratch
            .filter(|s| s.capacity >= objects && (!want_hiz || s.with_bounds))
        {
            Some(scratch) => scratch,
            None => {
                let mut capacity = objects.next_power_of_two();
                if Scratch::size(capacity, want_hiz) > renderer.storage_buffer_limit {
                    capacity = objects;
                }
                let buffer = renderer.device_buffer(
                    Scratch::size(capacity, want_hiz),
                    vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::INDIRECT_BUFFER,
                )?;
                Scratch {
                    buffer,
                    capacity,
                    with_bounds: want_hiz,
                }
            }
        };
        let counters = if renderer.culling_stats {
            Some(match old_counters {
                Some(buffer) => buffer,
                None => renderer.buffer(COUNT_BYTES, vk::BufferUsageFlags::TRANSFER_DST)?,
            })
        } else {
            None
        };
        Ok(Some(Self {
            scratch,
            pyramid,
            counters,
        }))
    }

    pub(super) fn mode(&self) -> SceneCulling {
        if self.pyramid.is_some() {
            SceneCulling::HiZ
        } else {
            SceneCulling::Frustum
        }
    }

    pub(super) fn visible_buffer(&self) -> &Buffer {
        &self.scratch.buffer
    }

    pub(super) fn visible_offset(&self) -> u32 {
        Scratch::offsets(self.scratch.capacity)[2]
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn record(
        &mut self,
        device: &ash::Device,
        command: vk::CommandBuffer,
        pool: vk::DescriptorPool,
        pipelines: &Pipelines,
        scene: &resident::SceneUpload,
        camera: [f32; 32],
        size: [u32; 2],
        objects: u32,
    ) -> Result<(), VulkanError> {
        let buffers = &scene.buffers;
        let culling = buffers
            .culling
            .as_ref()
            .expect("resident GPU culling inputs");
        let work = &self.scratch.buffer;
        let mut inputs = vec![
            (0, &*buffers.instances),
            (1, &*buffers.draws),
            (2, &*culling.bounds),
            (3, &*culling.runs),
            (4, work),
        ];
        if let Some(pyramid) = &self.pyramid {
            inputs.push((5, &pyramid.buffer));
        }
        let set = descriptors(device, pool, &pipelines.layout, &inputs, None)?;
        let groups = objects.div_ceil(GROUP);
        let mut push = camera.map(f32::to_bits);
        let counts = [
            objects,
            scene.runs.len() as u32,
            groups,
            u32::from(self.pyramid.is_some()),
        ];
        let offsets = Scratch::offsets(self.scratch.capacity);
        push[16..20].copy_from_slice(&counts);
        push[20..24].copy_from_slice(&offsets);
        push[24..26].copy_from_slice(&size);
        let compute_push = push.first_chunk::<28>().unwrap().map(u32::to_ne_bytes);

        // SAFETY: This slot completed its previous submission. Scratch
        // holds the three counters and all invocations fit its capacity.
        // Descriptor sets, buffers and pipelines are owned by this flight.
        unsafe {
            device.cmd_fill_buffer(command, work.buffer, 0, COUNT_BYTES, 0);
            barrier(
                device,
                command,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::AccessFlags::TRANSFER_WRITE,
                vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE,
            );
            let layout = pipelines.layout.raw;
            device.cmd_bind_descriptor_sets(
                command,
                vk::PipelineBindPoint::COMPUTE,
                layout,
                0,
                &[set],
                &[],
            );
            device.cmd_push_constants(command, layout, COMPUTE, 0, compute_push.as_flattened());
            dispatch(device, command, pipelines.classify, [groups, 1]);
            if let Some(pyramid) = &mut self.pyramid {
                let hiz = pipelines.hiz.as_ref().expect("Hi-Z pipelines");
                barrier(
                    device,
                    command,
                    vk::PipelineStageFlags::COMPUTE_SHADER | vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::DRAW_INDIRECT,
                    vk::AccessFlags::SHADER_WRITE | vk::AccessFlags::TRANSFER_WRITE,
                    vk::AccessFlags::INDIRECT_COMMAND_READ,
                );
                pyramid.record_seed(
                    device, command, pool, hiz, buffers, work, &push, size, objects, offsets[1],
                )?;
                pyramid.record_levels(device, command, pool, hiz, size)?;
                // The seed reads the same indirect storage compact reuses.
                // Classification and reduction writes feed the Hi-Z test.
                barrier(
                    device,
                    command,
                    vk::PipelineStageFlags::DRAW_INDIRECT | vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::AccessFlags::INDIRECT_COMMAND_READ | vk::AccessFlags::SHADER_WRITE,
                    vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE,
                );
                device.cmd_bind_descriptor_sets(
                    command,
                    vk::PipelineBindPoint::COMPUTE,
                    layout,
                    0,
                    &[set],
                    &[],
                );
                device.cmd_push_constants(command, layout, COMPUTE, 0, compute_push.as_flattened());
                dispatch(device, command, hiz.test, [groups, 1]);
            }
            compute_barrier(device, command);
            dispatch(device, command, pipelines.scan, [1, 1]);
            compute_barrier(device, command);
            dispatch(device, command, pipelines.compact, [groups, 1]);
            barrier(
                device,
                command,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::DRAW_INDIRECT | vk::PipelineStageFlags::VERTEX_SHADER,
                vk::AccessFlags::SHADER_WRITE,
                vk::AccessFlags::INDIRECT_COMMAND_READ | vk::AccessFlags::SHADER_READ,
            );
            if let Some(counters) = &self.counters {
                barrier(
                    device,
                    command,
                    vk::PipelineStageFlags::COMPUTE_SHADER | vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::AccessFlags::SHADER_WRITE | vk::AccessFlags::TRANSFER_WRITE,
                    vk::AccessFlags::TRANSFER_READ,
                );
                device.cmd_copy_buffer(
                    command,
                    work.buffer,
                    counters.buffer,
                    &[vk::BufferCopy::default().size(COUNT_BYTES)],
                );
                barrier(
                    device,
                    command,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::HOST,
                    vk::AccessFlags::TRANSFER_WRITE,
                    vk::AccessFlags::HOST_READ,
                );
            }
        }
        Ok(())
    }

    pub(super) fn draw(
        &self,
        device: &ash::Device,
        command: vk::CommandBuffer,
        buffers: &resident::Buffers,
        objects: u32,
    ) {
        let corners = &buffers.culling.as_ref().unwrap().corners;
        let args = Scratch::offsets(self.scratch.capacity)[1];
        let work = self.scratch.buffer.buffer;
        // SAFETY: Inside a compatible rendering instance. Compact emitted
        // at most objects commands. The mesh pool's corner stream covers
        // every command, and the preceding barrier publishes the writes.
        unsafe {
            device.cmd_bind_index_buffer(command, corners.buffer, 0, vk::IndexType::UINT32);
            device.cmd_draw_indexed_indirect_count(
                command,
                work,
                args.into(),
                work,
                0,
                objects,
                20,
            );
        }
    }

    pub(super) fn completed(&self) -> Result<Option<CullingStats>, VulkanError> {
        let Some(buffer) = &self.counters else {
            return Ok(None);
        };
        let mut data = [0_u8; COUNT_BYTES as usize];
        // SAFETY: Only called after confirmed GPU completion. Device writes
        // were made visible to the host before signaling the flight.
        unsafe {
            buffer
                .memory
                .as_ref()
                .unwrap()
                .access(buffer.size, |mapped| {
                    std::ptr::copy_nonoverlapping(mapped, data.as_mut_ptr(), data.len());
                })?;
        }
        let [visible, occluders, frustum_candidates] = data
            .as_chunks::<4>()
            .0
            .first_chunk::<3>()
            .unwrap()
            .map(u32::from_ne_bytes);
        Ok(Some(CullingStats {
            frustum_candidates,
            occluders,
            visible,
        }))
    }
}

impl Pyramid {
    #[allow(clippy::too_many_arguments)]
    fn record_seed(
        &mut self,
        device: &ash::Device,
        command: vk::CommandBuffer,
        pool: vk::DescriptorPool,
        pipelines: &HiZPipelines,
        buffers: &resident::Buffers,
        work: &Buffer,
        push: &[u32; 32],
        size: [u32; 2],
        objects: u32,
        args: u32,
    ) -> Result<(), VulkanError> {
        let set = descriptors(
            device,
            pool,
            &pipelines.seed_layout,
            &[(1, &buffers.vertices), (3, &buffers.instances)],
            None,
        )?;
        transition(
            device,
            command,
            &mut self.depth,
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        );
        let depth = vk::RenderingAttachmentInfo::default()
            .image_view(self.depth.view)
            .image_layout(self.depth.layout)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            });
        let [width, height] = size;
        let area = vk::Rect2D {
            offset: vk::Offset2D::default(),
            extent: vk::Extent2D { width, height },
        };
        let rendering = vk::RenderingInfo::default()
            .render_area(area)
            .layer_count(1)
            .depth_attachment(&depth);
        // SAFETY: This is a colorless rendering instance matching the
        // depth-only pipeline. Seed commands index the uploaded mesh pool.
        unsafe {
            device.cmd_begin_rendering(command, &rendering);
            device.cmd_set_viewport(
                command,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: width as f32,
                    height: height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            device.cmd_set_scissor(command, 0, &[area]);
            let bind = vk::PipelineBindPoint::GRAPHICS;
            let layout = pipelines.seed_layout.raw;
            device.cmd_bind_pipeline(command, bind, pipelines.seed);
            device.cmd_bind_descriptor_sets(command, bind, layout, 0, &[set], &[]);
            let camera = push.first_chunk::<16>().unwrap().map(u32::to_ne_bytes);
            device.cmd_push_constants(command, layout, VERTEX, 0, camera.as_flattened());
            device.cmd_bind_index_buffer(command, buffers.indices.buffer, 0, vk::IndexType::UINT32);
            device.cmd_draw_indexed_indirect_count(
                command,
                work.buffer,
                args.into(),
                work.buffer,
                4,
                objects,
                20,
            );
            device.cmd_end_rendering(command);
        }
        Ok(())
    }

    fn record_levels(
        &mut self,
        device: &ash::Device,
        command: vk::CommandBuffer,
        pool: vk::DescriptorPool,
        pipelines: &HiZPipelines,
        size: [u32; 2],
    ) -> Result<(), VulkanError> {
        transition(
            device,
            command,
            &mut self.depth,
            vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            vk::PipelineStageFlags::COMPUTE_SHADER,
            vk::AccessFlags::SHADER_READ,
        );
        let layout = &pipelines.reduction;
        let set = descriptors(
            device,
            pool,
            layout,
            &[(1, &self.buffer)],
            Some(&self.depth),
        )?;
        let mut source = Level { size, offset: 0 };
        // SAFETY: Each invocation writes a valid destination texel. The
        // prior level is always a disjoint region with an explicit barrier.
        unsafe {
            device.cmd_bind_descriptor_sets(
                command,
                vk::PipelineBindPoint::COMPUTE,
                layout.raw,
                0,
                &[set],
                &[],
            );
            for (i, &target) in self.levels.iter().enumerate() {
                let [width, height] = source.size;
                let [next_width, next_height] = target.size;
                let push = [
                    width,
                    height,
                    next_width,
                    next_height,
                    source.offset,
                    target.offset,
                ]
                .map(u32::to_ne_bytes);
                device.cmd_push_constants(command, layout.raw, COMPUTE, 0, push.as_flattened());
                let pipeline = if i == 0 {
                    pipelines.reduce_depth
                } else {
                    pipelines.reduce_mip
                };
                dispatch(
                    device,
                    command,
                    pipeline,
                    target.size.map(|v| v.div_ceil(8)),
                );
                if i + 1 != self.levels.len() {
                    compute_barrier(device, command);
                }
                source = target;
            }
        }
        Ok(())
    }
}

fn descriptors(
    device: &ash::Device,
    pool: vk::DescriptorPool,
    layout: &Layout,
    buffers: &[(u32, &Buffer)],
    depth: Option<&Image>,
) -> Result<vk::DescriptorSet, VulkanError> {
    // SAFETY: The slot pool is valid and unsent. All descriptor resources
    // belong to the flight, rather than to these temporary write arrays.
    unsafe {
        let set = device
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&[layout.set]),
            )
            .map_err(VulkanError::api("allocate scene culling descriptors"))?[0];
        let infos: Vec<_> = buffers
            .iter()
            .map(|(_, buffer)| {
                vk::DescriptorBufferInfo::default()
                    .buffer(buffer.buffer)
                    .range(buffer.size)
            })
            .collect();
        let mut writes: Vec<_> = buffers
            .iter()
            .zip(&infos)
            .map(|(&(binding, _), info)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(binding)
                    .descriptor_type(STORAGE)
                    .buffer_info(std::slice::from_ref(info))
            })
            .collect();
        let image;
        if let Some(depth) = depth {
            image = vk::DescriptorImageInfo::default()
                .image_layout(depth.layout)
                .image_view(depth.view);
            writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(0)
                    .descriptor_type(SAMPLED)
                    .image_info(std::slice::from_ref(&image)),
            );
        }
        device.update_descriptor_sets(&writes, &[]);
        Ok(set)
    }
}

unsafe fn dispatch(
    device: &ash::Device,
    command: vk::CommandBuffer,
    pipeline: vk::Pipeline,
    size: [u32; 2],
) {
    // SAFETY: Caller bound the compatible descriptor set and push constants.
    unsafe {
        device.cmd_bind_pipeline(command, vk::PipelineBindPoint::COMPUTE, pipeline);
        device.cmd_dispatch(command, size[0], size[1], 1);
    }
}

fn compute_barrier(device: &ash::Device, command: vk::CommandBuffer) {
    barrier(
        device,
        command,
        vk::PipelineStageFlags::COMPUTE_SHADER,
        vk::PipelineStageFlags::COMPUTE_SHADER,
        vk::AccessFlags::SHADER_WRITE,
        vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE,
    );
}

fn barrier(
    device: &ash::Device,
    command: vk::CommandBuffer,
    source: vk::PipelineStageFlags,
    destination: vk::PipelineStageFlags,
    available: vk::AccessFlags,
    visible: vk::AccessFlags,
) {
    let ready = vk::MemoryBarrier::default()
        .src_access_mask(available)
        .dst_access_mask(visible);
    // SAFETY: All users are on the graphics+compute queue, and barriers
    // are recorded outside rendering with access masks for these stages.
    unsafe {
        device.cmd_pipeline_barrier(
            command,
            source,
            destination,
            vk::DependencyFlags::empty(),
            &[ready],
            &[],
            &[],
        );
    }
}
