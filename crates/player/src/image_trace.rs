//! Opt-in, bounded observations of the ordinary Player, never enabled by launchers.
use eframe::egui::Rect;
use module_engine_core::Frame;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    time::Instant,
};

pub(super) struct ImageTrace {
    file: File,
    remaining: usize,
    last: Option<([u32; 2], [usize; 2], u32)>,
    started: Instant,
    last_write: Instant,
    final_observation: Option<serde_json::Value>,
}
impl ImageTrace {
    pub fn from_environment() -> Option<Self> {
        let path = PathBuf::from(std::env::var_os("MODULE_ENGINE_PLAYER_IMAGE_TRACE")?);
        if !path.is_absolute() {
            eprintln!("Player image trace requires an absolute new file");
            return None;
        }
        match OpenOptions::new().create_new(true).write(true).open(path) {
            Ok(file) => Some(Self {
                file,
                remaining: 256,
                last: None,
                started: Instant::now(),
                last_write: Instant::now(),
                final_observation: None,
            }),
            Err(error) => {
                eprintln!("Player image trace: {error}");
                None
            }
        }
    }
    pub fn record(
        &mut self,
        rect: Rect,
        scale: f32,
        requested: Option<[u32; 2]>,
        frame: Option<&Frame>,
        render_calls: Option<u64>,
    ) {
        let Some(requested) = requested else {
            return;
        };
        let rendered = frame
            .map(|frame| [frame.width, frame.height])
            .unwrap_or([0, 0]);
        let key = (requested, rendered, scale.to_bits());
        let suspended = requested == [0, 0];
        let value = serde_json::json!({"elapsed_ms":self.started.elapsed().as_millis(),"logical_area":[rect.width(),rect.height()],"pixels_per_point":scale,"requested_pixels":requested,"displayed_frame_pixels":rendered,"settled_1_to_1":!suspended && requested.map(|v|v as usize)==rendered,"render_suspended":suspended,"renderer_calls":render_calls,"gpu_resident":frame.is_some_and(|frame|frame.gpu.is_some()),"scope":"Ordinary Player observation; no physical scanout or GPU timestamp measurement"});
        self.final_observation = Some(value.clone());
        if self.remaining == 0
            || (self.last == Some(key)
                && (!suspended || self.last_write.elapsed().as_millis() < 250))
        {
            return;
        }
        self.remaining -= 1;
        self.last = Some(key);
        self.last_write = Instant::now();
        if let Err(error) = writeln!(self.file, "{value}") {
            eprintln!("Player image trace write: {error}");
            self.remaining = 0;
        }
    }
}

impl Drop for ImageTrace {
    fn drop(&mut self) {
        // A rapid resize may exhaust the bounded intermediate log. Preserve
        // one final observation without turning tracing into an unbounded log.
        if let Some(mut value) = self.final_observation.take() {
            value["final_observation"] = true.into();
            if let Err(error) = writeln!(self.file, "{value}") {
                eprintln!("Player final image trace: {error}");
            }
        }
    }
}
