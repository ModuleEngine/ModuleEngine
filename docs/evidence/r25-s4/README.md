# S4 image and transient memory reference for R2.5

[English](README.md) | [Русский](../../ru/evidence/r25-s4/README.md) | [Documentation](../../README.md)

S4 on `main` at `9aa0e7633b2bfbf87fbd7d89bb6c6085cc04c8c5` used
**203,489,280 bytes (194.0625 MiB)** for Forward and **238,878,720 bytes
(227.8125 MiB)** for Deferred at 1920×1080. The reference uses one completed
offscreen frame slot and the stock render graph with all passes enabled.

| Graph | Images | Graph image bytes | Published reference frame |
| --- | ---: | ---: | --- |
| Forward, SSAO, ACES tonemap, FXAA, color encode | 8 | 203,489,280 | [Camera 0, PNG](forward-000.png) |
| G-buffer, Deferred lighting, SSAO, ACES tonemap, FXAA, color encode | 9 | 238,878,720 | [Camera 31, PNG](deferred-031.png) |

The two full-resolution reference frames ship with the source. For each graph,
`baseline.json` records the PNG and capture SHA-256, camera pose, resolution,
and bound range report hash. The complete 32-frame runs and raw manifests
are preserved in the local `outputs/rv-c/` delivery directory. [Manifest](baseline.json)

## Source and render conditions

The workstation used Windows x64 `26300.10075`, Ryzen 9 3900X, RTX 4080
SUPER `10de:2702`, NVIDIA driver `617.42`, device Vulkan `1.4.351`, Rust
`1.99.0` and DXC `1.9.2609.5`. The Release Editor was built with `bench`.
Forward's published frame comes from the unchanged `main` build. The
benchmark-only diagnostic build adds the Deferred capture option and reads
the existing Vulkan bindings. All 32 Forward PNGs and the capture manifest
from that build matched the uninstrumented `main` capture byte for byte.

The stock glTF importer loads the pinned Sponza and two Khronos models.
Both graphs use indexed base-color images and the expanded snapshot input.
Camera `i` (0–31) uses target `[-5 - 0.025*i, 0.45, 0.0125*i]`, distance
`13`, pitch `0.10` and yaw `pi/2`. The scene inventory defines 256 point
lights. Its legacy shaders render the built-in directional lighting. The
capture uses Vulkan diagnostic RGBA readback at 1920×1080.
[Fixture](../../../crates/editor/src/bench/fixtures.rs) · [Camera and capture](../../../crates/editor/src/bench/mod.rs)

The Sponza images show the Khronos glTF conversion of the Sponza model
originally created by Marko Dabrovic and modified by Crytek. The exact input
manifest pins 71 downloads. [Input manifest](../../../examples/bench/sponza/manifest.json) · [Model credits and terms](https://github.com/KhronosGroup/glTF-Sample-Assets/blob/e124e8d8d7330a5372bc2b4d00d39a4a9d7997ea/Models/Sponza/README.md)

## Reproduce the comparison

From a source clone with the pinned native toolchain and Vulkan adapter,
restore DXC with the usual bootstrap and run from the repository root.
Use a fresh output directory for each capture. The two commands run
sequentially on the reference adapter.

```powershell
python -B -c "import sys; sys.path.insert(0, 'scripts'); from bench import ensure_bench_assets; print(ensure_bench_assets())"
cargo build --locked --release -p module-engine-editor --features bench
& target/release/module-engine-editor.exe --bench-scene S4-floor rv-c-s4-forward
& target/release/module-engine-editor.exe --bench-scene S4-floor-deferred rv-c-s4-deferred
Get-FileHash docs/evidence/r25-s4/forward-000.png, rv-c-s4-forward/frame-000.png
Get-FileHash docs/evidence/r25-s4/deferred-031.png, rv-c-s4-deferred/frame-031.png
python -B scripts/s4_graph_memory.py rv-c-s4-forward/graph-memory.json --reference docs/evidence/r25-s4/forward-memory.json
python -B scripts/s4_graph_memory.py rv-c-s4-deferred/graph-memory.json --reference docs/evidence/r25-s4/deferred-memory.json
```

On Linux the executable has no `.exe` suffix. The reference hashes in the
manifest identify the stored PNG bytes. Replay hash equality was checked
on the reference Windows device. Image comparison on a different device
or graph should use decoded pixels and the agreed R2.5 tolerance.
[Toolchain](../../../toolchain-manifest.json) · [Benchmark guide](../../bench.md)

The memory comparison unions `[offset, offset + VkMemoryRequirements::size)`
in bytes for each distinct `VkDeviceMemory` binding. It includes every image
owned by the graph in the completed slot, including hardware depth and the
temporary encoded output. It counts overlapping aliases once and excludes
allocator slack, persistent material images, geometry, readback buffers and
external presentation images. Both saved graphs have no overlapping ranges.
Subsequent measurements for the R2.5 memory gate use the same graph,
viewport, material path and slot definition. [Forward ranges](forward-memory.json) · [Deferred ranges](deferred-memory.json) · [Comparison helper](../../../scripts/s4_graph_memory.py)
