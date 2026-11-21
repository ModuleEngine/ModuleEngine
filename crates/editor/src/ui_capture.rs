//! Optional native-app screenshot diagnostics, after real UI frames settle.
//! Normal editor runs never request screenshots or additional repaints here.
use eframe::egui;
use std::{
    ffi::OsStr,
    fs,
    io::BufWriter,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct CaptureGeometry {
    /// Requested physical client pixels, independent of UI zoom/native DPI.
    pub size: [f32; 2],
    pub zoom: f32,
    pub settle_ms: u64,
}
fn parse_geometry(size: &str, zoom: &str, settle: &str) -> Result<CaptureGeometry, String> {
    let (width, height) = size
        .split_once('x')
        .ok_or("Capture size must be WIDTHxHEIGHT")?;
    let width = width.parse::<u32>().map_err(|_| "Invalid capture width")?;
    let height = height
        .parse::<u32>()
        .map_err(|_| "Invalid capture height")?;
    let zoom = zoom.parse::<f32>().map_err(|_| "Invalid capture zoom")?;
    let settle_ms = settle
        .parse::<u64>()
        .map_err(|_| "Invalid capture settling time")?;
    if !(320..=3840).contains(&width)
        || !(240..=2160).contains(&height)
        || !zoom.is_finite()
        || !(0.5..=3.).contains(&zoom)
        || settle_ms > 10000
    {
        return Err("Capture geometry exceeds diagnostic bounds".into());
    }
    Ok(CaptureGeometry {
        size: [width as f32, height as f32],
        zoom,
        settle_ms,
    })
}
pub(crate) fn capture_geometry() -> Result<Option<CaptureGeometry>, String> {
    let size = std::env::var("MODULE_ENGINE_CAPTURE_SIZE").ok();
    let zoom = std::env::var("MODULE_ENGINE_CAPTURE_ZOOM").ok();
    let settle = std::env::var("MODULE_ENGINE_UI_SETTLE_MS").ok();
    if size.is_none() && zoom.is_none() && settle.is_none() {
        return Ok(None);
    }
    if std::env::var_os("MODULE_ENGINE_UI_SCREENSHOT_TO").is_none()
        || diagnostic_personal_root()?.is_none()
    {
        return Err("Capture geometry requires a screenshot destination and isolated diagnostic personal root".into());
    }
    parse_geometry(
        size.as_deref().unwrap_or("1580x940"),
        zoom.as_deref().unwrap_or("1"),
        settle.as_deref().unwrap_or("400"),
    )
    .map(Some)
}

/// Personal settings are the only redirected paths. This never substitutes
/// package/source roots, Engine bindings, SDKs or authored scene data.
pub(crate) fn diagnostic_personal_root() -> Result<Option<PathBuf>, String> {
    parse_diagnostic_personal_root(
        std::env::var_os("MODULE_ENGINE_DIAGNOSTIC_USER_ROOT").as_deref(),
    )
}

fn parse_diagnostic_personal_root(value: Option<&OsStr>) -> Result<Option<PathBuf>, String> {
    let Some(value) = value else { return Ok(None) };
    let path = Path::new(value);
    if !path.is_absolute() || !path.is_dir() {
        return Err(
            "MODULE_ENGINE_DIAGNOSTIC_USER_ROOT requires an existing absolute directory".into(),
        );
    }
    path.canonicalize()
        .map(Some)
        .map_err(|error| format!("Diagnostic personal directory: {error}"))
}

pub struct Capture {
    destination: Option<PathBuf>,
    game_timings: bool,
    last_frame: Option<u64>,
    frames: u32,
    requested: bool,
    started: Option<Instant>,
    settled_since: Option<Instant>,
    done: bool,
    settle: Duration,
}

impl Default for Capture {
    fn default() -> Self {
        Self::new()
    }
}

impl Capture {
    pub fn new() -> Self {
        Self {
            destination: std::env::var_os("MODULE_ENGINE_UI_SCREENSHOT_TO")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            game_timings: false,
            last_frame: None,
            frames: 0,
            requested: false,
            started: None,
            settled_since: None,
            done: false,
            settle: Duration::from_millis(
                capture_geometry()
                    .ok()
                    .flatten()
                    .map_or(400, |g| g.settle_ms),
            ),
        }
    }

    pub fn wait_for_game_timings(&mut self) -> bool {
        self.game_timings = self.destination.is_some();
        self.game_timings
    }

    pub fn requires_game_timings(&self) -> bool {
        self.game_timings
    }

    /// Returns true only after the requested native screenshot was saved.
    /// The caller can then allow its diagnostic window to close normally.
    pub fn tick(&mut self, ctx: &egui::Context, ready: bool) -> Result<bool, String> {
        let Some(destination) = &self.destination else {
            return Ok(false);
        };
        if self.done {
            return Ok(true);
        }
        if self.requested {
            let viewport = ctx.viewport_id();
            let screenshot = ctx.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot {
                        viewport_id, image, ..
                    } if *viewport_id == viewport => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(image) = screenshot {
                write_png(destination, &image)?;
                self.done = true;
                return Ok(true);
            }
        }
        let started = self.started.get_or_insert_with(Instant::now);
        if started.elapsed() > Duration::from_secs(45) {
            return Err("Native UI capture timed out before a screenshot arrived".into());
        }
        if !ready {
            self.frames = 0;
            self.settled_since = None;
            ctx.request_repaint_after(Duration::from_millis(33));
            return Ok(false);
        }
        let frame = ctx.cumulative_frame_nr();
        if self.last_frame != Some(frame) {
            self.last_frame = Some(frame);
            self.frames += 1;
        }
        // Count frames rather than layout passes. A sizing pass can run twice
        // in one frame and must not capture a partly laid-out menu or toolbar.
        if !self.requested && self.frames >= 8 {
            // Initial host/shader preparation can occupy the first frame. Start
            // the animation wait only once subsequent frames have settled.
            let settled = self.settled_since.get_or_insert_with(Instant::now);
            if settled.elapsed() >= self.settle {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.requested = true;
            }
        }
        ctx.request_repaint_after(Duration::from_millis(16));
        Ok(false)
    }
}

fn write_png(path: &Path, image: &egui::ColorImage) -> Result<(), String> {
    let [width, height] = image.size;
    if width == 0 || height == 0 || width.checked_mul(height) != Some(image.pixels.len()) {
        return Err("Native screenshot has invalid pixel dimensions".into());
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|e| format!("UI capture folder: {e}"))?;
    }
    let width = u32::try_from(width).map_err(|_| "UI capture width exceeds PNG limits")?;
    let height = u32::try_from(height).map_err(|_| "UI capture height exceeds PNG limits")?;
    let file = fs::File::create(path).map_err(|e| format!("UI capture file: {e}"))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| format!("UI capture PNG: {e}"))?;
    let rgba = image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
        .collect::<Vec<_>>();
    writer
        .write_image_data(&rgba)
        .map_err(|e| format!("UI capture pixels: {e}"))?;
    writer
        .finish()
        .map_err(|e| format!("UI capture finish: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_geometry_has_explicit_physical_size_and_bounded_ui_zoom() {
        let geometry = parse_geometry("1280x720", "2", "1500").unwrap();
        assert_eq!(geometry.size, [1280., 720.]);
        assert_eq!(geometry.zoom, 2.);
        assert_eq!(geometry.settle_ms, 1500);
        for (size, zoom, delay) in [
            ("0x720", "1", "400"),
            ("1280x720", "NaN", "400"),
            ("1280x720", "4", "400"),
            ("1280x720", "1", "20000"),
        ] {
            assert!(parse_geometry(size, zoom, delay).is_err());
        }
    }
    #[test]
    fn diagnostic_personal_root_requires_an_explicit_existing_absolute_directory() {
        assert_eq!(parse_diagnostic_personal_root(None).unwrap(), None);
        for value in ["", "relative/personal", "."] {
            assert!(parse_diagnostic_personal_root(Some(OsStr::new(value))).is_err());
        }
        let guard = module_engine_script::TestArtifactGuard::new(
            &std::env::temp_dir().join("module-engine-diagnostic-settings"),
            "personal-root",
        )
        .unwrap();
        let root = guard.path().canonicalize().unwrap();
        assert_eq!(
            parse_diagnostic_personal_root(Some(root.as_os_str())).unwrap(),
            Some(root.clone())
        );
        assert!(parse_diagnostic_personal_root(Some(root.join("missing").as_os_str())).is_err());
        let file = root.join("not-a-directory");
        fs::write(&file, b"keep").unwrap();
        assert!(parse_diagnostic_personal_root(Some(file.as_os_str())).is_err());
        assert_eq!(fs::read(file).unwrap(), b"keep");
    }
}
