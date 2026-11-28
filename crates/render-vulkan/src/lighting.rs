use super::{Buffer, DeviceLifetime, VulkanError, VulkanRenderer, culling::Layout};
use ash::vk;
use module_engine_core::{LightKind, SceneLight};
use std::sync::Arc;

pub(crate) const SHADER: (&str, &str, &str) = (
    concat!(
        "#define MODULE_ENGINE_BUILD_LIGHT_GRID 1\n",
        include_str!("../../../assets/gpu/cluster_lights.hlsli"),
        "\n",
        include_str!("../../../assets/gpu/cluster_lights.hlsl")
    ),
    "clusterLights",
    "cs_6_0",
);

pub(super) const BINDINGS: [u32; 2] = [8, 9];
pub(super) const MAX_LIGHTS: usize = 1024;
const TILE: u32 = 64;
const SLICES: u32 = 16;
const STORAGE: vk::DescriptorType = vk::DescriptorType::STORAGE_BUFFER;

pub(super) struct Pipeline {
    layout: Layout,
    pipeline: vk::Pipeline,
    _context: Arc<DeviceLifetime>,
}

impl Pipeline {
    pub(super) fn new(renderer: &VulkanRenderer) -> Result<Self, VulkanError> {
        let shader = crate::compile_hlsl_at(&renderer.resource_root, SHADER.0, SHADER.1, SHADER.2)?;
        crate::compiler::verify_light_grid(&shader.words)?;
        let mut layout = Layout::new(
            &renderer.device,
            &BINDINGS.map(|binding| (binding, STORAGE)),
            vk::ShaderStageFlags::COMPUTE,
            128,
        )?;
        let pipeline = layout.compute_artifact(SHADER.1, &shader.words)?;
        renderer
            .context
            .name(pipeline, || "Cluster punctual lights".into());
        Ok(Self {
            layout,
            pipeline,
            _context: renderer.context.clone(),
        })
    }
}

pub(super) struct Frame {
    lights: Buffer,
    grid: Buffer,
    cells: [u32; 3],
    local_count: usize,
    words: u32,
    #[cfg(any(test, feature = "bench"))]
    exhaustive: bool,
}

impl Frame {
    pub(super) fn prepare(
        renderer: &VulkanRenderer,
        reuse: Option<Self>,
        lights: &[SceneLight],
        camera: &[f32; 32],
        size: [u32; 2],
    ) -> Result<Self, VulkanError> {
        if lights.len() > MAX_LIGHTS {
            return Err(VulkanError::Resource(format!(
                "Stock clustered lighting allows {MAX_LIGHTS} lights, got {}",
                lights.len()
            )));
        }
        let directional = lights
            .iter()
            .filter(|light| light.kind == LightKind::Directional);
        let directional_count = directional.clone().count();
        let local_count = lights.len() - directional_count;
        let words = local_count.div_ceil(32) as u32;
        let cells = [size[0].div_ceil(TILE), size[1].div_ceil(TILE), SLICES];
        let mut rows: Vec<[f32; 4]> = Vec::with_capacity(2 + lights.len() * 4);
        rows.push(
            [
                cells[0],
                cells[1],
                local_count as u32,
                directional_count as u32,
            ]
            .map(f32::from_bits),
        );
        rows.push([size[0], size[1], words, 0].map(f32::from_bits));
        for light in directional.chain(
            lights
                .iter()
                .filter(|light| light.kind != LightKind::Directional),
        ) {
            let position = light.position;
            let local = std::array::from_fn::<_, 3, _>(|i| position[i] - camera[i]);
            let view = [4, 8, 12].map(|start| {
                local
                    .iter()
                    .zip(&camera[start..start + 3])
                    .map(|(a, b)| a * b)
                    .sum()
            });
            let [red, green, blue] = light.color.map(|channel| channel * light.intensity);
            let inner = if light.kind == LightKind::Spot {
                light.inner_angle_cos
            } else {
                -1.
            };
            let [x, y, z] = light.direction;
            let inv_range_sq = if light.kind == LightKind::Directional {
                0.
            } else {
                1. / (light.range * light.range)
            };
            rows.extend([
                [position[0], position[1], position[2], inv_range_sq],
                [red, green, blue, inner],
                [x, y, z, light.outer_angle_cos],
                [view[0], view[1], view[2], light.range],
            ]);
        }
        let bytes = std::mem::size_of_val(rows.as_slice());
        let grid_bytes =
            cells.iter().map(|&n| u64::from(n)).product::<u64>() * u64::from(words) * 4;
        if grid_bytes > renderer.storage_buffer_limit
            || bytes as u64 > renderer.storage_buffer_limit
        {
            return Err(VulkanError::SceneBufferLimit);
        }
        let (old_lights, old_grid) = reuse
            .map(|frame| {
                #[cfg(any(test, feature = "bench"))]
                let grid =
                    (frame.exhaustive == renderer.exhaustive_light_reference).then_some(frame.grid);
                #[cfg(not(any(test, feature = "bench")))]
                let grid = Some(frame.grid);
                (Some(frame.lights), grid)
            })
            .unwrap_or_default();
        let records = match old_lights.filter(|buffer| buffer.size >= bytes as u64) {
            Some(buffer) => buffer,
            None => renderer.buffer(bytes as u64, vk::BufferUsageFlags::STORAGE_BUFFER)?,
        };
        let grid = match old_grid.filter(|buffer| buffer.size >= grid_bytes) {
            Some(buffer) => buffer,
            #[cfg(any(test, feature = "bench"))]
            None if renderer.exhaustive_light_reference => {
                renderer.buffer(grid_bytes, vk::BufferUsageFlags::STORAGE_BUFFER)?
            }
            None => renderer.device_buffer(grid_bytes, vk::BufferUsageFlags::STORAGE_BUFFER)?,
        };
        // SAFETY: This slot has retired. The independent row data fits the
        // host-visible allocation and the GPU sees it after submission.
        unsafe {
            records
                .memory
                .as_ref()
                .unwrap()
                .access(records.size, |mapped| {
                    std::ptr::copy_nonoverlapping(rows.as_ptr().cast::<u8>(), mapped, bytes);
                })?;
        }
        #[cfg(any(test, feature = "bench"))]
        if renderer.exhaustive_light_reference && words != 0 {
            let mut mask = vec![u32::MAX; words as usize];
            let tail = local_count % 32;
            if tail != 0 {
                mask[words as usize - 1] = (1 << tail) - 1;
            }
            // SAFETY: The test buffer is host visible and its slot has retired.
            // Exactly one mask is written for every allocated cluster.
            unsafe {
                grid.memory.as_ref().unwrap().access(grid.size, |mapped| {
                    let len = std::mem::size_of_val(mask.as_slice());
                    for offset in (0..grid_bytes as usize).step_by(len) {
                        std::ptr::copy_nonoverlapping(
                            mask.as_ptr().cast::<u8>(),
                            mapped.add(offset),
                            len,
                        );
                    }
                })?;
            }
        }
        Ok(Self {
            lights: records,
            grid,
            cells,
            local_count,
            words,
            #[cfg(any(test, feature = "bench"))]
            exhaustive: renderer.exhaustive_light_reference,
        })
    }

    pub(super) fn descriptor_infos(&self) -> [vk::DescriptorBufferInfo; 2] {
        [&self.lights, &self.grid].map(|buffer| {
            vk::DescriptorBufferInfo::default()
                .buffer(buffer.buffer)
                .range(buffer.size)
        })
    }

    pub(super) fn local_count(&self) -> usize {
        self.local_count
    }

    pub(super) fn record(
        &self,
        device: &ash::Device,
        command: vk::CommandBuffer,
        pool: vk::DescriptorPool,
        pipeline: &Pipeline,
        camera: [f32; 32],
    ) -> Result<(), VulkanError> {
        if self.words == 0 {
            return Ok(());
        }
        #[cfg(any(test, feature = "bench"))]
        if self.exhaustive {
            return Ok(());
        }
        let [light_data, light_grid] = self.descriptor_infos();
        let infos = [light_data, light_grid];
        let push = camera.map(f32::to_ne_bytes);
        let bytes = push.as_flattened();
        // SAFETY: The frame slot retains the buffers, descriptor pool and
        // pipeline. The command buffer is open on the graphics/compute family.
        unsafe {
            let set = device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(pool)
                        .set_layouts(&[pipeline.layout.set]),
                )
                .map_err(VulkanError::api("allocate light cluster descriptors"))?[0];
            let writes = BINDINGS.map(|binding| {
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(binding)
                    .descriptor_type(STORAGE)
                    .buffer_info(&infos[(binding - BINDINGS[0]) as usize..][..1])
            });
            device.update_descriptor_sets(&writes, &[]);
            device.cmd_bind_pipeline(command, vk::PipelineBindPoint::COMPUTE, pipeline.pipeline);
            device.cmd_bind_descriptor_sets(
                command,
                vk::PipelineBindPoint::COMPUTE,
                pipeline.layout.raw,
                0,
                &[set],
                &[],
            );
            device.cmd_push_constants(
                command,
                pipeline.layout.raw,
                vk::ShaderStageFlags::COMPUTE,
                0,
                bytes,
            );
            device.cmd_dispatch(command, self.cells[0], self.cells[1], self.cells[2]);
            device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[vk::BufferMemoryBarrier::default()
                    .buffer(self.grid.buffer)
                    .size(self.grid.size)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)],
                &[],
            );
        }
        Ok(())
    }
}
