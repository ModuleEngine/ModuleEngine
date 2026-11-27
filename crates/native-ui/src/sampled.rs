//! Egui user textures backed by the producer's render-graph image.
use crate::{VkResultExt, painter::Painter};
use ash::vk;
use egui::TextureId;
use module_engine_core::{Frame, GpuFrame};
use module_engine_vulkan::{WindowFrame, WindowTarget};
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone)]
pub struct FrameBridge {
    pub adapter: String,
    pub present_mode: &'static str,
    pub(crate) painter: Weak<Mutex<Painter>>,
    pub(crate) target: Arc<WindowTarget>,
}

pub struct FrameTexture {
    pub id: TextureId,
    pub dimensions: [usize; 2],
    painter: Weak<Mutex<Painter>>,
    _registration: Arc<()>,
}

impl FrameTexture {
    pub fn belongs_to(&self, bridge: &FrameBridge) -> bool {
        self.painter.ptr_eq(&bridge.painter)
    }
}

impl FrameBridge {
    pub fn graph_renderer(
        &self,
        root: std::path::PathBuf,
    ) -> Result<module_engine_vulkan::VulkanRenderer, String> {
        self.target
            .renderer(root)
            .map_err(|error| error.to_string())
    }
    pub fn target(&self) -> Arc<dyn module_engine_core::PresentationTarget> {
        self.target.clone()
    }

    pub fn bind(&self, frame: &Frame) -> Result<FrameTexture, String> {
        let lease = frame.gpu.as_ref().ok_or("Missing graph output lease")?;
        let image = lease
            .as_any()
            .downcast_ref::<WindowFrame>()
            .ok_or("Not a window render-graph image")?;
        if !image.belongs_to(&self.target) || image.size() != [frame.width, frame.height] {
            return Err("Graph output device or dimensions differ from the window".into());
        }
        let painter = self.painter.upgrade().ok_or("Native painter has closed")?;
        let mut painter = painter.lock().map_err(|_| "Native painter lock poisoned")?;
        let token = Arc::new(());
        let id = painter.register(image.view(), lease.clone(), Arc::downgrade(&token))?;
        Ok(FrameTexture {
            id,
            dimensions: image.size(),
            painter: self.painter.clone(),
            _registration: token,
        })
    }
}

pub(crate) struct Binding {
    pub token: Weak<()>,
    pub set: vk::DescriptorSet,
    pub _lease: Arc<dyn GpuFrame>,
}

/// One descriptor pool serves all graph views drawn by the window painter.
pub(crate) struct Descriptors {
    device: ash::Device,
    sampler: vk::Sampler,
    layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
}

impl Descriptors {
    pub fn new(device: &ash::Device) -> Result<Self, String> {
        let mut d = Self {
            device: device.clone(),
            sampler: vk::Sampler::null(),
            layout: vk::DescriptorSetLayout::null(),
            pool: vk::DescriptorPool::null(),
        };
        // SAFETY: All handles are created on the retained UI device. Partial
        // construction drops successfully created handles in dependency order.
        unsafe {
            d.sampler = device
                .create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(vk::Filter::LINEAR)
                        .min_filter(vk::Filter::LINEAR)
                        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                    None,
                )
                .at("create graph image sampler")?;
            let bindings = [vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
            d.layout = device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .at("create egui user image layout")?;
            let sizes = [vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1024,
            }];
            d.pool = device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
                        .max_sets(1024)
                        .pool_sizes(&sizes),
                    None,
                )
                .at("create graph image descriptor pool")?;
        }
        Ok(d)
    }
    pub fn image_set(&self, view: vk::ImageView) -> Result<vk::DescriptorSet, String> {
        // SAFETY: The lease passed by the caller keeps this view live. The
        // producer transitions it to shader-read layout before publication.
        unsafe {
            let set = self
                .device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(self.pool)
                        .set_layouts(&[self.layout]),
                )
                .at("allocate graph image descriptor")?[0];
            let image = [vk::DescriptorImageInfo::default()
                .image_view(view)
                .sampler(self.sampler)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            self.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&image)],
                &[],
            );
            Ok(set)
        }
    }
    pub fn release(&self, set: vk::DescriptorSet) -> Result<(), String> {
        // SAFETY: The common painter runs this after retiring its previous draw.
        unsafe { self.device.free_descriptor_sets(self.pool, &[set]) }
            .at("retire graph image descriptor")
    }
}
impl Drop for Descriptors {
    fn drop(&mut self) {
        // SAFETY: The UI device is idle before the painter is destroyed.
        unsafe {
            self.device.destroy_descriptor_pool(self.pool, None);
            self.device.destroy_descriptor_set_layout(self.layout, None);
            self.device.destroy_sampler(self.sampler, None);
        }
    }
}
