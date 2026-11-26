use crate::{VulkanError, presentation::DeviceLifetime};
use ash::vk;
use gpu_allocator::{
    MemoryLocation,
    vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator, AllocatorCreateDesc},
};
use std::sync::{Arc, Mutex};

pub(crate) struct DeviceAllocator {
    allocator: Mutex<Allocator>,
    _device: Arc<DeviceLifetime>,
}

impl DeviceAllocator {
    pub(crate) fn new(
        context: &Arc<DeviceLifetime>,
        instance: &ash::Instance,
        physical: vk::PhysicalDevice,
    ) -> Result<Arc<Self>, VulkanError> {
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: context.device.clone(),
            physical_device: physical,
            debug_settings: Default::default(),
            buffer_device_address: false,
            allocation_sizes: Default::default(),
        })
        .map_err(|error| VulkanError::Resource(format!("Create Vulkan allocator: {error}")))?;
        Ok(Arc::new(Self {
            allocator: Mutex::new(allocator),
            _device: context.clone(),
        }))
    }

    pub(crate) fn allocate(
        self: &Arc<Self>,
        name: &str,
        requirements: vk::MemoryRequirements,
        linear: bool,
        scheme: AllocationScheme,
    ) -> Result<ResourceMemory, VulkanError> {
        let allocation = self
            .allocator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .allocate(&AllocationCreateDesc {
                name,
                requirements,
                location: MemoryLocation::Unknown,
                linear,
                allocation_scheme: scheme,
            })
            .map_err(|error| VulkanError::Resource(format!("Allocate Vulkan {name}: {error}")))?;
        Ok(ResourceMemory::Suballocated {
            allocation: Some(allocation),
            owner: self.clone(),
        })
    }

    pub(crate) fn stats(&self) -> [u64; 3] {
        let report = self
            .allocator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generate_report();
        [
            report.blocks.len() as u64,
            report.total_allocated_bytes,
            report.total_capacity_bytes,
        ]
    }
}

pub(crate) enum ResourceMemory {
    Standalone(Arc<DeviceLifetime>, vk::DeviceMemory),
    Suballocated {
        allocation: Option<Allocation>,
        owner: Arc<DeviceAllocator>,
    },
}

impl ResourceMemory {
    pub(crate) fn binding(&self) -> (vk::DeviceMemory, u64) {
        match self {
            Self::Standalone(_, memory) => (*memory, 0),
            Self::Suballocated { allocation, .. } => {
                let allocation = allocation.as_ref().unwrap();
                // The allocation owns this binding range until the resource drops.
                // SAFETY: The allocation is live and owns this subrange until its resource is
                // destroyed.
                (unsafe { allocation.memory() }, allocation.offset())
            }
        }
    }

    /// The renderer only accesses a mapped range before submission or after
    /// confirming completion of the slot that owns it.
    pub(crate) unsafe fn access(
        &self,
        size: u64,
        copy: impl FnOnce(*mut u8),
    ) -> Result<(), VulkanError> {
        match self {
            // SAFETY: The caller excludes GPU and host conflicts in this host-visible range. The
            // mapping is checked before invoking the copy and is released immediately afterwards.
            Self::Standalone(context, memory) => unsafe {
                let device = &context.device;
                let mapped = device
                    .map_memory(*memory, 0, size, vk::MemoryMapFlags::empty())
                    .map_err(VulkanError::api("map buffer"))?;
                copy(mapped.cast());
                device.unmap_memory(*memory);
            },
            Self::Suballocated { allocation, .. } => {
                let mapped = allocation
                    .as_ref()
                    .unwrap()
                    .mapped_ptr()
                    .ok_or_else(|| VulkanError::Resource("Buffer is not host mapped".into()))?;
                copy(mapped.as_ptr().cast());
            }
        }
        Ok(())
    }
}

impl Drop for ResourceMemory {
    fn drop(&mut self) {
        match self {
            // SAFETY: The owning resource has destroyed its binding and completed GPU use before
            // dropping the allocation.
            Self::Standalone(context, memory) => unsafe {
                context.device.free_memory(*memory, None);
            },
            Self::Suballocated { allocation, owner } => {
                if let Err(error) = owner
                    .allocator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .free(allocation.take().unwrap())
                {
                    eprintln!("Vulkan allocator could not free allocation: {error}");
                }
            }
        }
    }
}
