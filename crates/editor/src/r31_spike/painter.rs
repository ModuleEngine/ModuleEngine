use super::{PainterKind, VkResultExt, gpu::Frames, thin::ThinPainter};
use ash::vk;
use eframe::egui::{TextureId, TexturesDelta, epaint::ClippedPrimitive};
use egui_ash_renderer::{
    DynamicRendering, Options, RenderMode, Renderer, allocator::DefaultAllocator,
};
use module_engine_vulkan::EditorDeviceHandles;
use std::path::Path;

pub(super) struct Painter {
    backend: Backend,
    pending_free: Vec<TextureId>,
}

enum Backend {
    Library(Box<Renderer<DefaultAllocator>>),
    Thin(Box<ThinPainter>),
}

impl Painter {
    pub fn new(
        handles: &EditorDeviceHandles<'_>,
        frames: &Frames,
        root: &Path,
        kind: PainterKind,
    ) -> Result<Self, String> {
        let backend = match kind {
            PainterKind::Library => Backend::Library(Box::new(
                Renderer::with_default_allocator(
                    handles.instance,
                    handles.physical,
                    handles.device.clone(),
                    RenderMode::DynamicRendering(DynamicRendering {
                        color_attachment_format: frames.format,
                        depth_attachment_format: None,
                        stencil_attachment_format: None,
                    }),
                    Options::default(),
                )
                .map_err(|e| format!("egui-ash-renderer initialization: {e}"))?,
            )),
            PainterKind::Thin => Backend::Thin(Box::new(ThinPainter::new(frames, root)?)),
        };
        Ok(Self {
            backend,
            pending_free: Vec::new(),
        })
    }

    pub fn update_textures(
        &mut self,
        frames: &Frames,
        mut delta: TexturesDelta,
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
        frames: &Frames,
        cmd: vk::CommandBuffer,
        extent: vk::Extent2D,
        scale: f32,
        primitives: &[ClippedPrimitive],
    ) -> Result<(), String> {
        match &mut self.backend {
            Backend::Library(painter) => painter
                .cmd_draw(cmd, extent, scale, primitives)
                .map_err(|e| format!("egui-ash-renderer draw: {e}")),
            Backend::Thin(painter) => painter.record(frames, cmd, extent, scale, primitives),
        }
    }
}

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
