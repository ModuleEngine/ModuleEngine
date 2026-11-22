use crate::{
    VulkanError,
    frames::{CommandPool, Timeline},
    presentation::DeviceLifetime,
};
use ash::vk;
use std::sync::Arc;

pub(crate) struct TransferQueue {
    pub family: u32,
    pub queue: vk::Queue,
    pub timeline: Arc<Timeline>,
}

impl TransferQueue {
    pub fn new(context: &Arc<DeviceLifetime>, family: u32) -> Result<Self, VulkanError> {
        let queue = unsafe { context.device.get_device_queue(family, 0) };
        context.name(queue, || format!("ModuleEngine transfer queue {family}"));
        let timeline = Timeline::new(context, "ModuleEngine transfer timeline")?;
        Ok(Self {
            family,
            queue,
            timeline,
        })
    }
}

/// One whole atlas subresource is compatible even with a transfer-only family
/// that requires whole-subresource copies.
pub(crate) fn record_upload(
    device: &ash::Device,
    pool: &CommandPool,
    graphics: vk::CommandBuffer,
    source: vk::Buffer,
    image: vk::Image,
    region: vk::BufferImageCopy,
    [transfer_family, graphics_family]: [u32; 2],
) -> Result<(), VulkanError> {
    let subresource = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .level_count(1)
        .layer_count(1);
    let prepare = vk::ImageMemoryBarrier2::default()
        .image(image)
        .subresource_range(subresource)
        .old_layout(vk::ImageLayout::UNDEFINED)
        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_stage_mask(vk::PipelineStageFlags2::COPY)
        .dst_access_mask(vk::AccessFlags2::TRANSFER_WRITE);
    let ownership = vk::ImageMemoryBarrier2::default()
        .image(image)
        .subresource_range(subresource)
        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .new_layout(vk::ImageLayout::GENERAL)
        .src_queue_family_index(transfer_family)
        .dst_queue_family_index(graphics_family);
    let release = ownership
        .src_stage_mask(vk::PipelineStageFlags2::COPY)
        .src_access_mask(vk::AccessFlags2::TRANSFER_WRITE);
    let acquire = ownership
        .dst_stage_mask(vk::PipelineStageFlags2::FRAGMENT_SHADER)
        .dst_access_mask(vk::AccessFlags2::SHADER_SAMPLED_READ);
    pool.begin()?;
    unsafe {
        device.cmd_pipeline_barrier2(
            pool.command,
            &vk::DependencyInfo::default().image_memory_barriers(&[prepare]),
        );
        device.cmd_copy_buffer_to_image(
            pool.command,
            source,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[region],
        );
        device.cmd_pipeline_barrier2(
            pool.command,
            &vk::DependencyInfo::default().image_memory_barriers(&[release]),
        );
        device
            .end_command_buffer(pool.command)
            .map_err(VulkanError::api("end atlas transfer command buffer"))?;
        device.cmd_pipeline_barrier2(
            graphics,
            &vk::DependencyInfo::default().image_memory_barriers(&[acquire]),
        );
    }
    Ok(())
}
