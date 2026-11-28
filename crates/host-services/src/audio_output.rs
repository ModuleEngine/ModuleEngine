//! Per-host Kira mixer and an owned CPAL stream.
use cpal::{
    FromSample, Sample, SampleFormat, SizedSample, Stream, SupportedBufferSize,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use kira::{
    Volume,
    clock::clock_info::ClockInfoProvider,
    dsp::Frame,
    manager::{
        AudioManager, AudioManagerSettings, Capacities,
        backend::{Backend, Renderer},
    },
    modulator::value_provider::ModulatorValueProvider,
    sound::{
        PlaybackState,
        static_sound::{StaticSoundData, StaticSoundHandle},
    },
    spatial::{
        emitter::{EmitterHandle, EmitterSettings},
        listener::{ListenerHandle, ListenerSettings},
        scene::{SpatialSceneHandle, SpatialSceneSettings},
    },
    track::{
        TrackBuilder, TrackHandle,
        effect::{Effect, EffectBuilder},
    },
    tween::Tween,
};
use serde::Deserialize;
use std::{
    fmt::Debug,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    time::Duration,
};

pub(crate) struct NativeBackend {
    device: cpal::Device,
    supported: cpal::SupportedStreamConfig,
    stream: Option<Stream>,
    errors: Receiver<cpal::Error>,
    error_sender: SyncSender<cpal::Error>,
}

impl Backend for NativeBackend {
    type Settings = ();
    type Error = String;

    fn setup(_: ()) -> Result<(Self, u32), String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No default output device")?;
        let supported = device
            .default_output_config()
            .map_err(|error| error.to_string())?;
        let rate = supported.sample_rate();
        if supported.channels() == 0 {
            return Err("The audio device reported zero output channels".into());
        }
        let (error_sender, errors) = sync_channel(8);
        Ok((
            Self {
                device,
                supported,
                stream: None,
                errors,
                error_sender,
            },
            rate,
        ))
    }

    fn start(&mut self, renderer: Renderer) -> Result<(), String> {
        let mut config = self.supported.config();
        if let SupportedBufferSize::Range { min, max } = *self.supported.buffer_size() {
            config.buffer_size = cpal::BufferSize::Fixed(256_u32.clamp(min, max));
        }
        let stream = match self.supported.sample_format() {
            SampleFormat::F32 => self.build_stream::<f32>(config, renderer),
            SampleFormat::F64 => self.build_stream::<f64>(config, renderer),
            SampleFormat::U8 => self.build_stream::<u8>(config, renderer),
            SampleFormat::I16 => self.build_stream::<i16>(config, renderer),
            SampleFormat::I24 => self.build_stream::<cpal::I24>(config, renderer),
            SampleFormat::I32 => self.build_stream::<i32>(config, renderer),
            SampleFormat::U16 => self.build_stream::<u16>(config, renderer),
            format => return Err(format!("Unsupported audio device sample format {format:?}")),
        }
        .map_err(|error| error.to_string())?;
        stream.play().map_err(|error| error.to_string())?;
        self.stream = Some(stream);
        Ok(())
    }
}

impl NativeBackend {
    fn build_stream<T>(
        &self,
        config: cpal::StreamConfig,
        mut renderer: Renderer,
    ) -> Result<Stream, cpal::Error>
    where
        T: SizedSample + Sample + FromSample<f32>,
    {
        let channels = usize::from(config.channels);
        let errors = self.error_sender.clone();
        self.device.build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                renderer.on_start_processing();
                for samples in output.chunks_exact_mut(channels) {
                    let frame = renderer.process();
                    match samples {
                        [mono] => {
                            *mono =
                                T::from_sample(endpoint_sample((frame.left + frame.right) * 0.5))
                        }
                        [left, right, extra @ ..] => {
                            *left = T::from_sample(endpoint_sample(frame.left));
                            *right = T::from_sample(endpoint_sample(frame.right));
                            extra.fill(T::EQUILIBRIUM);
                        }
                        [] => unreachable!(),
                    }
                }
            },
            move |error| {
                let _ = errors.try_send(error);
            },
            None,
        )
    }

    pub fn poll_error(&mut self) -> Option<String> {
        self.errors.try_recv().ok().map(|error| error.to_string())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Bus {
    #[default]
    Sfx,
    Music,
}

pub(crate) fn endpoint_sample(sample: f32) -> f32 {
    sample.clamp(-1., 1.)
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum VolumeBus {
    Master,
    Music,
    Sfx,
}

#[derive(Clone, Copy)]
pub(crate) struct Gains {
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
}
impl Gains {
    pub fn set(&mut self, bus: VolumeBus, gain: f32) {
        match bus {
            VolumeBus::Master => self.master = gain,
            VolumeBus::Music => self.music = gain,
            VolumeBus::Sfx => self.sfx = gain,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ListenerPose {
    pub position: [f32; 3],
    pub orientation: [f32; 4],
}
impl Default for ListenerPose {
    fn default() -> Self {
        Self {
            position: [0.; 3],
            orientation: [0., 0., 0., 1.],
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpatialDistances {
    pub min_distance: f32,
    pub max_distance: f32,
}

struct MixerBus {
    track: TrackHandle,
    scene: SpatialSceneHandle,
    listener: ListenerHandle,
}
impl MixerBus {
    fn new<B: Backend>(
        manager: &mut AudioManager<B>,
        pose: ListenerPose,
        gain: f32,
        capacity: usize,
    ) -> Result<Self, String> {
        let track = manager
            .add_sub_track(TrackBuilder::new().volume(amplitude(gain)))
            .map_err(|error| error.to_string())?;
        let mut scene = manager
            .add_spatial_scene(
                SpatialSceneSettings::new()
                    .emitter_capacity(capacity)
                    .listener_capacity(1),
            )
            .map_err(|error| error.to_string())?;
        let listener = scene
            .add_listener(
                pose.position,
                pose.orientation,
                ListenerSettings::new().track(&track),
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            track,
            scene,
            listener,
        })
    }
}

struct Voice {
    bus: Bus,
    handle: StaticSoundHandle,
    // The emitter must outlive the sound and any stop fade.
    _emitter: Option<EmitterHandle>,
    fade_out: Option<Tween>,
}

impl Voice {
    fn stop(&mut self, tween: Tween) -> Result<(), String> {
        if self
            .fade_out
            .is_none_or(|old| !old.duration.is_zero() && tween.duration.is_zero())
        {
            self.handle.stop(tween).map_err(|error| error.to_string())?;
            self.fade_out = Some(tween);
        }
        Ok(())
    }
}

struct PendingVoice {
    data: StaticSoundData,
    bus: Bus,
    emitter: Option<([f32; 3], SpatialDistances)>,
}

pub(crate) struct AudioOutput<B: Backend> {
    voices: Vec<Voice>,
    pending: Vec<PendingVoice>,
    music: MixerBus,
    sfx: MixerBus,
    pub manager: AudioManager<B>,
    rendered_blocks: Arc<AtomicU64>,
    max_voices: usize,
}

pub(crate) fn amplitude(gain: f32) -> Volume {
    Volume::Amplitude(f64::from(gain))
}

pub(crate) fn fade(duration_ms: u32) -> Tween {
    Tween {
        duration: Duration::from_millis(u64::from(duration_ms)),
        ..Tween::default()
    }
}

impl<B: Backend> AudioOutput<B> {
    pub fn start(
        backend_settings: B::Settings,
        gains: Gains,
        pose: ListenerPose,
        max_voices: usize,
        retired: Arc<AtomicBool>,
    ) -> Result<Self, String>
    where
        B::Error: Debug,
    {
        let rendered_blocks = Arc::new(AtomicU64::new(0));
        let mut main_track = TrackBuilder::new().volume(amplitude(gains.master));
        main_track.add_effect(OutputGate {
            retired,
            rendered_blocks: rendered_blocks.clone(),
            silent: false,
        });
        // Leave room for replacements while the output callback retires old voices.
        let capacity = 2 * max_voices + 1;
        let mut manager = AudioManager::<B>::new(AudioManagerSettings {
            capacities: Capacities {
                sub_track_capacity: 2,
                spatial_scene_capacity: 2,
                command_capacity: capacity * 8,
                sound_capacity: capacity,
                ..Capacities::default()
            },
            main_track_builder: main_track,
            backend_settings,
        })
        .map_err(|error| format!("Kira could not start native audio: {error:?}"))?;
        let music = MixerBus::new(&mut manager, pose, gains.music, capacity)?;
        let sfx = MixerBus::new(&mut manager, pose, gains.sfx, capacity)?;
        Ok(Self {
            manager,
            music,
            sfx,
            voices: Vec::new(),
            pending: Vec::with_capacity(max_voices),
            max_voices,
            rendered_blocks,
        })
    }

    pub fn queue(
        &mut self,
        data: StaticSoundData,
        bus: Bus,
        emitter: Option<([f32; 3], SpatialDistances)>,
    ) -> Result<bool, String> {
        if bus == Bus::Music {
            self.stop(Some(Bus::Music), fade(0))?;
        }
        let active = self
            .voices
            .iter()
            .filter(|voice| voice.fade_out.is_none())
            .count();
        let stolen = active + self.pending.len() >= self.max_voices;
        if stolen {
            if let Some(oldest) = self
                .voices
                .iter_mut()
                .find(|voice| voice.fade_out.is_none() && voice.bus == Bus::Sfx)
            {
                oldest.stop(fade(0))?;
            } else {
                let oldest = self
                    .pending
                    .iter()
                    .position(|voice| voice.bus == Bus::Sfx)
                    .expect("A full voice set has an effect voice");
                self.pending.remove(oldest);
            }
        }
        self.pending.push(PendingVoice { data, bus, emitter });
        Ok(stolen)
    }

    pub fn finish_batch(&mut self) -> Result<(), String> {
        // Kira releases reservations at callback boundaries. Keep selected
        // voices pending until slots are free and shorten older tails under load.
        let available = self
            .manager
            .sound_capacity()
            .saturating_sub(self.manager.num_sounds());
        for voice in self
            .voices
            .iter_mut()
            .filter(|voice| {
                voice
                    .fade_out
                    .is_some_and(|tween| !tween.duration.is_zero())
            })
            .take(self.pending.len().saturating_sub(available))
        {
            voice.stop(fade(0))?;
        }
        for _ in 0..available {
            let Some(next) = self.pending.first() else {
                break;
            };
            let destination = match next.bus {
                Bus::Music => &self.music,
                Bus::Sfx => &self.sfx,
            };
            // Kira marks an emitter finished during processing and reclaims it
            // at a later callback boundary after reap drops its handle.
            if next.emitter.is_some()
                && destination.scene.num_emitters() >= self.manager.sound_capacity()
            {
                break;
            }
            let voice = self.pending.remove(0);
            self.play(voice)?;
        }
        Ok(())
    }

    fn play(
        &mut self,
        PendingVoice {
            mut data,
            bus,
            emitter,
        }: PendingVoice,
    ) -> Result<(), String> {
        let destination = match bus {
            Bus::Music => &mut self.music,
            Bus::Sfx => &mut self.sfx,
        };
        let emitter = if let Some((position, distances)) = emitter {
            let emitter = destination
                .scene
                .add_emitter(
                    position,
                    EmitterSettings::new()
                        .distances((distances.min_distance, distances.max_distance)),
                )
                .map_err(|error| error.to_string())?;
            data.settings = data.settings.output_destination(&emitter);
            Some(emitter)
        } else {
            data.settings = data.settings.output_destination(&destination.track);
            None
        };
        let handle = self.manager.play(data).map_err(|error| error.to_string())?;
        self.voices.push(Voice {
            bus,
            handle,
            _emitter: emitter,
            fade_out: None,
        });
        Ok(())
    }

    pub fn stop(&mut self, bus: Option<Bus>, tween: Tween) -> Result<(), String> {
        self.pending
            .retain(|voice| bus.is_some_and(|bus| voice.bus != bus));
        for voice in self
            .voices
            .iter_mut()
            .filter(|voice| bus.is_none_or(|bus| voice.bus == bus))
        {
            voice.stop(tween)?;
        }
        Ok(())
    }

    pub fn set_gain(&mut self, bus: VolumeBus, gain: f32, tween: Tween) -> Result<(), String> {
        let gain = amplitude(gain);
        match bus {
            VolumeBus::Master => self.manager.main_track().set_volume(gain, tween),
            VolumeBus::Music => self.music.track.set_volume(gain, tween),
            VolumeBus::Sfx => self.sfx.track.set_volume(gain, tween),
        }
        .map_err(|error| error.to_string())
    }

    pub fn set_listener(&mut self, pose: ListenerPose, tween: Tween) -> Result<(), String> {
        for bus in [&mut self.music, &mut self.sfx] {
            bus.listener
                .set_position(pose.position, tween)
                .map_err(|error| error.to_string())?;
            bus.listener
                .set_orientation(pose.orientation, tween)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn reap(&mut self) {
        self.voices
            .retain(|voice| voice.handle.state() != PlaybackState::Stopped);
    }

    pub fn active_voices(&self) -> usize {
        self.voices
            .iter()
            .filter(|voice| {
                voice.fade_out.is_none() && voice.handle.state() != PlaybackState::Stopped
            })
            .count()
            + self.pending.len()
    }

    pub fn rendered_blocks(&self) -> u64 {
        self.rendered_blocks.load(Ordering::Relaxed)
    }
}

struct OutputGate {
    retired: Arc<AtomicBool>,
    rendered_blocks: Arc<AtomicU64>,
    silent: bool,
}
impl EffectBuilder for OutputGate {
    type Handle = ();

    fn build(self) -> (Box<dyn Effect>, ()) {
        (Box::new(self), ())
    }
}
impl Effect for OutputGate {
    fn on_start_processing(&mut self) {
        self.silent = self.retired.load(Ordering::Acquire);
        if !self.silent {
            self.rendered_blocks.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn process(
        &mut self,
        input: Frame,
        _: f64,
        _: &ClockInfoProvider,
        _: &ModulatorValueProvider,
    ) -> Frame {
        if self.silent { Frame::ZERO } else { input }
    }
}
