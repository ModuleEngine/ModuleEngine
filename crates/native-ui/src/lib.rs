//! Vulkan presentation shared by the native Editor and Player windows.
mod gpu;
mod offscreen;
mod painter;
pub use offscreen::Offscreen;
mod sampled;
pub use sampled::{FrameBridge, FrameTexture};
mod shell;
mod viewports;
pub use shell::{NativeApp, run, run_with_meshlets};
pub use winit::window::Window;
#[cfg(feature = "spike")]
mod thin;

use ash::vk;
pub use gpu::{DeviceInfo, Gpu};
use serde::Serialize;

#[derive(Clone, Copy, Serialize)]
pub enum PainterKind {
    #[serde(rename = "ash")]
    Library,
    #[cfg(feature = "spike")]
    #[serde(rename = "thin")]
    Thin,
}

trait VkResultExt<T> {
    fn at(self, operation: &str) -> Result<T, String>;
}

impl<T> VkResultExt<T> for Result<T, vk::Result> {
    fn at(self, operation: &str) -> Result<T, String> {
        self.map_err(|e| format!("Vulkan UI {operation}: {e:?}"))
    }
}
