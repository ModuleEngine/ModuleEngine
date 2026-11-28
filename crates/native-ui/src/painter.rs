use super::{PainterKind, gpu::Frames};
#[cfg(feature = "spike")]
use super::{VkResultExt, thin::ThinPainter};
use ash::vk;
use egui::{TextureId, TexturesDelta, epaint::ClippedPrimitive};
use egui_ash_renderer::{
    DynamicRendering, Options, RenderMode, Renderer, allocator::DefaultAllocator,
};
use module_engine_vulkan::{EditorDeviceHandles, WindowFrame};
use std::path::Path;

pub(super) struct Painter {
    backend: Backend,
    descriptors: crate::sampled::Descriptors,
    bindings: std::collections::HashMap<TextureId, crate::sampled::Binding>,
    pending_free: Vec<TextureId>,
}

enum Backend {
    Library(Box<Renderer<DefaultAllocator>>),
    #[cfg(feature = "spike")]
    Thin(Box<ThinPainter>),
}

impl Painter {
    pub fn new(
        handles: &EditorDeviceHandles<'_>,
        frames: &Frames,
        _root: &Path,
        kind: PainterKind,
    ) -> Result<Self, String> {
        match kind {
            PainterKind::Library => Self::library(
                handles.instance,
                handles.physical,
                handles.device,
                frames.format,
            ),
            #[cfg(feature = "spike")]
            PainterKind::Thin => Self::from_backend(
                handles.device,
                Backend::Thin(Box::new(ThinPainter::new(frames, _root)?)),
            ),
        }
    }

    pub fn library(
        instance: &ash::Instance,
        physical: vk::PhysicalDevice,
        device: &ash::Device,
        format: vk::Format,
    ) -> Result<Self, String> {
        let renderer = Renderer::with_default_allocator(
            instance,
            physical,
            device.clone(),
            RenderMode::DynamicRendering(DynamicRendering {
                color_attachment_format: format,
                depth_attachment_format: None,
                stencil_attachment_format: None,
            }),
            Options::default(),
        )
        .map_err(|e| format!("egui-ash-renderer initialization: {e}"))?;
        Self::from_backend(device, Backend::Library(Box::new(renderer)))
    }

    fn from_backend(device: &ash::Device, backend: Backend) -> Result<Self, String> {
        Ok(Self {
            backend,
            descriptors: crate::sampled::Descriptors::new(device)?,
            bindings: Default::default(),
            pending_free: Vec::new(),
        })
    }

    pub fn draw_diagnostic(
        &mut self,
        command: vk::CommandBuffer,
        extent: vk::Extent2D,
        primitives: &[ClippedPrimitive],
    ) -> Result<(), String> {
        match &mut self.backend {
            Backend::Library(painter) => painter
                .cmd_draw(command, extent, 1., primitives)
                .map_err(|error| error.to_string()),
            #[cfg(feature = "spike")]
            Backend::Thin(_) => {
                Err("Offscreen verification requires the native library painter".into())
            }
        }
    }

    pub fn register(
        &mut self,
        view: vk::ImageView,
        lease: std::sync::Arc<dyn module_engine_core::GpuFrame>,
        token: std::sync::Weak<()>,
    ) -> Result<TextureId, String> {
        match &mut self.backend {
            Backend::Library(renderer) => {
                let set = self.descriptors.image_set(view)?;
                let id = renderer.add_graph_texture(set);
                self.bindings
                    .insert(id, crate::sampled::Binding { token, set, lease });
                Ok(id)
            }
            #[cfg(feature = "spike")]
            Backend::Thin(_) => Err("R3.1 thin painter cannot sample game images".into()),
        }
    }

    pub fn retire(&mut self, primitives: &[ClippedPrimitive]) -> Result<(), String> {
        let retired: Vec<_> = self.bindings.iter().filter_map(|(&id, binding)| {
            let drawn = primitives.iter().any(|primitive| {
                matches!(&primitive.primitive, egui::epaint::Primitive::Mesh(mesh) if mesh.texture_id == id)
            });
            (binding.token.strong_count() == 0 && !drawn).then_some(id)
        }).collect();
        for id in retired {
            let binding = &self.bindings[&id];
            self.descriptors.release(binding.set)?;
            match &mut self.backend {
                Backend::Library(renderer) => renderer.remove_user_texture(id),
                #[cfg(feature = "spike")]
                Backend::Thin(_) => unreachable!(),
            }
            self.bindings.remove(&id);
        }
        Ok(())
    }

    pub fn note_ui_submission(&self, primitives: &[ClippedPrimitive]) {
        for primitive in primitives {
            if let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive
                && let Some(binding) = self.bindings.get(&mesh.texture_id)
                && let Some(frame) = binding.lease.as_any().downcast_ref::<WindowFrame>()
            {
                frame.note_ui_submission();
            }
        }
    }

    pub fn update_textures(
        &mut self,
        frames: &Frames,
        delta: &mut TexturesDelta,
    ) -> Result<(), String> {
        // The shared frame fence already retired every prior draw that sampled these textures.
        match &mut self.backend {
            Backend::Library(painter) => {
                for id in self.pending_free.drain(..) {
                    painter
                        .free_texture(id)
                        .map_err(|e| format!("egui-ash-renderer free: {e}"))?;
                }
                for (id, changes) in delta.set.drain() {
                    for change in changes {
                        painter
                            .set_texture(frames.queue, frames.pool, id, &change)
                            .map_err(|e| format!("egui-ash-renderer upload: {e}"))?;
                    }
                }
            }
            #[cfg(feature = "spike")]
            Backend::Thin(painter) => {
                for id in self.pending_free.drain(..) {
                    painter.free_texture(id);
                }
                for (id, changes) in delta.set.drain() {
                    for change in changes {
                        painter.set_texture(frames, id, &change)?;
                    }
                }
            }
        }
        self.pending_free.extend(delta.free.drain());
        Ok(())
    }

    pub fn record(
        &mut self,
        _frames: &Frames,
        cmd: vk::CommandBuffer,
        extent: vk::Extent2D,
        scale: f32,
        primitives: &[ClippedPrimitive],
    ) -> Result<(), String> {
        match &mut self.backend {
            Backend::Library(painter) => painter
                .cmd_draw(cmd, extent, scale, primitives)
                .map_err(|e| format!("egui-ash-renderer draw: {e}")),
            #[cfg(feature = "spike")]
            Backend::Thin(painter) => painter.record(_frames, cmd, extent, scale, primitives),
        }
    }
}

#[cfg(feature = "spike")]
pub(super) fn upload(
    frames: &Frames,
    record: impl FnOnce(vk::CommandBuffer),
) -> Result<(), String> {
    let device = &frames.device;
    // This command buffer is transient and both painters serialize texture uploads
    // on their sole graphics queue, between completed drawing submissions.
    unsafe {
        let command = device
            .allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(frames.pool)
                    .command_buffer_count(1),
            )
            .at("allocate upload command buffer")?[0];
        let result = (|| {
            device
                .begin_command_buffer(
                    command,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .at("begin painter upload")?;
            record(command);
            device
                .end_command_buffer(command)
                .at("end painter upload")?;
            device
                .queue_submit(
                    frames.queue,
                    &[vk::SubmitInfo::default().command_buffers(&[command])],
                    vk::Fence::null(),
                )
                .at("submit painter upload")?;
            device
                .queue_wait_idle(frames.queue)
                .at("retire painter upload")
        })();
        device.free_command_buffers(frames.pool, &[command]);
        result
    }
}
