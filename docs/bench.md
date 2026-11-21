# R0.2 native benchmarks

[English](bench.md) | [Русский](ru/bench.md) | [Documentation](README.md)

Run from a full source checkout with Python 3.9+, the native Rust toolchain,
DXC, and an interactive 1920×1080-capable desktop with a Vulkan 1.3 adapter
supported by the [Player presentation bridge](player.md). On Windows,
`python` must resolve to an installed interpreter. Use `python3` on Linux
when that is the system command.

```sh
python scripts/bench.py
```

Leave the benchmark window unobstructed and idle. Pause other GPU workloads
and Cargo builds for the series. Keep driver, power profile, display mode, and
monitor fixed. Use `--diagnostic` if competing work is known to be running.
S5 needs about 14 GB of available system memory on a 64-bit build, and the
renderer probes its actual Vulkan buffer and heap limits before expansion.
Allow space for the normal Release build and roughly 40 MB of evidence per
series, including the executable. The scene generator uses memory for dense
geometry and does not generate large source files.

## Fixture inputs

| Scene | Input and verified load |
| --- | --- |
| S1 | Existing Crate Garden source template and level-one Play action with a fresh profile. The native Player report must show Playing at physical 1920×1080. |
| S2 | Existing Two Arenas source template with the game controller and ordinary idle input. The native Player report must show Playing, 20 living enemies, and physical 1920×1080. |
| S3 | 11,000 draw items, of which 1,000 move at fixed 60 Hz fixture time. The loader checks 200 distinct mesh payloads, 500 distinct 8×8 textures, and 2,000 Core ECS entities with eight dynamic components each. The entities correspond to a subset of the render scene, and all moving items are in that subset. Each tick queries the eight columns and updates motion data and transforms. The benchmark then extracts those transforms into its neutral scene. |
| S4 | The Khronos Sponza interior (262,267 triangles, 103 primitives) plus the two pinned Khronos models, loaded through the stock glTF importer. The view looks west along the nave from inside its east end. The scene inventory contains all 256 point-light records and the directional sun used by the current Forward shader. Legacy Forward draws the interior and sun. Point-light evaluation needs R5, so the current timing covers the geometry and existing shader only. |
| S5 | Forty indexed patches with a combined 20,000,000 original triangles and 10,040,040 vertices before expansion. The legacy renderer writes a 4.32 GB expanded vertex stream per frame and requests one 8.64 GB host-visible Vulkan buffer. An allocation or memory-headroom limit retains the loaded counts and reason in JSON. |

S3–S5 compile from [fixed Rust fixtures](../crates/editor/src/bench/fixtures.rs)
with no random inputs. The app checks submitted triangle and object counts on every
completed frame. S4's `Avocado.glb` and `BoxVertexColors.glb` are unmodified
CC0-1.0 models from [Khronos glTF Sample Assets at e124e8d](https://github.com/KhronosGroup/glTF-Sample-Assets/tree/e124e8d8d7330a5372bc2b4d00d39a4a9d7997ea/Models).
The [model files and license notices](../examples/bench/khronos/) are about
8.1 MB together. The current static importer applies base color and does not
import vertex colors or every PBR channel.

Sponza is not redistributed: its model files are under the
[CRYENGINE Limited License Agreement](https://github.com/KhronosGroup/glTF-Sample-Assets/blob/e124e8d8d7330a5372bc2b4d00d39a4a9d7997ea/Models/Sponza/LICENSE.md).
[`examples/bench/sponza/manifest.json`](../examples/bench/sponza/manifest.json)
pins all 71 files (about 53 MB) by SHA-256 at the same Khronos commit.
`scripts/bench.py` downloads missing files into the Git-ignored
`.tools/bench-assets/sponza/`, and the fixture checks every hash again before
loading. Two fixture adaptations are needed for the legacy renderer. Its static
importer accepts only opaque materials, so the three alpha-mask materials
(leaves, chains, plant) are drawn opaque from a derived `Sponza.bench.gltf`
beside the download. Its base-color atlas is at most 8192 px high, so every S4
base-color texture is box-filtered to at most 256 px per edge. Geometry is
unchanged. `cargo test -p module-engine-editor --features bench bench::`
checks the S3–S5 fixture contracts without a GPU; the S4 test skips with a
message when Sponza has not been downloaded.

## Measurement and repeatability

The launcher builds the Editor once in Release with the source-only `bench`
feature. It records the source inventory and compilers, copies the built binary
into the series directory, and runs one unscored S1–S5 pass to prewarm shader
and Custom caches. It then runs three sequential S1–S5 passes in fresh
processes with identical inputs and fresh game profiles. Each process discards
30 warmup frames and measures 240 frames, except S5, which discards five and
measures 30. No authored project or installed Editor or Player is used.

| Scene | Per-run arithmetic means compared |
| --- | --- |
| S1 | Native Player UI-observed unique-frame interval |
| S2 | Native Player UI-observed unique-frame interval and reported GameHost step time |
| S3 | Window-observed frame interval, CPU render/submission time, and Core ECS fixture tick |
| S4, S5 | Window-observed frame interval and CPU render/submission time |

S3–S5 exercise the current Vulkan graph, GPU completion and the Player's
native GPU texture bridge in a real window. Their diagnostic app admits one
submission at a time on the window thread, observes completion on a later UI
callback and submits the next frame in that same callback. This includes window
scheduling. The launcher requests `MODULE_ENGINE_GUI_VSYNC=off` for every scene.
WGPU can fall back to display pacing when an unsynchronized mode is unavailable.
`--vsync` explicitly requests the display-paced mode. `comparison.json` also lists
FPS derived from each run's mean frame interval. The per-scene S1/S2 reports
include the Player's render time and the existing cadence observed from its
normal worker. Cadence measures application observations. The CPU submit
interval includes vertex expansion and recording.
S2 step time is the existing UI-weighted step metric. Atlases and vertex-buffer
writes are reported separately, as are actual Vulkan draw commands. Peak working
set is the OS high-water mark observed while the process was alive.

For each declared mean, calculate
`100 × (maximum − minimum) / arithmetic mean of the three run means`.
The limit is inclusive at 5%. p95 uses nearest rank and is reported separately.
A measurement error, missing mean, incompatible fixture or adapter, change of
sources or binary, or diagnostic run prevents a pass. The S1/S2 60 FPS targets
and S1's 1 GiB working-set target are recorded independently. GPU pass and
total timings, reference-image difference, and meshlet counts remain null with
reasons when unavailable. Aggregate Vulkan wall time is not a GPU timestamp.

Each invocation creates a unique `outputs/bench/<local date>/<series>/` with
`manifest.json`, per-scene JSON and logs, three `run.json` files,
`comparison.json`, and NVIDIA GPU telemetry when `nvidia-smi` is available.
The manifest declares the conditions before measurement. `source.json`
binds tracked and untracked source files, including fixture assets, to HEAD.
The binary hash and each run's input inventory are retained. An unsuccessful
series exits nonzero and its reports remain available. If conditions change,
start a new complete series. R0.3–R0.6 have separate acceptance criteria.
