//! Explicit display-free verification using the production egui texture descriptors and shader.
use crate::{FrameBridge, painter::Painter};
use ash::vk;
use egui::{
    Color32, Pos2, Rect, TextureId, Vec2,
    epaint::{ClippedPrimitive, Mesh, Primitive},
};
use module_engine_vulkan::{VulkanError, VulkanRenderer};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub struct Offscreen {
    painter: Arc<Mutex<Painter>>,
    renderer: VulkanRenderer,
    bridge: FrameBridge,
}

impl Offscreen {
    pub fn new(root: &Path) -> Result<Self, String> {
        let renderer = VulkanRenderer::new_at(root.to_owned()).map_err(|e| e.to_string())?;
        let (instance, physical, device) = renderer.graphics_device();
        let painter = Arc::new(Mutex::new(Painter::library(
            instance,
            physical,
            device,
            vk::Format::R8G8B8A8_UNORM,
        )?));
        let bridge = FrameBridge {
            adapter: renderer.device_name().into(),
            present_mode: "Offscreen",
            painter: Arc::downgrade(&painter),
            target: Arc::new(renderer.offscreen_target()),
        };
        Ok(Self {
            painter,
            renderer,
            bridge,
        })
    }

    pub fn bridge(&self) -> FrameBridge {
        self.bridge.clone()
    }

    /// All diagnostic draws finish synchronously before unused IDs are collected.
    pub fn collect(&self) -> Result<(), String> {
        self.painter
            .lock()
            .map_err(|_| "Offscreen painter lock poisoned")?
            .retire(&[])
    }

    pub fn capture(
        &mut self,
        id: TextureId,
        [width, height]: [usize; 2],
    ) -> Result<Vec<u8>, String> {
        self.collect()?;
        let extent = vk::Extent2D {
            width: u32::try_from(width).map_err(|e| e.to_string())?,
            height: u32::try_from(height).map_err(|e| e.to_string())?,
        };
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width as f32, height as f32));
        let mut mesh = Mesh::with_texture(id);
        mesh.add_rect_with_uv(
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
        let primitives = [ClippedPrimitive {
            clip_rect: rect,
            primitive: Primitive::Mesh(mesh),
        }];
        // SAFETY: The retained painter owns the registered graph lease, its descriptors and
        // mesh buffers. The offscreen owner serializes captures and waits before reuse.
        unsafe {
            self.renderer.diagnostic_rgba(
                [extent.width, extent.height],
                self.painter.clone(),
                |painter, cmd| {
                    painter
                        .lock()
                        .map_err(|_| {
                            VulkanError::Presentation("Offscreen painter lock poisoned".into())
                        })?
                        .draw_diagnostic(cmd, extent, &primitives)
                        .map_err(VulkanError::Presentation)
                },
            )
        }
        .map_err(|e| e.to_string())
    }
}
