//! Source-owned game event bindings and optional native audio service.
use crate::audio_codec::{MAX_FRAMES, decode};
use crate::audio_output::{
    AudioOutput, Bus, Gains, ListenerPose, NativeBackend, SpatialDistances, VolumeBus, amplitude,
    fade,
};
use crate::{HostService, HostServiceContext, HostServiceRegistry, WAV_SERVICE_ID};
use kira::{manager::backend::Backend, sound::static_sound::StaticSoundData};
use module_engine_core::GameEvent;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fmt::Debug,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_FADE_MS: u32 = 60_000;

#[derive(Deserialize)]
#[serde(untagged)]
enum CueConfig {
    Path(String),
    Settings(CueSettings),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CueSettings {
    path: String,
    #[serde(default)]
    bus: Bus,
    #[serde(default = "one")]
    volume: f32,
    #[serde(default)]
    looping: bool,
    #[serde(default)]
    fade_in_ms: u32,
    #[serde(default)]
    spatial: Option<SpatialDistances>,
}
impl From<CueConfig> for CueSettings {
    fn from(cue: CueConfig) -> Self {
        match cue {
            CueConfig::Path(path) => Self {
                path,
                bus: Bus::Sfx,
                volume: 1.,
                looping: false,
                fade_in_ms: 0,
                spatial: None,
            },
            CueConfig::Settings(settings) => settings,
        }
    }
}
fn one() -> f32 {
    1.
}
fn default_voices() -> usize {
    32
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AudioConfig {
    cues: BTreeMap<String, CueConfig>,
    event: String,
    cue_field: String,
    #[serde(default)]
    control_event: Option<String>,
    #[serde(default = "one")]
    master_volume: f32,
    #[serde(default = "one")]
    music_volume: f32,
    #[serde(default = "one")]
    sfx_volume: f32,
    #[serde(default = "default_voices")]
    max_voices: usize,
}
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum AudioControl {
    Volume {
        bus: VolumeBus,
        volume: f32,
        #[serde(default)]
        fade_ms: u32,
    },
    Stop {
        #[serde(default)]
        bus: Option<Bus>,
        #[serde(default)]
        fade_ms: u32,
    },
    Listener {
        position: [f32; 3],
        orientation: [f32; 4],
        #[serde(default)]
        fade_ms: u32,
    },
}

/// Compatibility name for the platform support of the audio.wav service.
pub const WAV_PLAYBACK_SUPPORTED: bool = cfg!(any(windows, target_os = "linux"));

pub fn install_audio(registry: &mut HostServiceRegistry) -> Result<(), String> {
    registry.register(WAV_SERVICE_ID, |request, context| {
        if !WAV_PLAYBACK_SUPPORTED {
            return Err("Native audio playback requires Windows or Linux".into());
        }
        let config = parse_config(&request.config)?;
        Ok(Box::new(AudioService::<NativeBackend>::new(
            config,
            context,
            || (),
            NativeBackend::poll_error,
        )?))
    })?;
    registry.describe_service(
        WAV_SERVICE_ID,
        crate::audio_capability_metadata(),
        module_engine_core::CapabilityFact::known(
            WAV_PLAYBACK_SUPPORTED,
            if WAV_PLAYBACK_SUPPORTED {
                "Kira native audio backend installed; device availability is checked on playback"
            } else {
                "Native audio playback requires Windows or Linux"
            },
        ),
    )
}

fn parse_config(value: &Value) -> Result<AudioConfig, String> {
    serde_json::from_value(value.clone())
        .map_err(|error| format!("Invalid audio service configuration: {error}"))
}

#[derive(Clone)]
struct Cue {
    sound: StaticSoundData,
    bus: Bus,
    spatial: Option<SpatialDistances>,
}

struct AudioService<B: Backend> {
    cues: BTreeMap<String, Cue>,
    event: String,
    cue_field: String,
    control_event: Option<String>,
    output: Option<AudioOutput<B>>,
    backend_settings: fn() -> B::Settings,
    poll_error: fn(&mut B) -> Option<String>,
    gains: Gains,
    listener: ListenerPose,
    max_voices: usize,
    retired: Arc<AtomicBool>,
    output_error: Option<String>,
    played: u64,
    stolen: u64,
    enabled: bool,
}

fn volume(value: f32) -> Result<f32, String> {
    if (0. ..=1.).contains(&value) {
        Ok(value)
    } else {
        Err("Audio volume must be finite and between 0 and 1".into())
    }
}

fn validate_fade(ms: u32) -> Result<(), String> {
    if ms > MAX_FADE_MS {
        return Err(format!("Audio fade must be within 0..={MAX_FADE_MS} ms"));
    }
    Ok(())
}

fn position_ok(position: [f32; 3]) -> bool {
    position
        .into_iter()
        .all(|component| component.abs() <= 1.0e7)
}

fn listener_pose(position: [f32; 3], mut orientation: [f32; 4]) -> Result<ListenerPose, String> {
    if !position_ok(position) {
        return Err("Expected a finite audio listener position within world bounds".into());
    }
    let length_sq: f32 = orientation
        .into_iter()
        .map(|component| component * component)
        .sum();
    if !length_sq.is_finite() || (length_sq - 1.).abs() > 0.001 {
        return Err("Expected a normalized audio listener orientation [x, y, z, w]".into());
    }
    let length = length_sq.sqrt();
    for component in &mut orientation {
        *component /= length;
    }
    Ok(ListenerPose {
        position,
        orientation,
    })
}

impl<B: Backend> AudioService<B> {
    fn new(
        config: AudioConfig,
        context: &HostServiceContext<'_>,
        backend_settings: fn() -> B::Settings,
        poll_error: fn(&mut B) -> Option<String>,
    ) -> Result<Self, String> {
        if config.event.trim().is_empty()
            || config.cue_field.trim().is_empty()
            || config
                .control_event
                .as_ref()
                .is_some_and(|event| event.trim().is_empty() || event == &config.event)
        {
            return Err(
                "Audio cue event/field must be nonempty and control event must be distinct".into(),
            );
        }
        if !(2..=128).contains(&config.max_voices) || config.cues.len() > 256 {
            return Err("Audio service needs 2..=128 voices and at most 256 cues".into());
        }
        let gains = Gains {
            master: volume(config.master_volume)?,
            music: volume(config.music_volume)?,
            sfx: volume(config.sfx_volume)?,
        };
        let mut remaining_frames = MAX_FRAMES;
        let mut cues = BTreeMap::new();
        for (name, cue) in config.cues {
            if name.trim().is_empty() {
                return Err("Audio cue names must be nonempty".into());
            }
            let settings = CueSettings::from(cue);
            let gain = volume(settings.volume)?;
            validate_fade(settings.fade_in_ms)?;
            if let Some(spatial) = settings.spatial
                && !(spatial.min_distance >= 0.
                    && spatial.max_distance > spatial.min_distance
                    && spatial.max_distance <= 1.0e7)
            {
                return Err(format!(
                    "Audio cue '{name}' needs 0 <= min_distance < max_distance <= 10000000"
                ));
            }
            let relative = if let Some(id) = settings.path.strip_prefix("asset://") {
                context
                    .asset_paths
                    .get(id)
                    .ok_or_else(|| format!("Audio cue '{name}' references unknown asset '{id}'"))?
            } else {
                &settings.path
            };
            let path = module_engine_project::resolve_relative(context.asset_root, relative)?;
            let mut sound = decode(&path, remaining_frames)
                .map_err(|error| format!("Audio cue '{name}': {error}"))?;
            remaining_frames -= sound.frames.len();
            sound.settings = sound.settings.volume(amplitude(gain));
            if settings.looping {
                sound.settings = sound.settings.loop_region(..);
            }
            if settings.fade_in_ms != 0 {
                sound.settings = sound.settings.fade_in_tween(fade(settings.fade_in_ms));
            }
            cues.insert(
                name,
                Cue {
                    sound,
                    bus: settings.bus,
                    spatial: settings.spatial,
                },
            );
        }
        Ok(Self {
            event: config.event,
            cue_field: config.cue_field,
            control_event: config.control_event,
            cues,
            gains,
            backend_settings,
            poll_error,
            max_voices: config.max_voices,
            output: None,
            listener: ListenerPose::default(),
            retired: context.retired.clone(),
            output_error: None,
            played: 0,
            stolen: 0,
            enabled: true,
        })
    }

    fn control(&mut self, command: AudioControl) -> Result<(), String> {
        match command {
            AudioControl::Volume {
                bus,
                volume: gain,
                fade_ms,
            } => {
                let gain = volume(gain)?;
                validate_fade(fade_ms)?;
                if let Some(output) = &mut self.output {
                    output.set_gain(bus, gain, fade(fade_ms))?;
                }
                self.gains.set(bus, gain);
            }
            AudioControl::Stop { bus, fade_ms } => {
                validate_fade(fade_ms)?;
                if let Some(output) = &mut self.output {
                    output.stop(bus, fade(fade_ms))?;
                }
            }
            AudioControl::Listener {
                position,
                orientation,
                fade_ms,
            } => {
                validate_fade(fade_ms)?;
                self.listener = listener_pose(position, orientation)?;
                if let Some(output) = &mut self.output {
                    output.set_listener(self.listener, fade(fade_ms))?;
                }
            }
        }
        Ok(())
    }

    fn play(&mut self, cue: Cue, payload: &Value) -> Result<(), String>
    where
        B::Error: Debug,
    {
        let mut emitter = None;
        if let Some(distances) = cue.spatial {
            let position: [f32; 3] = serde_json::from_value(payload["position"].clone())
                .map_err(|_| "A spatial audio cue requires a position [x, y, z]".to_string())?;
            if !position_ok(position) {
                return Err("Expected a finite audio emitter position within world bounds".into());
            }
            emitter = Some((position, distances));
        }
        self.played = self.played.saturating_add(1);
        if !self.enabled || self.retired.load(Ordering::Acquire) {
            return Ok(());
        }
        if self.output.is_none() {
            let output = AudioOutput::start(
                (self.backend_settings)(),
                self.gains,
                self.listener,
                self.max_voices,
                self.retired.clone(),
            )
            .inspect_err(|error| self.output_error = Some(error.clone()))?;
            self.output = Some(output);
            self.output_error = None;
        }
        let stolen = self
            .output
            .as_mut()
            .expect("Started owner output")
            .queue(cue.sound, cue.bus, emitter)?;
        if stolen {
            self.stolen = self.stolen.saturating_add(1);
        }
        Ok(())
    }

    fn dispatch(&mut self, events: &[GameEvent]) -> Result<(), String>
    where
        B::Error: Debug,
    {
        for event in events {
            if self.control_event.as_ref() == Some(&event.name) {
                let command = serde_json::from_value(event.payload.clone())
                    .map_err(|error| format!("Invalid audio control: {error}"))?;
                self.control(command)?;
            } else if event.name == self.event
                && let Some(name) = event.payload.get(&self.cue_field).and_then(Value::as_str)
                && let Some(cue) = self.cues.get(name).cloned()
            {
                self.play(cue, &event.payload)?;
            }
        }
        Ok(())
    }
}

impl<B: Backend + Send> HostService for AudioService<B>
where
    B::Error: Debug,
{
    fn consume(&mut self, events: &[GameEvent]) -> Result<(), String> {
        if self.retired.load(Ordering::Acquire) {
            self.output = None;
        }
        if let Some(output) = &mut self.output {
            output.reap();
            if let Some(error) = (self.poll_error)(output.manager.backend_mut()) {
                let error = format!("Kira audio output: {error}");
                self.output_error = Some(error.clone());
                self.output = None;
                return Err(error);
            }
        }
        let dispatched = self.dispatch(events);
        let played = self
            .output
            .as_mut()
            .map_or(Ok(()), AudioOutput::finish_batch);
        dispatched.and(played)
    }

    fn reset(&mut self) -> Result<(), String> {
        self.output = None;
        self.played = 0;
        self.stolen = 0;
        self.listener = ListenerPose::default();
        self.output_error.take().map_or(Ok(()), Err)
    }

    fn set_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.output = None;
        }
        self.enabled = enabled;
    }

    fn stop(&mut self) {
        self.set_enabled(false);
    }

    fn diagnostics(&self) -> Value {
        json!({
            "played": self.played, "enabled": self.enabled, "cues": self.cues.len(),
            "platform_playback": WAV_PLAYBACK_SUPPORTED,
            "active_voices": self.output.as_ref().map_or(0, AudioOutput::active_voices),
            "stolen_voices": self.stolen,
            "master_volume": self.gains.master, "music_volume": self.gains.music, "sfx_volume": self.gains.sfx,
            "output_error": self.output_error,
            "rendered_blocks": self.output.as_ref().map_or(0, AudioOutput::rendered_blocks),
        })
    }
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod tests;
