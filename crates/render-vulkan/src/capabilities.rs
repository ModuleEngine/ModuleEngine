use ash::vk;
use std::ffi::CStr;

// One descriptor is reserved for untextured geometry.
pub(crate) const MATERIAL_IMAGE_COUNT: u32 = 1024;

/// Capabilities reported by the selected physical device. Optional tiers share
/// the tier A baseline and can be present independently.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct DeviceCapabilities {
    pub tier_a: bool,
    pub tier_b: bool,
    pub tier_c: bool,
    pub ray_tracing_pipeline: bool,
    pub timeline_semaphore: bool,
    pub synchronization2: bool,
    pub bindless_materials: bool,
    pub max_material_anisotropy: f32,
    pub graphics_family: u32,
    pub dedicated_transfer_family: Option<u32>,
}

impl DeviceCapabilities {
    /// Vulkan 1.3 and graphics+compute were checked during adapter selection.
    pub(crate) fn query(
        instance: &ash::Instance,
        physical: vk::PhysicalDevice,
        extensions: &[vk::ExtensionProperties],
        graphics_family: u32,
        families: &[vk::QueueFamilyProperties],
    ) -> Option<Self> {
        let supports = |name| {
            extensions.iter().any(|extension| {
                let reported = unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) };
                reported == name
            })
        };
        let mut core12 = vk::PhysicalDeviceVulkan12Features::default();
        let mut core13 = vk::PhysicalDeviceVulkan13Features::default();
        let mut mesh = vk::PhysicalDeviceMeshShaderFeaturesEXT::default();
        let mut acceleration = vk::PhysicalDeviceAccelerationStructureFeaturesKHR::default();
        let mut rays = vk::PhysicalDeviceRayQueryFeaturesKHR::default();
        let mut ray_pipeline = vk::PhysicalDeviceRayTracingPipelineFeaturesKHR::default();
        let mut features = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut core12)
            .push_next(&mut core13);
        if supports(ash::ext::mesh_shader::NAME) {
            features = features.push_next(&mut mesh);
        }
        if supports(ash::khr::acceleration_structure::NAME)
            && supports(ash::khr::deferred_host_operations::NAME)
        {
            features = features.push_next(&mut acceleration);
        }
        if supports(ash::khr::ray_query::NAME) {
            features = features.push_next(&mut rays);
        }
        if supports(ash::khr::ray_tracing_pipeline::NAME) {
            features = features.push_next(&mut ray_pipeline);
        }
        unsafe { instance.get_physical_device_features2(physical, &mut features) };
        let sampler_anisotropy = features.features.sampler_anisotropy != 0;
        if core13.dynamic_rendering == 0 {
            return None;
        }
        let synchronization2 = core13.synchronization2 != 0;
        let tier_a = synchronization2
            && core12.buffer_device_address != 0
            && core12.descriptor_indexing != 0
            && core12.shader_sampled_image_array_non_uniform_indexing != 0
            && core12.runtime_descriptor_array != 0;
        let limits = unsafe { instance.get_physical_device_properties(physical) }.limits;
        let texture_format = unsafe {
            instance.get_physical_device_format_properties(physical, vk::Format::R8G8B8A8_SRGB)
        };
        let texture_features = vk::FormatFeatureFlags::SAMPLED_IMAGE
            | vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR
            | vk::FormatFeatureFlags::TRANSFER_SRC
            | vk::FormatFeatureFlags::TRANSFER_DST
            | vk::FormatFeatureFlags::BLIT_SRC
            | vk::FormatFeatureFlags::BLIT_DST;
        let bindless_materials = tier_a
            && core12.timeline_semaphore != 0
            && sampler_anisotropy
            && limits.max_bound_descriptor_sets >= 2
            && limits.max_per_stage_descriptor_sampled_images >= MATERIAL_IMAGE_COUNT
            && limits.max_descriptor_set_sampled_images >= MATERIAL_IMAGE_COUNT
            && limits.max_per_stage_resources >= MATERIAL_IMAGE_COUNT + 2
            && limits.max_per_stage_descriptor_storage_buffers >= 1
            && limits.max_descriptor_set_storage_buffers >= 1
            && limits.max_per_stage_descriptor_samplers >= 1
            && limits.max_descriptor_set_samplers >= 1
            && texture_format
                .optimal_tiling_features
                .contains(texture_features);
        Some(Self {
            tier_a,
            tier_b: tier_a && mesh.task_shader != 0 && mesh.mesh_shader != 0,
            tier_c: tier_a && acceleration.acceleration_structure != 0 && rays.ray_query != 0,
            ray_tracing_pipeline: tier_a
                && acceleration.acceleration_structure != 0
                && ray_pipeline.ray_tracing_pipeline != 0,
            timeline_semaphore: core12.timeline_semaphore != 0,
            synchronization2,
            bindless_materials,
            max_material_anisotropy: if bindless_materials {
                limits.max_sampler_anisotropy.min(16.0)
            } else {
                1.0
            },
            graphics_family,
            dedicated_transfer_family: families
                .iter()
                .position(|family| {
                    family.queue_count != 0
                        && family.queue_flags.contains(vk::QueueFlags::TRANSFER)
                        && !family
                            .queue_flags
                            .intersects(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
                })
                .map(|index| index as u32),
        })
    }

    pub(crate) fn use_modern_device(self) -> bool {
        self.tier_a && self.timeline_semaphore
    }
}
