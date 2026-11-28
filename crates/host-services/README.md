# Optional host services

[English](README.md) | [Русский](README.ru.md) | [Documentation](../../docs/README.md)

The default crate builds only the generic factory registry and service owner. It has no renderer, windowing, game-package or platform-playback dependency. Enable `audio` and call `install_audio` for `audio.wav`; enable `persistence` and call `install_persistence_at(&mut registry, state_root)` for `persistence.json`. Installing either factory performs no file or device access.

Game contributions discover `HostServiceRequest` values from their authored schedule. The host supplies checked asset ID/path metadata, the asset root and its retirement atomic. Omitted or disabled requests construct no instances and perform no factory calls, asset resolution, file reads, directory creation or device calls. An enabled request for a missing factory fails explicitly. Headless hosts keep services disabled by default.

`HostServices::initial_module_data()` merges source-owned bootstrap keys and rejects empty/duplicate keys. A new `GameSession` accepts these opaque values through `seed_module_data` before its first simulation operation, validating the entire candidate before publication. Exact keys, unknown payload fields and explicit null survive. This does not restore the ECS, clock, executor, Custom worker, input edges or service internals. The source package reads its own namespace during initialization and validates/reconstructs its resume and undo data. An authored-preview replacement seeds its fresh candidate before publication; a rejected candidate preserves the previous document/session. Restart starts a fresh session and does not automatically reload the save.

Service effects happen after committed game operations, including an accepted prefix when a later fixed tick in the same catchup call rejects. Rejected tick events are not delivered, and a no-commit error does not replay earlier events. A service failure remains visible as host activity and does not pretend that the committed tick was rolled back. Every service receives the committed events and reset independently; failures are aggregated so an audio error cannot suppress a save. Interpolated presentation invokes no service. `set_enabled(false)` suppresses external effects while retaining loaded resources and logical diagnostics; enabling an owner built with services disabled cannot create omitted services or perform hidden IO. Stop disables effects and releases only the owner's resources.

## Versioned JSON persistence

The application chooses a writable state root separately from authored content and packaged assets. A source-owned request configures:

```json
{
  "namespace": "example-game/player-profile",
  "data_key": "example.resume/1",
  "write_event": "example.save",
  "version": 1,
  "slot": "state",
  "payload_field": "data",
  "invalid_save": "reject",
  "reset_event": null
}
```

`slot` and `payload_field` default to `state` and `data`; `invalid_save` defaults to `reject`, and `reset_event` is absent by default. Namespace and slot identities become hex-encoded path segments below the application root, preserving case/Unicode identity without treating the identity as a path. Different games/profiles use different namespaces. The save envelope stores format version 1, the configured namespace/data key/payload version, and opaque JSON `data`. A missing file seeds no key; saved null seeds an explicit null. With the unchanged default policy, corrupt, oversized, unknown-format, mismatched identity or mismatched payload-version saves and load IO failures reject construction and remain unchanged. Payload migration is not automatic; a later source contract must authorize it.

An explicitly selected `invalid_save:"read_only"` policy keeps an enabled service usable after a load failure, with no bootstrap value and `write_blocked:true`. `last_error` is latched, and `error_kind` distinguishes `invalid_save` from `io` (including unreadable paths/permissions). Configuration errors still reject construction. This policy never silently repairs, replaces or discards a save; a game may continue without saving while its UI reports the blocked storage. The game still validates its typed payload: a structurally valid envelope is opaque to the service, even if its payload is invalid for the source-owned game schema.

Set an optional, distinct `reset_event`, for example `example.reset-storage`, to permit an explicit `GameEvent::new("example.reset-storage", json!({"confirm":true}))`. A confirmed enabled reset reserves a unique recovery sibling and atomically moves only the existing regular save file into it, preserving its bytes; it then clears the loaded value/error and unblocks writes. Recovery files are retained and reported by `recovery_file`. It never moves a directory or follows a symlink as a save. A reset followed by the normal fresh-save event in the same committed event batch is supported, including when the old envelope was valid but the source rejected its typed payload. If quarantine fails, the original stays in place, writes remain blocked and the reset error is latched. If a subsequent fresh save fails, the old recovery file remains intact. Missing/false `confirm`, disabled services, retired owners and unconfigured reset events cannot discard the published save. Application confirmation and the choice to reset belong to the source/UI; ordinary host restart is not this event.

The configured event, for example `GameEvent::new("example.save", json!({"data": payload}))`, requests a write. The source package owns the full payload, including any resume/history schema. Only matching enabled events perform persistence IO; idle frames do not. An identical published payload is a no-op. Reads and serialized envelopes are bounded to 8 MiB. A write flushes a uniquely created sibling candidate before atomically replacing the published entry. Serialization/write/flush/publication failure preserves the previous published save; owned temporary files are removed. The retired flag is rechecked before publication. A live host also revalidates an existing envelope before replacement so it cannot silently overwrite an externally corrupted or future-version save.

Ordinary host reset clears persistence counters without deleting the save, clearing a latched error or unblocking storage. `requests` counts matching writes, including disabled ones; `writes` counts actual replacements. `reset_requests` and `resets` track explicit reset requests and successful resets. `loaded` distinguishes a loaded/saved value from absence. `write_blocked`, `last_error`, `error_kind` and `recovery_file` are current UI diagnostics; idle consumption does not clear errors. A successful requested write (including an already-identical published value) or confirmed reset clears its current error. Write/publication failures remain retryable. With the opt-in policy, a newly invalid/unreadable existing envelope blocks replacement until reset; the default reject policy fails that operation and revalidates on later retries. Publication is serialized between services in this process; it is not a cross-process merge, compare-and-swap or locking protocol. The flushed candidate and atomic replacement protect ordinary publication failure, not a guarantee against every power-loss/filesystem failure. Disk work occurs on the host owner thread for explicit save/reset events; measure save latency when setting a game's frame-time criteria.

<a id="concurrent-wav-music-and-effects"></a>

## Audio cues and mixing

The service ID remains `audio.wav`. Requests still bind a named game event and the payload field that carries the cue ID. Declare the clip paths and asset IDs in the request's `asset_references` so Player includes them. The example uses the same event for a legacy string and a spatial cue.

```json
{
  "event": "example.sound",
  "cue_field": "cue",
  "control_event": "example.audio-control",
  "master_volume": 1.0,
  "music_volume": 0.5,
  "sfx_volume": 0.8,
  "max_voices": 32,
  "cues": {
    "hit": "asset://hit-wav",
    "theme": {"path":"asset://theme-ogg","bus":"music","volume":0.6,"looping":true,"fade_in_ms":150},
    "bell": {"path":"asset://bell-flac","spatial":{"min_distance":1.0,"max_distance":24.0}}
  }
}
```

A legacy string plays a one-shot at full cue gain on `sfx`. A rich cue also defaults to that bus, volume 1, no looping and no fade. WAV, OGG/Vorbis, MP3 and FLAC accept mono or stereo at 8,000–192,000 Hz. WAV retains RIFF validation, PCM8/16/24/32 and extensible PCM/float channel placement rules. Import and runtime loading reject NaN, infinity and WAV peaks above +24 dBFS with a diagnostic. Clip inputs are bounded to 128 MiB each and the decoded cues to 32 Mi stereo frames total. Each owner can bind up to 256 cues.

Kira accumulates in `f32`. The cancellation test sums 128 voices at ordinary levels and compares each sample with an `f64` reference at a maximum absolute error of −100 dBFS. Revisit `f64` support in Kira if an actual asset requires greater precision.

Each owner has its own Kira mixer and CPAL output on Windows and Linux. Effects overlap. Music replaces the owner's previous music voice. The default limit of 32 active voices can be configured from 2 to 128. At the limit a new voice replaces the oldest effect. Kira can shorten old fade tails when the output is waiting for voice slots. Per-cue, master and bus gains multiply and each is within 0..=1. The endpoint receives clamped samples.

The configured control event accepts the following payloads.

- `{"command":"volume","bus":"music","volume":0.25,"fade_ms":80}` adjusts one bus. The bus can be `master`, `music` or `sfx`.
- `{"command":"stop","bus":"sfx","fade_ms":20}` stops one bus. Omit `bus` to stop both.
- `{"command":"listener","position":[0,1,0],"orientation":[0,0,0,1],"fade_ms":40}` moves the spatial listener. The orientation is a unit quaternion in XYZW order. Identity faces −Z with +X to the right and +Y up.

With a spatial cue, its usual game event also carries `"position":[x,y,z]` in world units. The emitter pans and attenuates using the configured range. Listener controls apply to both spatial buses. Fade durations default to zero and may be at most 60,000 ms.

The device opens on the first enabled known cue. Muting still counts matching `played` events. Muting, reset and Stop release the owner's stream and voices. Reset clears counters and listener pose while preserving bus volumes. The current `output_error` reports native startup and streaming errors. A later cue retries failed output. The retirement flag silences the outgoing Kira mix while ownership is released.

`rendered_blocks` counts blocks processed by Kira. The `r63-audio-contract` check runs the same mixer with in-memory output and checks formats, gains, fading, spatial audio, retirement and error handling. The explicit `audio-loopback` check opens the default Windows WASAPI output for loopback, or on Linux uses the confirmed capture device identified by `MODULE_ENGINE_AUDIO_LOOPBACK_DEVICE`. It reports first-cue onset separately and then measures four warm plays per format. The 50 ms budget applies to the monotonic time from dispatch to the callback containing the onset, less the decoded clip's lead-in. This upper bound includes capture delivery time. The log also retains the device's sample timestamp estimate. The check requires a quiet output and an uncontended run. Loopback measures the operating system path before the physical speaker.

```sh
cargo test --locked -p module-engine-host-services --features audio --lib audio::tests::native_kira_loopback_receives_bound_cues_within_budget -- --exact --ignored --nocapture --test-threads=1
```
