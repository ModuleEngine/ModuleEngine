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
loading. The S4 fixture preserves the opaque leaves, chains and plant used for
the original baseline. It draws them from a derived `Sponza.bench.gltf`
beside the download. S4 uses original base-color dimensions on the
[indexed path](hardware-tiers.md#r22-materials). On the compatibility path
the fixture box-filters them to at most 256 px per edge to fit the atlas.
Geometry is unchanged. `cargo test -p module-engine-editor --features bench bench::`
checks the S3–S5 fixture contracts without a GPU; the S4 test skips with a
message when Sponza has not been downloaded.

<a id="r22-material-checks"></a>

## R2.2 material checks

Build the Editor with `cargo build --release --features bench -p module-engine-editor`.
Run it with the following options, supplying a new output directory for each.
These commands use the stock Vulkan renderer with explicit PNG readback.

- `--bench-scene R2-textures <output>` renders a grid of 1,000 unique 16×16
  source images. It checks every tile center against a linear material color
  control and records image, upload and pixel counts in `materials.json`.
- `--bench-scene S4-floor <output>` captures a 32-frame floor-level camera
  path on S4. `--bench-scene S4-floor-atlas <other-output>` replays that path
  using the original 256 px atlas fixture and the original Custom HLSL
  material ABI. Both directories contain the exact camera parameters and PNGs.

The [S4 baseline for R2.5](evidence/r25-s4/README.md) records the Forward and
Deferred images and the corresponding graph image memory measured at 1080p.

The Player example `gpu_presentation_check` exercises native interop and
checks that two in-flight material versions produce their individual reference
pixels while the second submission reuses the unchanged texture.

<a id="r25-checks"></a>

## R2.5 S4 comparison

Run both stock graphs using the [published 1080p capture conditions](evidence/r25-s4/README.md).
The decoded RGBA8 comparison uses the recorded Forward frame 000 and Deferred
frame 031. `image_quality_check --compare` writes the changed pixel count and
maximum channel delta to a new JSON report. The comparison on the reference
adapter requires exact pixels.

```text
cargo run --locked --release -p module-engine-vulkan --example image_quality_check -- --compare docs/evidence/r25-s4/forward-000.png <forward-capture>/frame-000.png <new-forward-report.json>
cargo run --locked --release -p module-engine-vulkan --example image_quality_check -- --compare docs/evidence/r25-s4/deferred-031.png <deferred-capture>/frame-031.png <new-deferred-report.json>
python -B scripts/s4_graph_memory.py <forward-capture>/graph-memory.json --reference docs/evidence/r25-s4/forward-memory.json --minimum-reduction 30
python -B scripts/s4_graph_memory.py <deferred-capture>/graph-memory.json --reference docs/evidence/r25-s4/deferred-memory.json --minimum-reduction 30
```

The memory check requires the same named images and formats. It unions
their bound ranges within the completed slot, including hardware depth and
the encoded output. Keep the capture manifests, raw ranges, image reports,
validation logs and executable hashes. `asynchronous_compute` in each
candidate range report records the route actually used. Repeat with
`MODULE_ENGINE_GRAPH_COMPUTE=graphics` for the single-queue control.
The [R2.5 receipt](evidence/r25-s4/receipt.md) records the verified results.

<a id="r23-scene-checks"></a>

## R2.3 scene uploads and timing

Use a Release Editor built with `bench`. Run native S3 with 11,000 drawables
and 1,000 moving instances, then repeat in a new output directory with
`MODULE_ENGINE_BENCHMARK_S3_STATIC=1` to hold every model fixed. Use the same
viewport, profiling and vsync configuration as the baseline and keep other
GPU work out of the measurement. Native runs should admit two tickets and
transfer no CPU pixel bytes. An optimized build with the Vulkan package's
debug assertions and the Khronos layer verifies synchronization separately
from performance runs.

`first_completed_frame_work` and every sample record bytes for vertices,
mesh indices, instances, material rows, draw commands, texture images and
the atlas. The moving series should write only changed instance rows in
steady state. The fixed series should write zero scene bytes after the first
submission. The counters exclude on-device copies for mesh pool growth.

`cpu_scene_frame_ms` measures construction and application of the standalone
scene delta plus the CPU elapsed time for the successful `render_scene_presented`
call, through queue submission. The per-submission work reports renderer
preparation, command recording, queue submit, and any blocking diagnostic
GPU wait. The native producer polls asynchronously. Its observed ticket
latency and the window-observed interval are separate measurements.
`cpu_fixture_frame_ms` also includes the Core ECS fixture tick and
standalone transform extraction. The roadmap's **S3 CPU frame** uses this
full measured boundary. The S3 report also breaks down the same ECS tick
into its eight-column query and motion writes. The render-call boundary is the one
reported in the baseline `cpu_submit_ms`. The benchmark emits its own deltas,
so the [GameHost acceptance fixture](shared-game-host.md#r16-scene-updates)
separately checks the production path and its upload counters.

Run `--bench-scene R23-structure <new output folder>` for the offscreen
structural comparison. It applies one insertion, removal or triangle-count
change at the start, middle and end of scenes with 1,000 and 11,000
drawables. Each case warms both mesh/material states, records 24 edit
samples with an unchanged control, and checks exact RGBA and render stats
against the expanded snapshot. Compare delta application, Vulkan
preparation, recording and queue submit as CPU work. Diagnostic GPU wait
is recorded separately. The benchmark does not exercise ECS changes or
the native window.

<a id="r24-culling-checks"></a>

## R2.4 culling checks

Build the Editor with `cargo build --release --features bench -p module-engine-editor`.
Use a new output folder for each run.

- `--bench-scene R24-visibility <output>` checks Forward and Deferred at
  641×361 with 8, 257 and 4,099 drawables. It compares both GPU culling modes
  with the R2.3 retained renderer using exact RGBA and render stats. The
  fixture covers an offscreen object, partial visibility at the frame edge,
  equal-depth order after insertion into a fragmented draw list, and an
  occluded target revealed by both camera orbit and an occluder edit. It
  saves paired PNGs and per-frame counters, including an offscreen
  reference comparison for one 1080p S3-100k frame.
- `--bench-scene S3-100k <output>` runs the native S3 workload at 1920×1080
  with 100,000 unique positions at the original grid spacing. The camera
  backs out by `sqrt(100000 / 11000)` to frame the larger field. The
  original 11,000 items, 1,000 moving entities, 2,000 ECS entities, 200 meshes
  and 500 distinct textures are retained.
  `--bench-scene S3 <output>` keeps the original 11,000-item workload.

S3 and S3-100k report the requested mode. Set
`MODULE_ENGINE_BENCHMARK_S3_CULLING` to `hiz` (default), `frustum`, or
`off` (R2.3 submission) to select the control. `culling_mode` in each
sample reports the actual path. `scene_draw_calls` and `occluder_draw_calls`
count Vulkan commands. `culling_stats` gives the number of frustum
candidates, seed commands and compacted scene commands. The stats field
`visible_objects` counts objects owning final pixels in a detailed readback.
`culling_input_write_bytes` counts CPU writes for the mesh bounds and the
ordered run table. The one-time corner index upload is included in
`mesh_index_write_bytes`.

Measure on the [reference GPU](hardware-tiers.md#reference-gpu) with
`MODULE_ENGINE_GUI_VSYNC=off` and record `effective_present_mode`. Native S3
reports 30 warmup frames and 240 measured frames plus the CPU and producer
timings. Evaluate the 60 FPS requirement from the window-observed mean
interval and report its p95. For GPU timings, set
`MODULE_ENGINE_BENCHMARK_PROFILING=on`. The internal `Scene culling` sample
includes the seed, pyramid and compaction. Run validation with the Khronos
layer and synchronization validation in a separate session.

The [R2.4 receipt](evidence/r24/README.md) records the historical pass on the
mean interval. S3-100k averaged 1.764 ms with Hi-Z and 1.733 ms with culling
off. Each Hi-Z run had one interval over 1000/60 ms (20.6348, 20.8540 and
20.6695 ms). On the occlusion fixture, the current-frame depth seed reduced
the indirect list from 2,052 to 4 with exact reference pixels. That fixture
has no on/off pair of frame-time measurements.

For a **new** 60 FPS gate, any interval above 1000/60 ms fails unless the
criterion states an allowance before measurement, with a maximum count or
rate and a worst-interval limit. Report the count and worst interval along
with the mean and p95. The original R2.4 mean-based verdict still applies to
the historical runs.

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
native GPU texture bridge in a real window. S3 can pipeline two producer
requests on [R2.1-capable devices](hardware-tiers.md#r21-device). S4 and S5
use one request at a time. The window thread observes at most one completion
per UI callback and refills available requests in that callback. The timing
includes window scheduling. S3 records the producer slot count, observed
maximum pending tickets and allocator totals in the raw scene report.
The launcher requests `MODULE_ENGINE_GUI_VSYNC=off` for every scene.
WGPU can fall back to display pacing when an unsynchronized mode is unavailable.
`--vsync` explicitly requests the display-paced mode. Each scene records the
request separately from `effective_present_mode`, the concrete mode selected by
wgpu when configuring the benchmark window. If the selection is unavailable
or changes during the run, the effective mode is null and the report gives a
reason. This reports swapchain configuration. It does not measure compositor
behavior, scanout, or time spent waiting for presentation. `comparison.json` also lists
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
and S1's 1 GiB working-set target are recorded independently. Reference-image
difference and meshlet counts remain null with reasons when unavailable.

Each invocation creates a unique `outputs/bench/<local date>/<series>/` with
`manifest.json`, per-scene JSON and logs, three `run.json` files,
`comparison.json`, and NVIDIA GPU telemetry when `nvidia-smi` is available.
The manifest declares the conditions before measurement. `source.json`
binds tracked and untracked source files, including fixture assets, to HEAD.
The binary hash and each run's input inventory are retained. An unsuccessful
series exits nonzero and its reports remain available. If conditions change,
start a new complete series. R0.3–R0.6 have separate acceptance criteria.
`python scripts/bench.py --once` runs one diagnostic S1–S5 pass with per-scene
warmup and returns success when all scenes complete. It does not evaluate the
three-run repeatability gate.

<a id="not-worse"></a>

## Not worse than the baseline

Every later acceptance criterion that says "not worse than R0.6" (or than
any other baseline) is decided by this rule, and `scripts/bench.py --compare`
applies it and returns the verdict. Archived numbers are not reused: drivers,
Windows and background load drift, so the baseline commit is rebuilt and
measured again in the same session as the candidate. The R0.6 baseline
commit is the source revision recorded in the
[R0.6 report](product-roadmap.md#rebuild-r0).

```sh
python scripts/bench.py --no-profiling --compare <baseline commit>
python scripts/bench.py --no-profiling --compare <baseline commit> --scenes S2,S3
```

1. **Paired session.** Both sides are built in Release from their own
   sources: the baseline in a temporary Git worktree, the candidate from the
   current checkout including uncommitted changes, or from `--candidate <commit>`.
   Each side has a separate Cargo target directory inside its series build
   directory. The launcher only accepts the Editor artifact from that side's target.
   After one unscored prewarm pass per side, full passes alternate baseline,
   candidate, baseline, … until each side has five (`--rounds`, at least 3).
   Machine drift then affects both sides equally.
2. **Median verdict.** Each metric is reduced to one value per run. The verdict
   compares the median of the candidate's runs with the median of the
   baseline's runs: `change = 100 × (candidate − baseline) / baseline`.
   Positive means slower or larger.
3. **Per-metric tolerance from measured noise.**
   `tolerance = max(floor, 2 × spread)`, where `spread = 100 × (max − min) /
   median` of the baseline's runs in this session. The floor is 5 % for
   timings and 1 % for peak working set, so memory is compared almost
   strictly. A change above `+tolerance` is **worse**, below `−tolerance` is
   **better**, otherwise **same**.
4. **One automatic rerun on the border.** If a blocking metric's change is
   within 25 % of its tolerance on either side of the line
   (`|change − tolerance| ≤ 0.25 × tolerance`), the affected scenes are
   measured once more, starting with the candidate. The verdict is then taken
   from both sessions together, with no further reruns.

| Metric | Role |
| --- | --- |
| Mean and p95 of each declared metric (see the table above: frame interval, S2 GameHost step, CPU submit, S3 ECS tick) | Blocking |
| Peak working set of each scene process | Blocking, 1 % floor |
| p99 of each declared metric | Tracked: reported, never blocks. S2 step time has no raw samples, so its p99 is reported as missing. |

A scene passes when every run on both sides completed, every run reports
its vsync request, frame profiling setting and a known effective present mode,
the scene signature (adapter, driver, present mode, profiling, resolution,
workload counts) and fixture inventory match across all runs, and no blocking
metric is worse or missing. A null present mode means the mode was unknown or
changed during that run, so the scene is incomparable even when both sides
report it. A changed fixture is not comparable: record a new baseline instead.
The series passes, and the command exits 0, only when every requested scene
passes and neither side's sources or binary changed during the series. A
rejected comparison exits 1, an error 2. A comparison of a commit against
itself (A/A) shows the machine's current noise.

**25 November 2026 — keep the current tolerance (RV-D.4).** For each blocking
metric, we recalculated the cutoff as `max(floor, 2 × 1.4826 × relative MAD)`.
Relative MAD is the median absolute deviation from the baseline median divided
by that median, expressed as a percentage. The scale factor estimates standard
deviation under normal noise. The input was six comparable Windows series from
22–23 November, including one A/A rerun with ten runs on each side. The MAD
cutoff was tighter for 38 of 58 comparisons. For the five-run series it was
tighter for 24 of 37 comparisons and changed none of their worse/pass decisions.

Two metrics crossed the cutoff across all series. One three-run S3 series moved
from `same` to `worse` for mean ECS tick (+5.28%, 11.47% range cutoff and 5.00%
MAD cutoff). The ten-run A/A series for `077371c464d2c124288c858e10950f212a76c93e`
also moved to `worse` for peak working set (+4.19%, 17.97% range cutoff and 1.00%
MAD cutoff). That A/A run had two working-set bands on the same code. This is
evidence of a false regression signal under the proposed cutoff, so the present
rule and borderline rerun stay in force. The [recorded samples and calculation](evidence/rv-d-bench.json)
identify each input report, its hash, the measured arrays and the excluded runs.

**Declared presentation change.** When the presentation path itself is the
change under test (R2.6: the R0.6 WGPU path against the direct Vulkan
swapchain), declare each field that may differ between the sides:
`--declare-change gui_graphics.backend` (also `gui_graphics.renderer`,
`gui_graphics.requested_renderer`, `gui_graphics.requested_gpu_backend`; the
option repeats). Only these fields can be declared. A declared field must be
reported (non-null) in every run on both sides and identical across all runs of one side, both values are written to
`compare.json` and `compare.md`, and every other field stays strict: a
different adapter, resolution, workload or effective present mode is still
incomparable. Without the option the comparison is fully strict.

Use the following pair for R2.6. The blocking frame interval remains
UI-observed. The candidate's `window_swapchain` report includes the measured
calls and intervals on the actual window surface.

For the R0.6 pair, extract the two exact source documents from the
[R2.6 fixture archive](evidence/r26-player/r06-game-fixtures.zip). It contains
`S1.json` and `S2.json`. Set `MODULE_ENGINE_BENCHMARK_R06_GAME_FIXTURES` to
their absolute directory before invoking the command below.
The old executable generates those documents itself. The candidate checks the
input bytes against the R0.6 digests, attaches its current profile and SDK pins
in memory, and requires the resulting bytes to match its stock scene document.
It records the input and accepted host hashes in `r06-game-fixture.json`.
Both sides report the same full input hash in `fixture.json`, and the normal
signature and timing checks still apply. A difference in the scene, schedule, or
document-owned shader or Custom source fails the replay before the window opens.

| Input | R0.6 document SHA-256 |
| --- | --- |
| S1 | `71d44139df5d3c7eb42e5b1974bec80b37d4e3b496d0e9da77135d7c98ad73c8` |
| S2 | `cd71b49d91a25c174016e47b17ce09d0f00aebba26ea527d03ce1e8ee1a77439` |

```sh
python -m zipfile -e docs/evidence/r26-player/r06-game-fixtures.zip work/r06-game-fixtures
python scripts/bench.py --no-profiling --compare 731661c47e181847450e00480e60c0645abd4753 --scenes S1,S2 --declare-change gui_graphics.backend --declare-change gui_graphics.renderer --declare-change gui_graphics.requested_renderer --declare-change gui_graphics.requested_gpu_backend
```

[Measured R2.6 result](evidence/r26-player/README.md).

The series directory `outputs/bench/<date>/<time>-compare/` contains both
build logs and binaries, `compare-manifest.json` with the commits and rule,
per-run scene JSON in `session-1/` (and `session-2/` after a rerun),
`compare.json` with every value and verdict, and `compare.md` with the table
that is also printed. A full S1–S5 comparison runs 12 passes and takes much
longer than the repeatability series; limit `--scenes` to what the criterion
names when appropriate.

<a id="editor-first-frame"></a>

### First visible Editor frame (R3.5)

Use [the paired Windows startup benchmark](../scripts/bench_editor_start.py)
with Release executables built from the candidate and the R0.6 source
`731661c47e181847450e00480e60c0645abd4753`. Use `bench` on R0.6 and
`bench,verification` on the current sources to enable isolated personal
settings. Retain the build commands, feature
sets, source revisions and binary hashes. Use each binary's own Full source
directory and the same disposable project with its scene files. Run on one
unlocked 100% DPI desktop with room for the default 1580×940 client.

```powershell
python scripts/bench_editor_start.py `
  --baseline-editor $baselineEditor --baseline-root $baselineSources `
  --candidate-editor $candidateEditor --candidate-root $candidateSources `
  --project $isolatedProject --out outputs/bench/editor-start
```

Each sample starts just before launching the ordinary Editor process. The
probe raises only its own root window without activating it and samples the
desktop until the client displays the File, Edit, Tools and Windows menu
labels. It retains the first matching toolbar image, the interval from the
last negative observation to the first positive observation, and the actual
graphics backend and adapter. The Editor's UI capture route takes a later
snapshot and closes the process normally. This later readback is outside
the timed interval. A launch without a valid readback or a clean shutdown
invalidates the series. Keep the desktop unobstructed and avoid input.

After one unscored launch per side, the probe alternates five fresh processes
per side and submits the first positive observations to the timing rule above,
including its single borderline repeat. The report verifies the common
adapter, monitor, DPI and client size, along with unchanged fixture and
binary bytes. It measures the first Editor UI on a prewarmed desktop, before
Scene or Game needs to finish loading. It provides no cold-cache or physical
scanout result. The R0.6 S1–S5 report has no startup number.

<a id="vulkan-validation"></a>

## Debug Vulkan validation (R0.5)

A debug build requests `VK_LAYER_KHRONOS_validation` when the loader exposes
it. The startup log reports layer and debug-utils availability. Debug utils
names the producer device, queue, commands, pipelines, and frame resources.
Keep stderr through process exit to capture validation messages and per-instance
counts. The S1 command exits with an error if a validation error was recorded,
including during retirement. Use this run for functional diagnostics.

```powershell
cargo build --offline --locked -p module-engine-editor --features bench
$env:VK_LAYER_PATH = 'C:\path\to\explicit-layer-manifests'
.\target\debug\module-engine-editor.exe --bench-scene S1 outputs/s1-debug
```

<a id="frame-timings"></a>

## Frame timings (R0.3)

In the Editor's **Game** area, enable **Frame timings** to see the CPU and
GPU timings for the most recently displayed frame. Timing is off by default.
The switch belongs to the editor session and does not change project data.
The panel scrolls when the pass list exceeds the available height.
[See the native Game capture on Windows](media/game-frame-timings-r03.png).

Vulkan records a timestamp pair around each graph pass and around the graph
as a whole. Pass times include that pass's resource transitions and scheduled
resource operations. A bypass row measures its copy route. Pipeline overlap
can make pass intervals overlap. Atlas upload and output transfer commands
are recorded outside the graph markers. Window presentation is outside the
producer command buffer. The renderer reads the query results after completion
of the same frame, without waiting on the queries.
An unsupported queue or unavailable result carries a reason instead of a
numeric sample. Software rendering can still report the host CPU intervals.

The CPU tick row sums the time of fixed ticks committed since the previous
admitted render request. A rejected or busy request leaves that time queued.
Frames without a new tick have no tick sample. Extraction covers
preparing the render-scene input, including interpolation and content
enrichment as applicable. Command recording starts after geometry preparation
and uploads to mapped memory. Queue submission covers the CPU call to Vulkan.
S3 supplies its own fixture tick and extraction. S4/S5 render static inputs
and have no gameplay tick or per-frame extraction sample.

The benchmark launcher enables capture for all five scenes by default.
`--no-profiling` disables the added CPU spans and GPU timestamp queries. The
manifest and every scene record the setting in `frame_profiling`. In the raw JSON,
S1/S2 associate `timings` with observed `presentation_samples`; S3–S5 store it
in `samples`. Per-scene summaries expose sample counts and timings under
`metrics.profiling`, including `gpu_graph_ms`, `gpu_pass_ms` and unavailable
reasons. The R0.2 gate still uses the metrics declared above. The
[accepted R0.2 series](bench-r02-20261121.md) predates this instrumentation.
Use `--no-profiling` for the R0.6 baseline when comparing against R0.2, and
record the profiling setting and effective present modes in the R0.6 report.
