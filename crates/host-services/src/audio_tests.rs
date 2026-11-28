use super::*;
use crate::audio_output::endpoint_sample;
use crate::{HostServiceContext, validate_audio_bytes};
use kira::manager::backend::Renderer;
use module_engine_core::{AUDIO_CUE_EXTENSIONS, HostServiceRequest};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn service_context<'a>(
    root: &'a Path,
    paths: &'a BTreeMap<String, String>,
) -> HostServiceContext<'a> {
    HostServiceContext {
        asset_root: root,
        asset_paths: paths,
        retired: Arc::new(AtomicBool::new(false)),
    }
}

fn sound(cue: &str) -> GameEvent {
    GameEvent {
        name: "Sound".into(),
        payload: json!({ "cue": cue }),
    }
}

mod contracts {
    use super::*;
    fn control(payload: Value) -> GameEvent {
        GameEvent {
            name: "AudioControl".into(),
            payload,
        }
    }

    #[derive(Default)]
    struct BufferBackend(Option<Renderer>);

    impl Backend for BufferBackend {
        type Settings = ();
        type Error = ();

        fn setup(_: ()) -> Result<(Self, u32), ()> {
            Ok((Self::default(), 48_000))
        }

        fn start(&mut self, renderer: Renderer) -> Result<(), ()> {
            self.0 = Some(renderer);
            Ok(())
        }
    }

    fn render_frames(
        service: &mut AudioService<BufferBackend>,
        frames: u32,
    ) -> impl Iterator<Item = [f32; 2]> + '_ {
        let renderer = service
            .output
            .as_mut()
            .unwrap()
            .manager
            .backend_mut()
            .0
            .as_mut()
            .unwrap();
        renderer.on_start_processing();
        (0..frames).map(move |_| {
            let frame = renderer.process();
            [endpoint_sample(frame.left), endpoint_sample(frame.right)]
        })
    }

    fn render(service: &mut AudioService<BufferBackend>, ms: u32) -> [f32; 2] {
        render_frames(service, ms * 48).fold([0_f32; 2], |[left, right], frame| {
            [left.max(frame[0].abs()), right.max(frame[1].abs())]
        })
    }

    #[test]
    fn binding_decodes_all_formats_and_plays_through_kira() {
        let root = fixtures();
        let paths = [("legacy".into(), "tone.wav".into())].into();
        let context = service_context(&root, &paths);
        let config = parse_config(&json!({
            "event": "Sound", "cue_field": "cue", "control_event": "AudioControl",
            "cues": {"wav":"asset://legacy", "ogg":"tone.ogg", "mp3":"tone.mp3", "flac":"tone.flac"}
        }))
        .unwrap();
        let mut service =
            AudioService::<BufferBackend>::new(config, &context, || (), |_| None).unwrap();
        for extension in AUDIO_CUE_EXTENSIONS {
            let path = root.join(format!("tone.{extension}"));
            validate_audio_bytes(extension, &std::fs::read(&path).unwrap()).unwrap();
            assert!(decode(&path, 30).unwrap_err().contains("frame budget"));
            service.consume(&[sound(extension)]).unwrap();
            let [left, right] = render(&mut service, 220);
            assert!(left > 0.05 && right > 0.05, "{extension}: {left}, {right}");
            assert!(
                (left - right).abs() < 1e-6,
                "{extension}: expected centered mono"
            );
        }
        assert_eq!(service.diagnostics()["played"], 4);
        assert!(service.diagnostics()["output_error"].is_null());
        assert!(validate_audio_bytes("ogg", b"not an audio file").is_err());
    }

    #[test]
    fn gains_spatial_fades_and_retirement_change_kira_rendered_samples() {
        let root = fixtures();
        let paths = BTreeMap::new();
        let context = service_context(&root, &paths);
        let config = || {
            parse_config(&json!({
        "event": "Sound", "cue_field": "cue", "control_event": "AudioControl", "max_voices": 2,
        "cues": {
            "music": {"path":"tone.wav", "bus":"music", "looping":true},
            "effect": {"path":"tone.wav", "looping":true},
            "world": {"path":"tone.wav", "spatial":{"min_distance":1.0,"max_distance":15.0}, "looping":true}
        }
    })).unwrap()
        };
        let create = |context: &HostServiceContext<'_>| {
            AudioService::<BufferBackend>::new(config(), context, || (), |_| None).unwrap()
        };
        let mut left_owner = create(&context);
        let other_context = service_context(&root, &paths);
        let mut right_owner = create(&other_context);
        for service in [&mut left_owner, &mut right_owner] {
            let mut events = vec![sound("music")];
            events.extend(std::iter::repeat_with(|| sound("effect")).take(12));
            service.consume(&events).unwrap();
            assert_eq!(service.diagnostics()["active_voices"], 2);
            assert_eq!(service.diagnostics()["stolen_voices"], 11);
        }
        assert!(render(&mut right_owner, 25)[0] > 0.15);
        let full = render(&mut left_owner, 25)[0];
        left_owner
            .consume(&[control(
                json!({"command":"volume","bus":"sfx","volume":0.0,"fade_ms":20}),
            )])
            .unwrap();
        assert!(render(&mut left_owner, 10)[0] > full / 2.);
        render(&mut left_owner, 30);
        let music_only = render(&mut left_owner, 30)[0];
        assert!(music_only > 0.08 && music_only < full * 0.6);

        left_owner
            .consume(&[
                control(json!({"command":"stop","fade_ms":10})),
                control(json!({"command":"volume","bus":"sfx","volume":1.0})),
            ])
            .unwrap();
        render(&mut left_owner, 35);
        assert_eq!(render(&mut left_owner, 15), [0.; 2]);
        assert!(left_owner.consume(&[sound("world")]).is_err());
        left_owner
            .consume(&[GameEvent {
                name: "Sound".into(),
                payload: json!({"cue":"world", "position":[5.,0.,0.]}),
            }])
            .unwrap();
        let [l, r] = render(&mut left_owner, 30);
        assert!(
            r > 0.005 && r > 1.5 * l && r < music_only / 2.,
            "{l} {r} {music_only}"
        );
        left_owner
            .consume(&[control(
                json!({"command":"listener","position":[10.,0.,0.],"orientation":[0.,0.,0.,1.]}),
            )])
            .unwrap();
        let [l, r] = render(&mut left_owner, 30);
        assert!(l > 1.5 * r);
        context.retired.store(true, Ordering::Release);
        assert_eq!(render(&mut left_owner, 20), [0.; 2]);
        assert!(render(&mut right_owner, 30)[0] > 0.15);
    }

    #[test]
    fn disabled_binding_counts_events_and_start_failure_retries() {
        struct MissingBackend;
        impl Backend for MissingBackend {
            type Settings = ();
            type Error = &'static str;
            fn setup(_: ()) -> Result<(Self, u32), Self::Error> {
                Err("endpoint")
            }
            fn start(&mut self, _: Renderer) -> Result<(), Self::Error> {
                unreachable!()
            }
        }
        let root = fixtures();
        let paths = BTreeMap::new();
        let context = service_context(&root, &paths);
        let config = parse_config(&json!({
            "event":"player_action", "cue_field":"audio_id", "control_event":"gain",
            "cues":{"step":"tone.wav"}
        }))
        .unwrap();
        let mut service =
            AudioService::<MissingBackend>::new(config, &context, || (), |_| None).unwrap();
        let step = GameEvent {
            name: "player_action".into(),
            payload: json!({"audio_id":"step"}),
        };
        service.set_enabled(false);
        service.consume(&[sound("step"), step.clone()]).unwrap();
        assert_eq!(service.diagnostics()["played"], 1);
        assert!(service.diagnostics()["output_error"].is_null());
        service.set_enabled(true);
        for _ in 0..2 {
            assert!(
                service
                    .consume(std::slice::from_ref(&step))
                    .unwrap_err()
                    .contains("endpoint")
            );
            assert!(
                service.diagnostics()["output_error"]
                    .as_str()
                    .unwrap()
                    .contains("endpoint")
            );
        }
        assert!(service.reset().unwrap_err().contains("endpoint"));
        assert!(service.diagnostics()["output_error"].is_null());
        assert_eq!(service.diagnostics()["played"], 0);
    }

    #[test]
    fn wav_cues_reject_nonfinite_and_peaks_above_24_dbfs() {
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let error =
                validate_audio_bytes("wav", &crate::wav::tests::float32(&[invalid])).unwrap_err();
            assert!(error.contains("NaN or infinite"), "{error}");
        }
        for invalid in [16., -16.] {
            let error =
                validate_audio_bytes("wav", &crate::wav::tests::float32(&[invalid])).unwrap_err();
            assert!(error.contains("exceeds +24 dBFS"), "{error}");
        }
        validate_audio_bytes("wav", &crate::wav::tests::float32(&[15.8, -15.8])).unwrap();
    }

    #[test]
    fn voice_cancellation_matches_f64_at_normal_levels() {
        let root = fixtures();
        let paths = BTreeMap::new();
        let context = service_context(&root, &paths);
        let config = parse_config(&json!({
            "event":"Sound", "cue_field":"cue", "cues":{}, "max_voices":128
        }))
        .unwrap();
        let mut service =
            AudioService::<BufferBackend>::new(config, &context, || (), |_| None).unwrap();
        let mut reference = 0_f64;
        let mut events = Vec::new();
        for (name, value, count) in [
            ("up", 0.141_31, 64),
            ("down", -0.141_31, 63),
            ("tail", -0.126_82, 1),
        ] {
            let wav = crate::wav::tests::float32(&[value; 960]);
            let clip =
                crate::audio_codec::decode_source("wav", std::io::Cursor::new(&wav), 960).unwrap();
            service.cues.insert(
                name.into(),
                Cue {
                    sound: clip,
                    bus: Bus::Sfx,
                    spatial: None,
                },
            );
            events.extend(std::iter::repeat_with(|| sound(name)).take(count));
            reference += f64::from(value) * count as f64;
        }
        service.consume(&events).unwrap();
        assert_eq!(service.diagnostics()["active_voices"], 128);
        let maximum_error = render_frames(&mut service, 480)
            .skip(24)
            .flatten()
            .map(|sample| (f64::from(sample) - reference).abs())
            .fold(0., f64::max);
        eprintln!(
            "128-voice f32 vs f64 peak error {maximum_error:e} ({:.1} dBFS)",
            20. * maximum_error.log10()
        );
        assert!(
            maximum_error <= 1.0e-5,
            "maximum cancellation error {maximum_error:e} exceeds -100 dBFS"
        );
    }

    #[test]
    fn spatial_voices_wait_until_both_kira_reservations_are_free() {
        let root = fixtures();
        let paths = BTreeMap::new();
        let context = service_context(&root, &paths);
        let config = parse_config(&json!({
            "event":"Sound", "cue_field":"cue", "max_voices":2,
            "cues":{"world":{"path":"tone.wav", "looping":true,
                "spatial":{"min_distance":1.0,"max_distance":35.0}}}
        }))
        .unwrap();
        let mut service =
            AudioService::<BufferBackend>::new(config, &context, || (), |_| None).unwrap();
        let emitter = GameEvent {
            name: "Sound".into(),
            payload: json!({"cue":"world","position":[5.,0.,0.]}),
        };
        for _ in 0..3 {
            service
                .consume(&[emitter.clone(), emitter.clone()])
                .unwrap();
        }
        assert_eq!(service.diagnostics()["active_voices"], 2);
        assert_eq!(service.diagnostics()["stolen_voices"], 4);
        for _ in 0..4 {
            render(&mut service, 8);
        }
        service.consume(&[]).unwrap();
        let one_voice = render(&mut service, 12);
        // The first callback marks dropped emitters. The next reclaims them.
        render(&mut service, 8);
        service.consume(&[]).unwrap();
        let two_voices = render(&mut service, 12);
        assert!(
            one_voice[1] > 0.02 && two_voices[1] > 1.4 * one_voice[1],
            "{one_voice:?}, {two_voices:?}"
        );
    }
}

#[cfg(any(windows, target_os = "linux"))]
#[test]
#[ignore = "Requires the stand's default output and a confirmed loopback monitor; run sequentially"]
fn native_kira_loopback_receives_bound_cues_within_budget() {
    use cpal::{
        FromSample, Sample, SampleFormat, SizedSample,
        traits::{DeviceTrait, HostTrait, StreamTrait},
    };
    use std::{
        sync::mpsc::{SyncSender, sync_channel},
        thread,
        time::{Duration, Instant},
    };

    const ONSET: f32 = 0.035;

    fn capture<T>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        hits: SyncSender<Instant>,
    ) -> cpal::Stream
    where
        T: SizedSample + Sample,
        f32: FromSample<T>,
    {
        let channels = usize::from(config.channels);
        let rate = f64::from(config.sample_rate);
        let mut previous = false;
        device
            .build_input_stream(
                config,
                move |input: &[T], info: &cpal::InputCallbackInfo| {
                    let now = Instant::now();
                    let stamp = info.timestamp();
                    let delay = stamp.callback.duration_since(stamp.capture);
                    for (offset, frame) in input.chunks_exact(channels).enumerate() {
                        let audible = frame
                            .iter()
                            .any(|sample| sample.to_sample::<f32>().abs() > ONSET);
                        if audible && !previous {
                            let detected = now.checked_sub(delay).unwrap_or(now)
                                + Duration::from_secs_f64(offset as f64 / rate);
                            let _ = hits.try_send(detected);
                        }
                        previous = audible;
                    }
                },
                |error| eprintln!("Audio loopback stream: {error}"),
                None,
            )
            .unwrap()
    }

    let native = cpal::default_host();
    let output = native
        .default_output_device()
        .expect("default output device");
    let (monitor, support) = if cfg!(windows) {
        let support = output.default_output_config().unwrap();
        (output, support)
    } else {
        let id = std::env::var("MODULE_ENGINE_AUDIO_LOOPBACK_DEVICE").expect(
            "Set MODULE_ENGINE_AUDIO_LOOPBACK_DEVICE to the confirmed output monitor CPAL ID",
        );
        let monitor = native
            .device_by_id(&id.parse().unwrap())
            .expect("loopback monitor");
        let support = monitor.default_input_config().unwrap();
        (monitor, support)
    };
    eprintln!(
        "CPAL host {:?}, capture {}, config {support:?}",
        native.id(),
        monitor.id().unwrap()
    );
    let (tx, rx) = sync_channel(128);
    let stream = match support.sample_format() {
        SampleFormat::F32 => capture::<f32>(&monitor, support.config(), tx),
        SampleFormat::I16 => capture::<i16>(&monitor, support.config(), tx),
        SampleFormat::I32 => capture::<i32>(&monitor, support.config(), tx),
        format => panic!("Unsupported loopback format {format:?}"),
    };
    stream.play().unwrap();
    assert!(
        rx.recv_timeout(Duration::from_millis(150)).is_err(),
        "Loopback must start quiet"
    );

    let root = fixtures();
    let paths = [("legacy".into(), "tone.wav".into())].into();
    let context = service_context(&root, &paths);
    let mut registry = HostServiceRegistry::new();
    install_audio(&mut registry).unwrap();
    let request = HostServiceRequest {
        id: WAV_SERVICE_ID.into(),
        asset_references: ["asset://legacy", "tone.ogg", "tone.mp3", "tone.flac"]
            .map(str::to_owned)
            .into(),
        config: json!({
            "event":"Sound","cue_field":"cue","cues":{
                "wav":"asset://legacy", "ogg":"tone.ogg", "mp3":"tone.mp3", "flac":"tone.flac"
            }
        }),
    };
    let mut service = registry.build(&[request], &context, true).unwrap();
    let mut latencies = Vec::new();
    for (index, name) in std::iter::once("wav")
        .chain(AUDIO_CUE_EXTENSIONS.into_iter().cycle().take(16))
        .enumerate()
    {
        let source = decode(&root.join(format!("tone.{name}")), MAX_FRAMES).unwrap();
        let crossing = source
            .frames
            .iter()
            .position(|frame| endpoint_sample(frame.left).abs() > ONSET)
            .unwrap();
        let source_lead_ms = 1000. * crossing as f64 / f64::from(source.sample_rate);
        thread::sleep(Duration::from_millis(260));
        while rx.try_recv().is_ok() {}
        let dispatch = Instant::now();
        service.consume(&[sound(name)]).unwrap();
        let deadline = dispatch + Duration::from_secs(2);
        let onset = loop {
            let instant = rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("bound cue absent from loopback");
            if instant > dispatch {
                break instant;
            }
        };
        let elapsed = (onset - dispatch).as_secs_f64() * 1000.;
        let latency = elapsed - source_lead_ms;
        eprintln!(
            "Cue {name}, dispatch {index}, onset {elapsed:.3} ms, decoded source lead-in {source_lead_ms:.3} ms, output estimate {latency:.3} ms"
        );
        if index > 0 {
            latencies.push(latency);
        }
    }
    assert_eq!(service.diagnostics()[WAV_SERVICE_ID]["played"], 17);
    assert!(service.diagnostics()[WAV_SERVICE_ID]["output_error"].is_null());
    assert!(
        latencies.iter().all(|ms| (0. ..=50.).contains(ms)),
        "warm output budget 50 ms: {latencies:?}"
    );
    service.stop();
    thread::sleep(Duration::from_millis(180));
    while rx.try_recv().is_ok() {}
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
}
