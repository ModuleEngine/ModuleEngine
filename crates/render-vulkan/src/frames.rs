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
        // SAFETY: family is a queue family enabled on this device. The pool is fresh and kept alive
        // by CommandPool while its primary buffer exists.
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
        // SAFETY: The slot is exclusively reserved and its last flight has completed. Reset and
        // begin use the buffer from this pool on the same device.
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
        // SAFETY: The slot is unused or its flight completed. Unknown in-flight status retains the
        // whole owner in retirement instead of dropping this pool.
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
                // Up to 64 graph passes with five resident bindings each,
                // plus the classification, reduction and depth seed sets.
                descriptor_count: 336,
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
        // SAFETY: The pool is new on this device, and the fixed descriptor sizes stay live during
        // creation.
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
        // SAFETY: No flight using this slot's descriptor sets remains pending when its pools are
        // released.
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
        // SAFETY: The device has timeline semaphores enabled. The semaphore type chain lives
        // through creation.
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
        // SAFETY: This Arc-owned semaphore was created as a timeline on the retained device.
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
        let _queue = self
            .context
            .queue_access
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        // SAFETY: The lock serializes submissions to the selected queue. The buffer is
        // executable for that queue family, the signal value increases, and the flight retains the
        // command pool and timelines through completion.
        unsafe {
            self.context
                .device
                .queue_submit2(queue, &[submit], vk::Fence::null())
        }
    }
}

impl Drop for Timeline {
    fn drop(&mut self) {
        // SAFETY: The final Arc is released after all flights using this timeline complete or
        // before any submission.
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
        // SAFETY: The retained device is live and the default create info makes a fresh unsignaled
        // fence.
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
                let _queue = context
                    .queue_access
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                // SAFETY: The renderer exclusively submits the executable buffer to its own queue.
                // This completion owns a fresh unsignaled fence retained with the flight.
                unsafe { context.device.queue_submit(queue, &[submit], *fence) }
            }
        }
    }

    pub fn ready(&self) -> Result<bool, vk::Result> {
        match self {
            Self::Timeline { timeline, value } => {
                timeline.counter().map(|completed| completed >= *value)
            }
            // SAFETY: The fence is owned by this completion on the same retained device.
            Self::Fence { context, fence } => unsafe { context.device.get_fence_status(*fence) },
        }
    }

    pub fn wait(&self) -> Result<(), vk::Result> {
        // SAFETY: The chosen completion owns a live fence or timeline on its retained device. The
        // one-element wait arrays live through the call.
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
            // SAFETY: The fence is uniquely owned. The flight is dropped only before submission or
            // after confirmed completion.
            unsafe { context.device.destroy_fence(*fence, None) };
        }
    }
}
