//! GPU presentation exchange with a separately owned native GUI device.
//!
//! Allocations are dedicated RGBA8_UNORM optimal images, one mip/layer/sample,
//! with no create flags and TRANSFER_SRC | TRANSFER_DST | SAMPLED usage. A newly
//! allocated source is uninitialized: the producer performs the first write.
//! Every subsequent handoff uses EXTERNAL ownership and GENERAL layout. The
//! producer confirms submission completion before publishing a frame. Consumers must retain
//! the entire `Frame::gpu` lease until their GPU work has completed, and release
//! the image to EXTERNAL/GENERAL before allowing that lease to drop.
use crate::VulkanError;
use ash::vk;
use instance::InstanceLifetime;
use module_engine_core::{GpuFrame, PresentationTarget};
use std::{
    any::Any,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdapterIdentity {
    WindowsLuid([u8; 8]),
    VulkanUuid { device: [u8; 16], driver: [u8; 16] },
}

/// Each allocation exports a fresh owned handle. Vulkan consumes an FD only
/// when allocation succeeds; Win32 NT handles remain caller-owned on import.
pub enum SharedMemoryHandle {
    #[cfg(windows)]
    Win32(std::os::windows::io::OwnedHandle),
    #[cfg(unix)]
    OpaqueFd(std::os::fd::OwnedFd),
}

pub struct SharedImageInfo {
    pub handle: SharedMemoryHandle,
    /// Exact exporter allocation size for OPAQUE_FD. The D3D12 committed
    /// resource's native size is informational: Vulkan imports query the OS.
    pub allocation_size: u64,
    /// Linux must supply exactly the exported allocation's memory type bit.
    /// Win32 may supply zero; native handle properties determine compatibility.
    pub memory_type_bits: u32,
    pub payload: Arc<dyn GpuFrame>,
}

pub trait SharedImageAllocator: Send + Sync {
    fn allocate(&self, width: u32, height: u32) -> Result<SharedImageInfo, String>;
}

pub struct VulkanPresentationTarget {
    pub identity: AdapterIdentity,
    pub allocator: Arc<dyn SharedImageAllocator>,
}
impl PresentationTarget for VulkanPresentationTarget {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The downcast belongs to the GUI payload, while this owner retains both the
/// producer allocation and its Vulkan context through actual consumer completion.
pub struct VulkanGpuFrame {
    pub payload: Arc<dyn GpuFrame>,
    _lease: Arc<dyn Any + Send + Sync>,
}
impl fmt::Debug for VulkanGpuFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VulkanGpuFrame")
            .field("payload", &self.payload)
            .finish_non_exhaustive()
    }
}
impl GpuFrame for VulkanGpuFrame {
    fn as_any(&self) -> &dyn Any {
        self.payload.as_any()
    }
}

pub(crate) struct DeviceLifetime {
    pub instance: Arc<InstanceLifetime>,
    pub device: ash::Device,
}
impl DeviceLifetime {
    pub(crate) fn name(&self, _object: impl vk::Handle, _name: impl FnOnce() -> String) {
        #[cfg(debug_assertions)]
        self.instance.name(&self.device, _object, &_name());
    }
}

impl Drop for DeviceLifetime {
    fn drop(&mut self) {
        unsafe {
            // Every producer submission is confirmed before its resources drop,
            // or its entire owner/device is quarantined on an unknown result.
            self.device.destroy_device(None);
        }
    }
}

pub(crate) mod instance {
    use crate::VulkanError;
    use ash::{Entry, vk};
    use std::{ffi::CStr, sync::Arc};

    pub(crate) struct InstanceLifetime {
        // Keep the Vulkan loader mapped until after vkDestroyInstance.
        _entry: Entry,
        pub raw: ash::Instance,
        #[cfg(debug_assertions)]
        debug: debug::Diagnostics,
    }

    impl InstanceLifetime {
        pub(crate) fn new(extensions: &[&CStr]) -> Result<Self, VulkanError> {
            // The owner retains the dynamically loaded entry until instance destruction.
            let entry =
                unsafe { Entry::load() }.map_err(|source| VulkanError::Loader(Arc::new(source)))?;
            let app = vk::ApplicationInfo::default()
                .application_name(c"ModuleEngine GPU graph")
                .api_version(vk::API_VERSION_1_3);
            #[cfg(debug_assertions)]
            let (raw, debug) = debug::Diagnostics::create(&entry, &app, extensions)?;
            #[cfg(not(debug_assertions))]
            let extensions = extensions
                .iter()
                .map(|name| name.as_ptr())
                .collect::<Vec<_>>();
            #[cfg(not(debug_assertions))]
            // All application and extension names live through the create call.
            let raw = unsafe {
                entry.create_instance(
                    &vk::InstanceCreateInfo::default()
                        .application_info(&app)
                        .enabled_extension_names(&extensions),
                    None,
                )
            }
            .map_err(VulkanError::api("1.3 instance"))?;

            Ok(Self {
                _entry: entry,
                raw,
                #[cfg(debug_assertions)]
                debug,
            })
        }

        #[cfg(all(windows, feature = "editor-spike"))]
        pub(crate) fn entry(&self) -> &Entry {
            &self._entry
        }

        #[cfg(debug_assertions)]
        pub(crate) fn name(&self, device: &ash::Device, object: impl vk::Handle, name: &str) {
            self.debug.name(&self.raw, device, object, name);
        }
    }

    impl Drop for InstanceLifetime {
        fn drop(&mut self) {
            #[cfg(debug_assertions)]
            self.debug.destroy();
            unsafe { self.raw.destroy_instance(None) };
            #[cfg(debug_assertions)]
            self.debug.report();
        }
    }

    #[cfg(debug_assertions)]
    mod debug {
        use super::*;
        use std::{
            ffi::{CStr, CString, c_void},
            io::Write,
            sync::atomic::{AtomicU64, Ordering::Relaxed},
        };

        const LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";
        static ERRORS: AtomicU64 = AtomicU64::new(0);

        #[derive(Default)]
        struct Counts {
            warnings: AtomicU64,
            errors: AtomicU64,
            named: AtomicU64,
        }

        pub(super) struct Diagnostics {
            counts: Box<Counts>,
            validation: bool,
            messenger: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
        }

        fn log(args: std::fmt::Arguments<'_>) {
            let _ = writeln!(std::io::stderr().lock(), "{args}");
        }

        unsafe extern "system" fn callback(
            severity: vk::DebugUtilsMessageSeverityFlagsEXT,
            _: vk::DebugUtilsMessageTypeFlagsEXT,
            data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
            user: *mut c_void,
        ) -> vk::Bool32 {
            let counts = unsafe { &*user.cast::<Counts>() };
            if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
                counts.errors.fetch_add(1, Relaxed);
                ERRORS.fetch_add(1, Relaxed);
            } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
                counts.warnings.fetch_add(1, Relaxed);
            }
            let message = unsafe { CStr::from_ptr((*data).p_message) }.to_string_lossy();
            log(format_args!("Vulkan validation {severity:?}: {message}"));
            vk::FALSE
        }

        pub(super) fn error_count() -> u64 {
            ERRORS.load(Relaxed)
        }

        impl Diagnostics {
            pub(super) fn create(
                entry: &Entry,
                app: &vk::ApplicationInfo<'_>,
                requested_extensions: &[&CStr],
            ) -> Result<(ash::Instance, Self), VulkanError> {
                let layers = unsafe { entry.enumerate_instance_layer_properties() }
                    .map_err(VulkanError::api("enumerate instance layers"))?;
                let extensions = unsafe { entry.enumerate_instance_extension_properties(None) }
                    .map_err(VulkanError::api("enumerate instance extensions"))?;
                let validation = layers
                    .iter()
                    .any(|layer| unsafe { CStr::from_ptr(layer.layer_name.as_ptr()) == LAYER });
                let utils = extensions.iter().any(|extension| unsafe {
                    CStr::from_ptr(extension.extension_name.as_ptr()) == ash::ext::debug_utils::NAME
                });
                let counts = Box::<Counts>::default();
                let layer_names = [LAYER.as_ptr()];
                let mut severity = vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;
                // The native spike records the validation layer's create-time status.
                if requested_extensions.contains(&ash::khr::win32_surface::NAME) {
                    severity |= vk::DebugUtilsMessageSeverityFlagsEXT::INFO;
                }
                let mut extension_names = requested_extensions
                    .iter()
                    .map(|name| name.as_ptr())
                    .collect::<Vec<_>>();
                if utils {
                    extension_names.push(ash::ext::debug_utils::NAME.as_ptr());
                }
                let callback_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                    .message_severity(severity)
                    .message_type(
                        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                    )
                    .pfn_user_callback(Some(callback))
                    .user_data(std::ptr::from_ref(counts.as_ref()).cast_mut().cast());
                let mut info = vk::InstanceCreateInfo::default()
                    .application_info(app)
                    .enabled_extension_names(&extension_names);
                if validation {
                    info = info.enabled_layer_names(&layer_names);
                }
                let instance = if utils {
                    let mut instance_callback = callback_info;
                    // The callback data, layers and extensions live through creation.
                    unsafe { entry.create_instance(&info.push_next(&mut instance_callback), None) }
                } else {
                    unsafe { entry.create_instance(&info, None) }
                }
                .map_err(VulkanError::api("1.3 instance"))?;
                let messenger = if utils {
                    let loader = ash::ext::debug_utils::Instance::new(entry, &instance);
                    match unsafe { loader.create_debug_utils_messenger(&callback_info, None) } {
                        Ok(messenger) => Some((loader, messenger)),
                        Err(error) => {
                            unsafe { instance.destroy_instance(None) };
                            return Err(VulkanError::api("create debug messenger")(error));
                        }
                    }
                } else {
                    None
                };
                let status = if validation { "active" } else { "unavailable" };
                let utils_status = if utils { "active" } else { "unavailable" };
                log(format_args!(
                    "Vulkan debug: {LAYER:?}={status}; VK_EXT_debug_utils={utils_status}"
                ));
                Ok((
                    instance,
                    Self {
                        counts,
                        validation,
                        messenger,
                    },
                ))
            }

            pub(super) fn name(
                &self,
                instance: &ash::Instance,
                device: &ash::Device,
                object: impl vk::Handle,
                name: &str,
            ) {
                if self.messenger.is_none() {
                    return;
                }
                let name = CString::new(name.replace('\0', "\\0")).expect("NUL was escaped");
                let loader = ash::ext::debug_utils::Device::new(instance, device);
                let info = vk::DebugUtilsObjectNameInfoEXT::default()
                    .object_handle(object)
                    .object_name(&name);
                match unsafe { loader.set_debug_utils_object_name(&info) } {
                    Ok(()) => {
                        // Record a bounded sample of successful names, even when
                        // transient resources are created every frame.
                        if self.counts.named.fetch_add(1, Relaxed) < 64 {
                            log(format_args!(
                                "Vulkan debug named {:?}: {name:?}",
                                info.object_type
                            ));
                        }
                    }
                    Err(error) => log(format_args!("Vulkan debug cannot name {name:?}: {error:?}")),
                }
            }

            pub(super) fn destroy(&mut self) {
                if let Some((loader, messenger)) = self.messenger.take() {
                    unsafe { loader.destroy_debug_utils_messenger(messenger, None) };
                }
            }

            pub(super) fn report(&self) {
                log(format_args!(
                    "Vulkan debug summary: validation={} named={} warnings={} errors={}",
                    if self.validation {
                        "active"
                    } else {
                        "unavailable"
                    },
                    self.counts.named.load(Relaxed),
                    self.counts.warnings.load(Relaxed),
                    self.counts.errors.load(Relaxed),
                ));
            }
        }
    }

    /// Count debug-messenger errors reported by this process.
    pub fn validation_error_count() -> u64 {
        #[cfg(debug_assertions)]
        {
            debug::error_count()
        }
        #[cfg(not(debug_assertions))]
        {
            0
        }
    }
}

pub(crate) const SHARED_USAGE: vk::ImageUsageFlags = vk::ImageUsageFlags::from_raw(
    vk::ImageUsageFlags::TRANSFER_SRC.as_raw()
        | vk::ImageUsageFlags::TRANSFER_DST.as_raw()
        | vk::ImageUsageFlags::SAMPLED.as_raw(),
);

pub(crate) fn handle_type() -> vk::ExternalMemoryHandleTypeFlags {
    #[cfg(windows)]
    {
        vk::ExternalMemoryHandleTypeFlags::D3D12_RESOURCE
    }
    #[cfg(unix)]
    {
        vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD
    }
}

pub(crate) fn extension_name() -> &'static std::ffi::CStr {
    #[cfg(windows)]
    {
        ash::khr::external_memory_win32::NAME
    }
    #[cfg(unix)]
    {
        ash::khr::external_memory_fd::NAME
    }
}

pub(crate) fn matches_adapter(
    instance: &ash::Instance,
    physical: vk::PhysicalDevice,
    identity: &AdapterIdentity,
) -> bool {
    let mut ids = vk::PhysicalDeviceIDProperties::default();
    let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut ids);
    unsafe {
        instance.get_physical_device_properties2(physical, &mut properties);
    }
    match identity {
        AdapterIdentity::WindowsLuid(luid) => {
            ids.device_luid_valid != 0 && ids.device_luid == *luid
        }
        AdapterIdentity::VulkanUuid { device, driver } => {
            ids.device_uuid == *device && ids.driver_uuid == *driver
        }
    }
}

pub(crate) fn validate_external_format(
    context: &DeviceLifetime,
    physical: vk::PhysicalDevice,
    width: u32,
    height: u32,
) -> Result<(), VulkanError> {
    let mut external =
        vk::PhysicalDeviceExternalImageFormatInfo::default().handle_type(handle_type());
    let info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(vk::Format::R8G8B8A8_UNORM)
        .ty(vk::ImageType::TYPE_2D)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(SHARED_USAGE)
        .push_next(&mut external);
    let mut external_properties = vk::ExternalImageFormatProperties::default();
    let max_extent = {
        let mut properties =
            vk::ImageFormatProperties2::default().push_next(&mut external_properties);
        unsafe {
            context
                .instance
                .raw
                .get_physical_device_image_format_properties2(physical, &info, &mut properties)
        }
        .map_err(VulkanError::api("query external RGBA8 image format"))?;
        properties.image_format_properties.max_extent
    };
    if !external_properties
        .external_memory_properties
        .external_memory_features
        .contains(vk::ExternalMemoryFeatureFlags::IMPORTABLE)
        || !external_properties
            .external_memory_properties
            .compatible_handle_types
            .contains(handle_type())
        || width > max_extent.width
        || height > max_extent.height
    {
        return Err(VulkanError::Presentation(
            "GPU adapter cannot import the GUI's dedicated shared RGBA8 image".into(),
        ));
    }
    Ok(())
}

pub(crate) struct SharedImage {
    pub size: [u32; 2],
    image: vk::Image,
    memory: vk::DeviceMemory,
    payload: Arc<dyn GpuFrame>,
    initialized: AtomicBool,
    context: Arc<DeviceLifetime>,
}

/// A lease is the only way to keep a slot busy. Free resize generations are
/// retired before replacement allocation, keeping the native allocation bound.
pub(crate) struct PresentationPool<T> {
    slots: Vec<([u32; 2], Arc<T>)>,
}
impl<T> Default for PresentationPool<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}
impl<T> PresentationPool<T> {
    pub fn busy(&self) -> bool {
        self.slots
            .iter()
            .any(|(_, slot)| Arc::strong_count(slot) != 1)
    }
    pub fn clear(&mut self) {
        self.slots.clear();
    }
    pub fn reserve(
        &mut self,
        size: [u32; 2],
        allocate: impl FnOnce() -> Result<Arc<T>, VulkanError>,
    ) -> Result<Option<Arc<T>>, VulkanError> {
        let reusable = self
            .slots
            .iter()
            .position(|(dimensions, slot)| *dimensions == size && Arc::strong_count(slot) == 1)
            .or_else(|| {
                self.slots
                    .iter()
                    .position(|(_, slot)| Arc::strong_count(slot) == 1)
            });
        if let Some(index) = reusable {
            if self.slots[index].0 == size {
                return Ok(Some(self.slots[index].1.clone()));
            }
            self.slots.swap_remove(index);
        } else if self.slots.len() == 4 {
            return Ok(None);
        }
        let slot = allocate()?;
        self.slots.push((size, slot.clone()));
        Ok(Some(slot))
    }
}
impl Drop for SharedImage {
    fn drop(&mut self) {
        unsafe {
            self.context.device.destroy_image(self.image, None);
            if self.memory != vk::DeviceMemory::null() {
                self.context.device.free_memory(self.memory, None);
            }
        }
    }
}

impl SharedImage {
    pub fn import(
        context: Arc<DeviceLifetime>,
        physical: vk::PhysicalDevice,
        memory: &vk::PhysicalDeviceMemoryProperties,
        width: u32,
        height: u32,
        info: SharedImageInfo,
    ) -> Result<Arc<Self>, VulkanError> {
        validate_external_format(&context, physical, width, height)?;
        let mut external = vk::ExternalMemoryImageCreateInfo::default().handle_types(handle_type());
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(SHARED_USAGE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut external);
        let image = unsafe { context.device.create_image(&image_info, None) }
            .map_err(VulkanError::api("create shared image"))?;
        context.name(image, || format!("GUI shared RGBA8 frame {width}x{height}"));
        let mut result = Self {
            size: [width, height],
            image,
            memory: vk::DeviceMemory::null(),
            payload: info.payload,
            initialized: AtomicBool::new(false),
            context,
        };
        let requirements = unsafe { result.context.device.get_image_memory_requirements(image) };
        // Vulkan's Resource Creation contract says requirements.size has no
        // meaning for D3D12_RESOURCE images. Comparing it with DX12 allocation
        // information rejects valid committed resources at e.g. 1920x1080.
        // OPAQUE_FD still requires the exact exporter size and memory type.
        #[cfg(unix)]
        if info.allocation_size < requirements.size {
            return Err(VulkanError::Presentation(
                "GUI shared allocation is smaller than Vulkan image requirements".into(),
            ));
        }
        #[cfg(windows)]
        let compatible = {
            use std::os::windows::io::AsRawHandle;
            let SharedMemoryHandle::Win32(handle) = &info.handle;
            let extension = ash::khr::external_memory_win32::Device::new(
                &result.context.instance.raw,
                &result.context.device,
            );
            let mut properties = vk::MemoryWin32HandlePropertiesKHR::default();
            unsafe {
                extension.get_memory_win32_handle_properties(
                    handle_type(),
                    handle.as_raw_handle() as _,
                    &mut properties,
                )
            }
            .map_err(VulkanError::api(
                "query GUI shared handle memory properties",
            ))?;
            requirements.memory_type_bits & properties.memory_type_bits
        };
        #[cfg(unix)]
        let compatible = {
            if info.memory_type_bits.count_ones() != 1 {
                return Err(VulkanError::Presentation(
                    "Opaque-FD shared image requires its exact exported memory type".into(),
                ));
            }
            requirements.memory_type_bits & info.memory_type_bits
        };
        let memory_index = (0..memory.memory_type_count)
            .find(|i| {
                compatible & (1 << i) != 0
                    && memory.memory_types[*i as usize]
                        .property_flags
                        .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            })
            .ok_or(VulkanError::Presentation(
                "GUI shared image has no compatible device-local Vulkan memory type".into(),
            ))?;
        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        // allocationSize is ignored for D3D12_RESOURCE imports; the driver
        // queries the OS payload. Supply Vulkan's own requirements value for
        // dedicated-allocation validation, without resizing the DX12 resource.
        #[cfg(windows)]
        let allocation_size = requirements.size;
        #[cfg(unix)]
        let allocation_size = info.allocation_size;
        let allocation = vk::MemoryAllocateInfo::default()
            .allocation_size(allocation_size)
            .memory_type_index(memory_index);
        #[cfg(windows)]
        let allocation_result = {
            use std::os::windows::io::AsRawHandle;
            let SharedMemoryHandle::Win32(handle) = &info.handle;
            let mut import = vk::ImportMemoryWin32HandleInfoKHR::default()
                .handle_type(handle_type())
                .handle(handle.as_raw_handle() as _);
            unsafe {
                result.context.device.allocate_memory(
                    &allocation.push_next(&mut dedicated).push_next(&mut import),
                    None,
                )
            }
        };
        #[cfg(unix)]
        let allocation_result = {
            use std::os::fd::AsRawFd;
            let SharedMemoryHandle::OpaqueFd(handle) = &info.handle;
            let mut import = vk::ImportMemoryFdInfoKHR::default()
                .handle_type(handle_type())
                .fd(handle.as_raw_fd());
            unsafe {
                result.context.device.allocate_memory(
                    &allocation.push_next(&mut dedicated).push_next(&mut import),
                    None,
                )
            }
        };
        result.memory =
            allocation_result.map_err(VulkanError::api("import GUI shared image memory"))?;
        #[cfg(unix)]
        {
            use std::os::fd::IntoRawFd;
            let SharedMemoryHandle::OpaqueFd(handle) = info.handle;
            let _ = handle.into_raw_fd(); // Successful vkAllocateMemory consumed this FD.
        }
        unsafe {
            result
                .context
                .device
                .bind_image_memory(image, result.memory, 0)
        }
        .map_err(VulkanError::api("bind GUI shared image memory"))?;
        Ok(Arc::new(result))
    }

    pub fn record_copy(&self, command: vk::CommandBuffer, source: vk::Image, family: u32) {
        let initialized = self.initialized.load(Ordering::Acquire);
        let subresource = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .level_count(1)
            .layer_count(1);
        let acquire = vk::ImageMemoryBarrier::default()
            .image(self.image)
            .subresource_range(subresource)
            .old_layout(if initialized {
                vk::ImageLayout::GENERAL
            } else {
                vk::ImageLayout::UNDEFINED
            })
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .src_queue_family_index(if initialized {
                vk::QUEUE_FAMILY_EXTERNAL
            } else {
                vk::QUEUE_FAMILY_IGNORED
            })
            .dst_queue_family_index(if initialized {
                family
            } else {
                vk::QUEUE_FAMILY_IGNORED
            });
        let layers = vk::ImageSubresourceLayers::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .layer_count(1);
        let region = vk::ImageCopy::default()
            .src_subresource(layers)
            .dst_subresource(layers)
            .extent(vk::Extent3D {
                width: self.size[0],
                height: self.size[1],
                depth: 1,
            });
        let release = vk::ImageMemoryBarrier::default()
            .image(self.image)
            .subresource_range(subresource)
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::GENERAL)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .src_queue_family_index(family)
            .dst_queue_family_index(vk::QUEUE_FAMILY_EXTERNAL);
        unsafe {
            self.context.device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[acquire],
            );
            self.context.device.cmd_copy_image(
                command,
                source,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                self.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );
            self.context.device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[release],
            );
        }
    }
    pub fn completed(&self) {
        self.initialized.store(true, Ordering::Release);
    }
    pub fn frame(self: &Arc<Self>) -> Arc<dyn GpuFrame> {
        Arc::new(VulkanGpuFrame {
            payload: self.payload.clone(),
            _lease: self.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct Lifetime(Arc<AtomicUsize>);
    impl Drop for Lifetime {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[derive(Debug)]
    struct Payload(u64);
    impl GpuFrame for Payload {
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[test]
    fn bounded_pool_backpressure_never_allocates_and_resize_retires_only_free_slots() {
        let drops = Arc::new(AtomicUsize::new(0));
        let allocations = AtomicUsize::new(0);
        let mut pool = PresentationPool::default();
        let mut leases = Vec::new();
        for _ in 0..4 {
            leases.push(
                pool.reserve([64, 32], || {
                    allocations.fetch_add(1, Ordering::SeqCst);
                    Ok(Arc::new(Lifetime(drops.clone())))
                })
                .unwrap()
                .unwrap(),
            );
        }
        assert!(pool.busy());
        assert!(
            pool.reserve([128, 64], || panic!("backpressure must not allocate"))
                .unwrap()
                .is_none()
        );
        assert_eq!(allocations.load(Ordering::SeqCst), 4);
        leases.pop();
        let resized = pool
            .reserve([128, 64], || {
                assert_eq!(
                    drops.load(Ordering::SeqCst),
                    1,
                    "free previous generation is retired first"
                );
                allocations.fetch_add(1, Ordering::SeqCst);
                Ok(Arc::new(Lifetime(drops.clone())))
            })
            .unwrap()
            .unwrap();
        assert_eq!(pool.slots.len(), 4);
        assert!(
            pool.reserve([256, 128], || panic!("all four native slots remain leased"))
                .unwrap()
                .is_none()
        );
        drop(leases);
        drop(resized);
        assert!(!pool.busy());
        let reused = pool
            .reserve([128, 64], || panic!("same-size free slot is reused"))
            .unwrap()
            .unwrap();
        assert_eq!(allocations.load(Ordering::SeqCst), 5);
        pool.clear();
        assert_eq!(drops.load(Ordering::SeqCst), 4);
        drop(reused);
        assert_eq!(drops.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn gpu_frame_delegates_gui_downcast_but_retains_lease_after_producer_retirement() {
        let drops = Arc::new(AtomicUsize::new(0));
        let lease = Arc::new(Lifetime(drops.clone()));
        let frame: Arc<dyn GpuFrame> = Arc::new(VulkanGpuFrame {
            payload: Arc::new(Payload(73)),
            _lease: lease.clone(),
        });
        drop(lease);
        assert_eq!(frame.as_any().downcast_ref::<Payload>().unwrap().0, 73);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        let consumer = frame.clone();
        drop(frame);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(consumer);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
