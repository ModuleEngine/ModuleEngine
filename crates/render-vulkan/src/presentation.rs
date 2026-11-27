//! Shared Vulkan device and instance ownership.
use ash::vk;
use instance::InstanceLifetime;
use std::sync::Arc;

pub(crate) struct DeviceLifetime {
    pub instance: Arc<InstanceLifetime>,
    pub device: ash::Device,
    pub queue_access: std::sync::Mutex<()>,
}
impl DeviceLifetime {
    pub(crate) fn name(&self, _object: impl vk::Handle, _name: impl FnOnce() -> String) {
        #[cfg(debug_assertions)]
        self.instance.name(&self.device, _object, &_name());
    }
}

impl Drop for DeviceLifetime {
    fn drop(&mut self) {
        // SAFETY: DeviceLifetime owns the device and retains its instance. Submitted work is
        // confirmed before releasing its owners. An unknown result quarantines the entire owner.
        unsafe {
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
            let entry =
                // SAFETY: Use the system Vulkan loader with the platform ABI. InstanceLifetime
                // keeps the entry loaded until the instance and its devices are destroyed.
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
            // SAFETY: The application and requested extension names are terminated
            // strings retained with the create info until the synchronous call returns.
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

        #[cfg(feature = "window-surface")]
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
            // SAFETY: All DeviceLifetime owners retain this instance. The debug messenger is
            // already destroyed and no device can still reference the instance.
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
            // SAFETY: The callback user data is a live Box<Counts> retained through messenger
            // destruction. Atomics support concurrent callbacks.
            let counts = unsafe { &*user.cast::<Counts>() };
            if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
                counts.errors.fetch_add(1, Relaxed);
                ERRORS.fetch_add(1, Relaxed);
            } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
                counts.warnings.fetch_add(1, Relaxed);
            }
            // SAFETY: Vulkan supplies callback data and its NUL-terminated message for the duration
            // of this callback.
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
                // SAFETY: The loaded entry stays live for the entire instance-layer enumeration.
                let layers = unsafe { entry.enumerate_instance_layer_properties() }
                    .map_err(VulkanError::api("enumerate instance layers"))?;
                // SAFETY: The live loader enumerates global extensions with no layer-name pointer.
                let extensions = unsafe { entry.enumerate_instance_extension_properties(None) }
                    .map_err(VulkanError::api("enumerate instance extensions"))?;
                let validation = layers
                    .iter()
                    // SAFETY: Vulkan initializes every enumerated layer name as a NUL-terminated
                    // array.
                    .any(|layer| unsafe { CStr::from_ptr(layer.layer_name.as_ptr()) == LAYER });
                // SAFETY: Vulkan initializes each enumerated extension name as a NUL-terminated
                // array.
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
                    // SAFETY: All enabled names are live terminated strings. The create
                    // chain and callback data are retained throughout the call.
                    unsafe { entry.create_instance(&info.push_next(&mut instance_callback), None) }
                } else {
                    // SAFETY: The live create info contains only advertised layer names and borrows
                    // buffers that outlive the call.
                    unsafe { entry.create_instance(&info, None) }
                }
                .map_err(VulkanError::api("1.3 instance"))?;
                let messenger = if utils {
                    let loader = ash::ext::debug_utils::Instance::new(entry, &instance);
                    // SAFETY: The debug-utils extension was enabled on this instance. The boxed
                    // callback data outlives this messenger.
                    match unsafe { loader.create_debug_utils_messenger(&callback_info, None) } {
                        Ok(messenger) => Some((loader, messenger)),
                        Err(error) => {
                            // SAFETY: Instance creation succeeded, but no messenger or logical
                            // device was created. This path owns and destroys the instance.
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
                // SAFETY: The provided handle is still live on this device. The name is terminated
                // and lives through the call.
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
                    // SAFETY: This messenger belongs to the retained instance. Destruction occurs
                    // once, before the instance and boxed callback state are released.
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
