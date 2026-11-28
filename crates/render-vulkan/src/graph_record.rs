//! Image dependencies for declared graph operations and their consumers.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Recorder<'a> {
    pub device: &'a ash::Device,
    pub command: vk::CommandBuffer,
    pub synchronization2: bool,
    pub compute_queue: bool,
}

impl Recorder<'_> {
    pub fn use_image(
        self,
        image: &mut Image,
        layout: vk::ImageLayout,
        stage: vk::PipelineStageFlags,
        access: vk::AccessFlags,
    ) {
        let Self {
            device,
            command,
            synchronization2,
            compute_queue,
        } = self;
        if !synchronization2 {
            transition(device, command, image, layout, stage, access);
            return;
        }
        let first = image.layout == vk::ImageLayout::UNDEFINED;
        let queue_change = !first && image.recorded_on_compute != compute_queue;
        let aliased = first && image.aliased;
        // An alias changes the image handle. Synchronize the whole backing
        // through a memory dependency before transitioning the new image.
        // SAFETY: Only a reserved slot's recording graph images reach this
        // code. A queue split orders both submissions with a timeline wait.
        unsafe {
            if aliased {
                let handover = [vk::MemoryBarrier2::default()
                    .src_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                    .src_access_mask(vk::AccessFlags2::MEMORY_WRITE)
                    .dst_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                    .dst_access_mask(
                        vk::AccessFlags2::MEMORY_READ | vk::AccessFlags2::MEMORY_WRITE,
                    )];
                device.cmd_pipeline_barrier2(
                    command,
                    &vk::DependencyInfo::default().memory_barriers(&handover),
                );
            }
            let src_stage = if first || queue_change {
                vk::PipelineStageFlags::ALL_COMMANDS
            } else {
                image.stage
            };
            let src_access = if queue_change || aliased {
                vk::AccessFlags::empty()
            } else if first {
                vk::AccessFlags::MEMORY_WRITE
            } else {
                image.access
            };
            let barrier = [vk::ImageMemoryBarrier2::default()
                .src_stage_mask(stage2(src_stage))
                .src_access_mask(access2(src_access))
                .dst_stage_mask(stage2(stage))
                .dst_access_mask(access2(access))
                .old_layout(image.layout)
                .new_layout(layout)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image.image)
                .subresource_range(range(image.aspect))];
            device.cmd_pipeline_barrier2(
                command,
                &vk::DependencyInfo::default().image_memory_barriers(&barrier),
            );
        }
        // Include earlier readers and writes on this queue when a later use
        // changes stages or overwrites the image. The queue wait covers the
        // earlier batch when the next use is on the async queue.
        if first || queue_change {
            image.stage = stage;
            image.access = access;
        } else {
            image.stage |= stage;
            image.access |= access;
        }
        image.layout = layout;
        image.recorded_on_compute = compute_queue;
    }

    pub fn copy_to_host(
        self,
        image: &mut Image,
        staging: &Buffer,
        plane: &ReadbackImage,
        width: u32,
        height: u32,
    ) {
        self.use_image(
            image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::TRANSFER_READ,
        );
        let region = vk::BufferImageCopy::default()
            .buffer_offset(plane.offset as u64)
            .image_subresource(
                vk::ImageSubresourceLayers::default()
                    .aspect_mask(image.aspect)
                    .layer_count(1),
            )
            .image_extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            });
        // SAFETY: The checked plane fits the slot's retained staging buffer.
        // The complete image mip was just transitioned for transfer reads.
        unsafe {
            self.device.cmd_copy_image_to_buffer(
                self.command,
                image.image,
                image.layout,
                staging.buffer,
                &[region],
            );
        }
    }

    pub fn finish_host_read(self, staging: &Buffer) {
        let Self {
            device,
            command,
            synchronization2,
            ..
        } = self;
        // SAFETY: The staging buffer belongs to the frame slot. Copies
        // recorded on this queue must become visible before the CPU maps it.
        unsafe {
            if synchronization2 {
                let barrier = [vk::BufferMemoryBarrier2::default()
                    .src_stage_mask(vk::PipelineStageFlags2::COPY)
                    .src_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
                    .dst_stage_mask(vk::PipelineStageFlags2::HOST)
                    .dst_access_mask(vk::AccessFlags2::HOST_READ)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .buffer(staging.buffer)
                    .size(staging.size)];
                device.cmd_pipeline_barrier2(
                    command,
                    &vk::DependencyInfo::default().buffer_memory_barriers(&barrier),
                );
            } else {
                let barrier = [vk::BufferMemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::HOST_READ)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .buffer(staging.buffer)
                    .size(staging.size)];
                device.cmd_pipeline_barrier(
                    command,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::HOST,
                    vk::DependencyFlags::empty(),
                    &[],
                    &barrier,
                    &[],
                );
            }
        }
    }
}

// The graph uses the core stage/access flags that retain their bit values in
// synchronization2. The zero stage is used for the release before a semaphore.
fn stage2(stage: vk::PipelineStageFlags) -> vk::PipelineStageFlags2 {
    vk::PipelineStageFlags2::from_raw(u64::from(stage.as_raw()))
}

fn access2(access: vk::AccessFlags) -> vk::AccessFlags2 {
    vk::AccessFlags2::from_raw(u64::from(access.as_raw()))
}
