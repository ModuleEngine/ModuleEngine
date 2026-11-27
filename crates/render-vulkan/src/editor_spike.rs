//! Native Vulkan surfaces for the shared Editor and Player graphics device.
use super::{InstanceLifetime, VulkanRenderer};
use crate::{DeviceCapabilities, VulkanError};
use ash::vk;
use std::{path::PathBuf, sync::Arc};

pub struct Surface {
    instance: Arc<InstanceLifetime>,
    loader: ash::khr::surface::Instance,
    raw: vk::SurfaceKHR,
}

impl Surface {
    unsafe fn new(
        instance: Arc<InstanceLifetime>,
        display: raw_window_handle::RawDisplayHandle,
        window: raw_window_handle::RawWindowHandle,
    ) -> Result<Self, VulkanError> {
        // SAFETY: The caller retains the matching display and window. The
        // instance enables the platform extensions selected for this display.
        let raw = unsafe {
            ash_window::create_surface(instance.entry(), &instance.raw, display, window, None)
        }
        .map_err(VulkanError::api("create native surface"))?;
        Ok(Self {
            loader: ash::khr::surface::Instance::new(instance.entry(), &instance.raw),
            instance,
            raw,
        })
    }

    pub fn handle(&self) -> vk::SurfaceKHR {
        self.raw
    }

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
            .map_err(VulkanError::api("native surface support"))?;
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
    meshlets: bool,
    renderer: VulkanRenderer,
    surface: Surface,
}

pub struct EditorDeviceHandles<'a> {
    pub queue_access: &'a std::sync::Mutex<()>,
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
    /// Initialize the R2 renderer for a native window.
    ///
    /// # Safety
    ///
    /// The handles must describe the same live window and remain valid until
    /// this owner drops. Retire GPU work and destroy borrowed resources first.
    pub unsafe fn for_window(
        resource_root: PathBuf,
        display: raw_window_handle::RawDisplayHandle,
        window: raw_window_handle::RawWindowHandle,
        meshlets: bool,
    ) -> Result<Self, VulkanError> {
        let required = ash_window::enumerate_required_extensions(display)
            .map_err(VulkanError::api("enumerate native surface extensions"))?;
        let extensions = required
            .iter()
            .map(|&name| {
                // SAFETY: ash-window returns static Vulkan extension names.
                unsafe { std::ffi::CStr::from_ptr(name) }
            })
            .collect::<Vec<_>>();
        let instance = Arc::new(InstanceLifetime::new(&extensions)?);
        // SAFETY: The display and window obey this constructor's lifetime contract.
        let surface = unsafe { Surface::new(instance.clone(), display, window) }?;
        let renderer =
            VulkanRenderer::new_with_instance(resource_root, instance, meshlets, Some(&surface))?;
        Ok(Self {
            renderer,
            surface,
            meshlets,
        })
    }

    /// Attach a second window on the same display to the root's presentation device.
    ///
    /// # Safety
    /// Keep the supplied window and display live until the surface drops, and
    /// destroy its swapchain before dropping the surface.
    pub unsafe fn attach_window(
        &self,
        display: raw_window_handle::RawDisplayHandle,
        window: raw_window_handle::RawWindowHandle,
    ) -> Result<Surface, VulkanError> {
        // SAFETY: The caller supplies a sibling window on the original display.
        let surface = unsafe { Surface::new(self.surface.instance.clone(), display, window) }?;
        // SAFETY: Surface and physical device belong to the same live instance.
        let supported = unsafe {
            surface.loader.get_physical_device_surface_support(
                self.renderer.physical,
                self.renderer.family,
                surface.raw,
            )
        }
        .map_err(VulkanError::api("query detached surface support"))?;
        if !supported {
            return Err(VulkanError::Presentation(
                "Root queue cannot present to the detached window".into(),
            ));
        }
        Ok(surface)
    }

    pub fn target(&self) -> super::WindowTarget {
        super::WindowTarget(super::RenderDevice {
            meshlets: self.meshlets,
            context: self.renderer.context.clone(),
            physical: self.renderer.physical,
            capabilities: self.renderer.capabilities,
            timestamp_valid_bits: self.renderer.timestamp_valid_bits,
        })
    }

    pub fn handles(&self) -> EditorDeviceHandles<'_> {
        EditorDeviceHandles {
            queue_access: &self.renderer.context.queue_access,
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
