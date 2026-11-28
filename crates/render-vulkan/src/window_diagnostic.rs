//! Offscreen consumer for explicit GPU presentation verification.
use super::*;
use crate::frames::CommandPool;

impl VulkanRenderer {
    pub fn offscreen_target(&self) -> WindowTarget {
        WindowTarget {
            device: RenderDevice {
                context: self.context.clone(),
                physical: self.physical,
                capabilities: self.capabilities,
                timestamp_valid_bits: self.timestamp_valid_bits,
                meshlets: false,
            },
            scheduling: WindowScheduling::Default,
        }
    }

    pub fn graphics_device(&self) -> (&ash::Instance, vk::PhysicalDevice, &ash::Device) {
        (&self.instance, self.physical, &self.device)
    }

    /// Draw to a transparent RGBA8 attachment and read the completed pixels.
    ///
    /// # Safety
    /// `encode` must only record valid graphics commands inside the supplied
    /// rendering scope. `owner` must retain every referenced consumer resource.
    pub unsafe fn diagnostic_rgba<T>(
        &self,
        size: [u32; 2],
        owner: T,
        encode: impl FnOnce(&T, vk::CommandBuffer) -> Result<(), VulkanError>,
    ) -> Result<Vec<u8>, VulkanError> {
        let [width, height] = size;
        let bytes = u64::from(width) * u64::from(height);
        if bytes == 0 || bytes > 16_777_216 {
            return Err(VulkanError::Viewport);
        }
        let bytes = bytes * 4;
        let mut output = self.image(width, height, vk::Format::R8G8B8A8_UNORM, false)?;
        let staging = self.buffer(bytes, vk::BufferUsageFlags::TRANSFER_DST)?;
        let pool = CommandPool::new(&self.context, self.family)?;
        let fence = Completion::new(&self.context, None, 0)?;
        pool.begin()?;
        let command = pool.command;
        let extent = vk::Extent2D { width, height };
        transition(
            &self.device,
            command,
            &mut output,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        );
        let attachments = [vk::RenderingAttachmentInfo::default()
            .image_view(output.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue { float32: [0.; 4] },
            })];
        // SAFETY: The exclusive recording owns the fresh color target; the callback
        // receives this command only between begin/end rendering.
        unsafe {
            self.device.cmd_begin_rendering(
                command,
                &vk::RenderingInfo::default()
                    .render_area(vk::Rect2D::default().extent(extent))
                    .layer_count(1)
                    .color_attachments(&attachments),
            );
            let encoded = encode(&owner, command);
            self.device.cmd_end_rendering(command);
            encoded?;
        }
        transition(
            &self.device,
            command,
            &mut output,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::TRANSFER_READ,
        );
        let region = vk::BufferImageCopy::default()
            .image_subresource(
                vk::ImageSubresourceLayers::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1),
            )
            .image_extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            });
        // SAFETY: The tightly packed copy covers the complete fresh allocation.
        // Make transfer writes visible to the host before signaling the fence.
        unsafe {
            self.device.cmd_copy_image_to_buffer(
                command,
                output.image,
                output.layout,
                staging.buffer,
                &[region],
            );
            let barrier = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::HOST_READ)];
            self.device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::HOST,
                vk::DependencyFlags::empty(),
                &barrier,
                &[],
                &[],
            );
            self.device
                .end_command_buffer(command)
                .map_err(VulkanError::api("end offscreen diagnostic"))?;
        }
        fence
            .submit(self.queue, command, None)
            .map_err(VulkanError::api("submit offscreen diagnostic"))?;
        if let Err(err) = fence.wait() {
            std::mem::forget((owner, output, staging, pool, fence));
            return Err(VulkanError::api("wait offscreen diagnostic")(err));
        }
        let mut pixels = vec![0; bytes as usize];
        // SAFETY: Completion confirms the host barrier; pixels covers the exact mapped range.
        unsafe {
            staging.memory.as_ref().unwrap().access(bytes, |mapped| {
                std::ptr::copy_nonoverlapping(mapped, pixels.as_mut_ptr(), pixels.len());
            })?;
        }
        Ok(pixels)
    }
}
