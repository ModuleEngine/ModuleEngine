use crate::{VulkanError, presentation::DeviceLifetime};
use ash::vk;
use std::sync::Arc;

const COMPLETION_TIMEOUT_NS: u64 = 30_000_000_000;

pub(crate) struct CommandPool {
    pool: vk::CommandPool,
    pub command: vk::CommandBuffer,
    context: Arc<DeviceLifetime>,
}

impl CommandPool {
    pub fn new(context: &Arc<DeviceLifetime>, family: u32) -> Result<Self, VulkanError> {
        let device = &context.device;
        unsafe {
            let pool = device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(family)
                        .flags(vk::CommandPoolCreateFlags::TRANSIENT),
                    None,
                )
                .map_err(VulkanError::api("create frame command pool"))?;
            let mut result = Self {
                pool,
                command: vk::CommandBuffer::null(),
                context: context.clone(),
            };
            result.command = device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .map_err(VulkanError::api("allocate frame command buffer"))?[0];
            Ok(result)
        }
    }

    /// The caller acquired a free slot and confirmed the previous command's completion.
    pub fn begin(&self) -> Result<(), VulkanError> {
        unsafe {
            let device = &self.context.device;
            device
                .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
                .map_err(VulkanError::api("reset frame command pool"))?;
            device
                .begin_command_buffer(
                    self.command,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(VulkanError::api("begin frame command buffer"))
        }
    }
}

impl Drop for CommandPool {
    fn drop(&mut self) {
        unsafe { self.context.device.destroy_command_pool(self.pool, None) };
    }
}

pub(crate) struct FramePools {
    pub graphics: CommandPool,
    pub transfer: Option<CommandPool>,
    pub descriptors: vk::DescriptorPool,
    context: Arc<DeviceLifetime>,
}

impl FramePools {
    pub fn new(
        context: &Arc<DeviceLifetime>,
        graphics_family: u32,
        transfer_family: Option<u32>,
    ) -> Result<Self, VulkanError> {
        let graphics = CommandPool::new(context, graphics_family)?;
        let transfer = transfer_family
            .map(|family| CommandPool::new(context, family))
            .transpose()?;
        let sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 256,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLED_IMAGE,
                descriptor_count: 1024,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_IMAGE,
                descriptor_count: 1024,
            },
        ];
        let descriptors = unsafe {
            context.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(128)
                    .pool_sizes(&sizes),
                None,
            )
        }
        .map_err(VulkanError::api("create frame descriptor pool"))?;
        Ok(Self {
            graphics,
            transfer,
            descriptors,
            context: context.clone(),
        })
    }
}

impl Drop for FramePools {
    fn drop(&mut self) {
        unsafe {
            self.context
                .device
                .destroy_descriptor_pool(self.descriptors, None)
        };
    }
}

pub(crate) struct Timeline {
    raw: vk::Semaphore,
    context: Arc<DeviceLifetime>,
}

impl Timeline {
    pub fn new(context: &Arc<DeviceLifetime>, name: &str) -> Result<Arc<Self>, VulkanError> {
        let mut kind =
            vk::SemaphoreTypeCreateInfo::default().semaphore_type(vk::SemaphoreType::TIMELINE);
        let raw = unsafe {
            context.device.create_semaphore(
                &vk::SemaphoreCreateInfo::default().push_next(&mut kind),
                None,
            )
        }
        .map_err(VulkanError::api("create device timeline"))?;
        context.name(raw, || name.to_owned());
        Ok(Arc::new(Self {
            raw,
            context: context.clone(),
        }))
    }

    pub fn counter(&self) -> Result<u64, vk::Result> {
        unsafe { self.context.device.get_semaphore_counter_value(self.raw) }
    }

    fn point(&self, value: u64) -> vk::SemaphoreSubmitInfo<'static> {
        vk::SemaphoreSubmitInfo::default()
            .semaphore(self.raw)
            .value(value)
            .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
    }

    pub fn submit(
        &self,
        queue: vk::Queue,
        command: vk::CommandBuffer,
        value: u64,
        wait: Option<&Timeline>,
    ) -> Result<(), vk::Result> {
        let wait_info = wait.map(|timeline| timeline.point(value));
        let signal = [self.point(value)];
        let command = [vk::CommandBufferSubmitInfo::default().command_buffer(command)];
        let submit = vk::SubmitInfo2::default()
            .wait_semaphore_infos(wait_info.as_slice())
            .command_buffer_infos(&command)
            .signal_semaphore_infos(&signal);
        unsafe {
            self.context
                .device
                .queue_submit2(queue, &[submit], vk::Fence::null())
        }
    }
}

impl Drop for Timeline {
    fn drop(&mut self) {
        unsafe { self.context.device.destroy_semaphore(self.raw, None) };
    }
}

pub(crate) enum Completion {
    Timeline {
        timeline: Arc<Timeline>,
        value: u64,
    },
    Fence {
        context: Arc<DeviceLifetime>,
        fence: vk::Fence,
    },
}

impl Completion {
    pub fn new(
        context: &Arc<DeviceLifetime>,
        timeline: Option<&Arc<Timeline>>,
        value: u64,
    ) -> Result<Self, VulkanError> {
        if let Some(timeline) = timeline {
            return Ok(Self::Timeline {
                timeline: timeline.clone(),
                value,
            });
        }
        let fence = unsafe { context.device.create_fence(&Default::default(), None) }
            .map_err(VulkanError::api("create producer fence"))?;
        Ok(Self::Fence {
            context: context.clone(),
            fence,
        })
    }

    pub fn submit(
        &self,
        queue: vk::Queue,
        command: vk::CommandBuffer,
        wait: Option<&Timeline>,
    ) -> Result<(), vk::Result> {
        match self {
            Self::Timeline { timeline, value } => timeline.submit(queue, command, *value, wait),
            Self::Fence { context, fence } => {
                debug_assert!(wait.is_none());
                let commands = [command];
                let submit = vk::SubmitInfo::default().command_buffers(&commands);
                unsafe { context.device.queue_submit(queue, &[submit], *fence) }
            }
        }
    }

    pub fn ready(&self) -> Result<bool, vk::Result> {
        match self {
            Self::Timeline { timeline, value } => {
                timeline.counter().map(|completed| completed >= *value)
            }
            Self::Fence { context, fence } => unsafe { context.device.get_fence_status(*fence) },
        }
    }

    pub fn wait(&self) -> Result<(), vk::Result> {
        unsafe {
            match self {
                Self::Timeline { timeline, value } => timeline.context.device.wait_semaphores(
                    &vk::SemaphoreWaitInfo::default()
                        .semaphores(&[timeline.raw])
                        .values(&[*value]),
                    COMPLETION_TIMEOUT_NS,
                ),
                Self::Fence { context, fence } => {
                    context
                        .device
                        .wait_for_fences(&[*fence], true, COMPLETION_TIMEOUT_NS)
                }
            }
        }
    }
}

impl Drop for Completion {
    fn drop(&mut self) {
        if let Self::Fence { context, fence } = self {
            unsafe { context.device.destroy_fence(*fence, None) };
        }
    }
}
