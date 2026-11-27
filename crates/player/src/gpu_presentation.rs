//! Same-device graph textures displayed by the native Vulkan painter.
use eframe::egui;
use module_engine_core::{Frame, GpuFrame, PresentationTarget};
use module_engine_native_ui::{FrameBridge, FrameTexture};
use std::sync::{
    Arc, Weak,
    atomic::{AtomicU64, Ordering},
};
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct GpuPresentationCounters {
    pub gpu_presentations: u64,
    pub busy_skips: u64,
    pub pending_copies: usize,
    pub pending_high_water: usize,
    pub cpu_readback_bytes: u64,
    pub cpu_upload_bytes: u64,
    pub diagnostic_readback_bytes: u64,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct GpuRetirementReport {
    pub registered_jobs: usize,
    pub joined_jobs: usize,
    pub completed_copies: usize,
    pub unconfirmed_copies: usize,
    pub failed_jobs: usize,
    #[cfg(feature = "vulkan")]
    pub producer: module_engine_vulkan::ProducerRetirementReport,
}

#[derive(Debug)]
pub struct GpuRetirementFailure {
    pub report: GpuRetirementReport,
    pub errors: Vec<String>,
}
impl std::fmt::Display for GpuRetirementFailure {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "GPU retirement failed: {}", self.errors.join("; "))
    }
}
impl std::error::Error for GpuRetirementFailure {}

/// Join outstanding graph producer submissions after the window owners have retired.
pub fn finish_gpu_retirements() -> Result<GpuRetirementReport, GpuRetirementFailure> {
    #[cfg(feature = "vulkan")]
    let (report, errors) = {
        let (producer, errors) = module_engine_vulkan::finish_producer_retirements();
        (
            GpuRetirementReport {
                producer,
                ..Default::default()
            },
            errors,
        )
    };
    #[cfg(not(feature = "vulkan"))]
    let (report, errors) = (GpuRetirementReport::default(), Vec::<String>::new());
    eprintln!(
        "ModuleEngine GPU retirement: {}",
        serde_json::to_string(&report).expect("GPU retirement report")
    );
    if errors.is_empty() {
        Ok(report)
    } else {
        Err(GpuRetirementFailure { report, errors })
    }
}

#[derive(Clone)]
pub struct GpuPresentationBridge {
    window: FrameBridge,
    presentations: Arc<AtomicU64>,
}
impl GpuPresentationBridge {
    pub fn window(window: FrameBridge) -> Self {
        Self {
            window,
            presentations: Default::default(),
        }
    }
    pub fn target(&self) -> Arc<dyn PresentationTarget> {
        self.window.target()
    }
    pub fn counters(&self) -> GpuPresentationCounters {
        GpuPresentationCounters {
            gpu_presentations: self.presentations.load(Ordering::Relaxed),
            ..Default::default()
        }
    }
}

#[derive(Default)]
pub struct GpuTexture {
    window: Option<FrameTexture>,
    last: Option<Weak<dyn GpuFrame>>,
    presentations: u64,
}
impl GpuTexture {
    pub fn update(
        &mut self,
        bridge: &GpuPresentationBridge,
        frame: &Frame,
    ) -> Result<Option<egui::TextureId>, String> {
        let Some(lease) = frame.gpu.as_ref() else {
            return Ok(None);
        };
        let last = Arc::downgrade(lease);
        if self.last.as_ref().is_some_and(|old| old.ptr_eq(&last))
            && self
                .window
                .as_ref()
                .is_some_and(|display| display.belongs_to(&bridge.window))
        {
            return Ok(None);
        }
        let texture = bridge.window.bind(frame)?;
        let id = texture.id;
        self.window = Some(texture);
        self.last = Some(last);
        self.presentations += 1;
        bridge.presentations.fetch_add(1, Ordering::Relaxed);
        Ok(Some(id))
    }
    pub fn texture_id(&self) -> Option<egui::TextureId> {
        self.window.as_ref().map(|t| t.id)
    }
    pub fn dimensions(&self) -> Option<[usize; 2]> {
        self.window.as_ref().map(|t| t.dimensions)
    }
    pub fn counters(&self) -> GpuPresentationCounters {
        GpuPresentationCounters {
            gpu_presentations: self.presentations,
            ..Default::default()
        }
    }
}
