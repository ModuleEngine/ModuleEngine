//! Window-specific setup for the R3.1 spike. R2 owns the instance and device.
use super::{InstanceLifetime, VulkanRenderer};
use crate::{DeviceCapabilities, VulkanError};
use ash::vk;
use std::{path::PathBuf, sync::Arc};

pub(super) struct Surface {
    instance: Arc<InstanceLifetime>,
    loader: ash::khr::surface::Instance,
    raw: vk::SurfaceKHR,
}

impl Surface {
    pub(super) fn queue_family(
        &self,
        physical: vk::PhysicalDevice,
        families: &[vk::QueueFamilyProperties],
    ) -> Result<Option<usize>, VulkanError> {
        for (index, properties) in families.iter().enumerate() {
            if properties.queue_count == 0
                || !properties
                    .queue_flags
                    .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
            {
                continue;
            }
            // SAFETY: The candidate and surface belong to this retained instance, and the
            // index comes from the candidate's enumerated queue families.
            let supported = unsafe {
                self.loader
                    .get_physical_device_surface_support(physical, index as u32, self.raw)
            }
            .map_err(VulkanError::api("Win32 surface support"))?;
            if supported {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: The surface retains its instance. EditorDevice's caller must destroy any
        // swapchain made from the borrowed handles before releasing this owner.
        unsafe { self.loader.destroy_surface(self.raw, None) };
    }
}

pub struct EditorDevice {
    renderer: VulkanRenderer,
    surface: Surface,
}

pub struct EditorDeviceHandles<'a> {
    pub instance: &'a ash::Instance,
    pub device: &'a ash::Device,
    pub physical: vk::PhysicalDevice,
    pub queue: vk::Queue,
    pub family: u32,
    pub surface_loader: &'a ash::khr::surface::Instance,
    pub surface: vk::SurfaceKHR,
    pub capabilities: &'a DeviceCapabilities,
    pub producer_slots: usize,
}

impl EditorDevice {
    /// Create a Win32 surface and initialize the actual R2 renderer against it.
    ///
    /// # Safety
    ///
    /// The Win32 handles must refer to a live window owned by the calling process.
    /// Keep the window alive until this owner is dropped. Retire all GPU work and
    /// destroy any resources created from the borrowed handles before dropping it.
    pub unsafe fn new(
        resource_root: PathBuf,
        hinstance: vk::HINSTANCE,
        hwnd: vk::HWND,
    ) -> Result<Self, VulkanError> {
        let instance = Arc::new(InstanceLifetime::new(&[
            ash::khr::surface::NAME,
            ash::khr::win32_surface::NAME,
        ])?);
        let win32 = ash::khr::win32_surface::Instance::new(instance.entry(), &instance.raw);
        // SAFETY: Both instance extensions are enabled and the caller keeps the Win32
        // window live for the surface's lifetime.
        let raw = unsafe {
            win32.create_win32_surface(
                &vk::Win32SurfaceCreateInfoKHR::default()
                    .hinstance(hinstance)
                    .hwnd(hwnd),
                None,
            )
        }
        .map_err(VulkanError::api("create Win32 surface"))?;
        let surface = Surface {
            loader: ash::khr::surface::Instance::new(instance.entry(), &instance.raw),
            instance: instance.clone(),
            raw,
        };
        let renderer =
            VulkanRenderer::new_with_instance(resource_root, None, instance, Some(&surface))?;
        Ok(Self { renderer, surface })
    }

    pub fn handles(&self) -> EditorDeviceHandles<'_> {
        EditorDeviceHandles {
            instance: &self.surface.instance.raw,
            device: &self.renderer.device,
            physical: self.renderer.physical,
            queue: self.renderer.queue,
            family: self.renderer.family,
            surface_loader: &self.surface.loader,
            surface: self.surface.raw,
            capabilities: &self.renderer.capabilities,
            producer_slots: self.renderer.slots.len(),
        }
    }
}
