use serde::Serialize;
use std::sync::Mutex;

const TARGET: &str = "wgpu_core::device::surface_config";

static OBSERVER: PresentModeObserver = PresentModeObserver(Mutex::new(Vec::new()));

struct PresentModeObserver(Mutex<Vec<&'static str>>);

impl log::Log for PresentModeObserver {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target() == TARGET
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Some(mode) = resolved_mode(&record.args().to_string()) {
            let mut seen = self.0.lock().unwrap();
            if !seen.contains(&mode) {
                seen.push(mode);
            }
        }
    }

    fn flush(&self) {}
}

// wgpu 30 reports its exact Auto resolution from the surface validation path.
// The eframe painter keeps the Surface private. Match this pinned diagnostic
// narrowly so an upstream log change becomes unavailable evidence.
fn resolved_mode(message: &str) -> Option<&'static str> {
    let (request, mode) = message
        .strip_prefix("Automatically choosing presentation mode by rule ")?
        .split_once(". Chose ")?;
    match (request, mode) {
        ("AutoNoVsync", "Immediate") => Some("Immediate"),
        ("AutoNoVsync", "Mailbox") => Some("Mailbox"),
        ("AutoVsync", "FifoRelaxed") => Some("FifoRelaxed"),
        ("AutoVsync" | "AutoNoVsync", "Fifo") => Some("Fifo"),
        _ => None,
    }
}

/// Call before eframe configures the benchmark process's only native surface.
pub fn observe_benchmark_present_mode() {
    if log::set_logger(&OBSERVER).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
    }
}

#[derive(Serialize)]
pub struct BenchmarkPresentMode {
    pub effective_present_mode: Option<&'static str>,
    pub effective_present_mode_unavailable_reason: Option<&'static str>,
    pub observed_present_modes: Vec<&'static str>,
}

pub fn benchmark_present_mode() -> BenchmarkPresentMode {
    let observed_present_modes = OBSERVER.0.lock().unwrap().clone();
    let (effective_present_mode, effective_present_mode_unavailable_reason) =
        match observed_present_modes.as_slice() {
            [mode] => (Some(*mode), None),
            [] => (None, Some("No wgpu surface present mode was observed")),
            _ => (
                None,
                Some("wgpu selected multiple present modes during the run"),
            ),
        };
    BenchmarkPresentMode {
        effective_present_mode,
        effective_present_mode_unavailable_reason,
        observed_present_modes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_the_concrete_resolution_including_vsync_fallback() {
        assert_eq!(
            resolved_mode(
                "Automatically choosing presentation mode by rule AutoNoVsync. Chose Immediate"
            ),
            Some("Immediate")
        );
        assert_eq!(
            resolved_mode(
                "Automatically choosing presentation mode by rule AutoNoVsync. Chose Fifo"
            ),
            Some("Fifo")
        );
        assert_eq!(
            resolved_mode(
                "Automatically choosing presentation mode by rule AutoVsync. Chose FifoRelaxed"
            ),
            Some("FifoRelaxed")
        );
        assert_eq!(
            resolved_mode("Automatically choosing color space by rule Auto. Chose Srgb"),
            None
        );
        assert_eq!(resolved_mode("AutoNoVsync"), None);
    }
}
