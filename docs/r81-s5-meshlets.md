# R8.1 S5 mesh shader experiment

[English](r81-s5-meshlets.md) | [Русский](ru/r81-s5-meshlets.md) | [Documentation](README.md)

R8.1 compares two explicit source Editor benchmark commands. Both use the
same S5 fixture with 40 patches, 20,000,000 indexed source triangles,
10,040,040 vertices, the saved fixture camera and the stock Forward graph at
1920×1080. The production renderer retains its existing device selection,
shader ABI and scene submission.

- `S5-r24` builds the existing retained scene and uses R2.4 Hi-Z plus
  indexed indirect count. It requires that route to be active and records the
  compaction counts. Its Vulkan logical device leaves mesh shaders disabled.
- `S5-r81` builds meshlets in memory with `meshopt` 0.6.2, using at most
  64 vertices and 124 triangles per meshlet. One task group schedules up to
  32 mesh groups. Each mesh group transforms its indexed vertices, produces
  flat face normals and writes the original triangle and object IDs. The
  static task shader emits every meshlet. These counts come from the
  dispatch workload. The command requires the mesh and task features of
  `VK_EXT_mesh_shader` and the indexed Forward materials route.

The prototype build cost includes clustering, restoration of the oriented
source triangles and packing for upload. Both paths upload the scene once
and draw from device-local buffers. The experiment does not author meshlet
assets or measure per-meshlet visibility, import caching, LOD or the 60 FPS
goal of R8.3.

## Reproduction

Use the isolated reference Windows GPU and the project-pinned DXC. Select
the executable reported by the bench build, and keep the path to that exact
artifact in the result.

```powershell
cargo build --locked --release -p module-engine-editor --features bench
python -B scripts/bench_r81.py --exe <bench-exe> --output outputs/r81/series
```

The comparison first reads back both 1080p images. It checks the device,
dimensions, all RenderStats counts, nonzero geometry coverage and object
ownership at every pixel. A changed Forward triangle ID requires two
triangles of that object with one shared source edge and positive depths
within one representable f32 step. The pixel center must be within one
device subpixel step of the projected edge. Both raw normals and untextured
Forward colors must agree with their respective source faces within 1e-5.
The check accepts only unambiguous IDs and up to two changed edge pixels
at 1080p, with at most 4 absolute RGBA8 units in those pixels. The
absolute output delta at all other pixels is at most 2 per channel. The
report records the overall maximum, the number of shared-edge changes and the depth,
normal, color and source IDs at the worst differing pixel. It retains
both PNGs. `R81-visibility-no-fxaa` bypasses FXAA in both paths for
diagnosis. The full Forward graph is required for the scored comparison,
which runs five pairs of native
processes, switching which path starts each pair. Each process warms up
for five frames and records 30 GPU-only native frames with GPU timestamps,
vsync off and Immediate presentation. Both paths must report the same
presentation, scene and device signature and use the actual intended
submission path on every frame. The comparison records median run means
and p95 values, the noise tolerance from the R2.4 runs, peak process
working set, producer allocator bytes, one-time CPU preparation and upload
and GPU draw counts. Producer allocator bytes include all pools accounted
for by that allocator. They do not represent a peak measurement of total
VRAM usage.

Run the same image check separately in a debug build with the Khronos
validation layer and synchronization validation active, using
`--bench-scene R81-visibility <new-output>`. Keep the compiler and process
logs and source and artifact hashes. See [R2.4 checks](bench.md#r24-culling-checks),
[Vulkan mesh shader features and limits](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_mesh_shader.html),
and [DXC's SPIR-V attributes](https://github.com/microsoft/DirectXShaderCompiler/blob/main/docs/SPIR-V.rst).

<a id="r81-result"></a>

## Measured result — 25 November 2026

On the fixed S5 view, the task/mesh path reduced mean producer GPU graph
time by **45.94%** and p95 by **42.56%**. The values below are
medians of per-run statistics across five pairs, with 150 scored frames
per path. The renderer used stock Forward and the native WGPU/DX12
bridge at 1920×1080 with Immediate presentation, vsync off and GPU
timestamps on every scored frame. The timed runs read back zero pixel bytes.

| Metric | R2.4 Hi-Z + indexed indirect count | R8.1 task/mesh |
| --- | ---: | ---: |
| GPU graph mean / p95, ms | 4.552 / 4.977 | 2.461 / 2.858 |
| GPU geometry pass mean / p95, ms | 2.560 / 3.012 | 2.067 / 2.452 |
| Observed window interval mean / p95, ms | 6.001 / 6.528 | 3.590 / 4.201 |
| CPU submission mean / p95, ms | 0.324 / 0.401 | 0.211 / 0.289 |
| Peak process working set, MiB | 2,391.8 | 1,560.4 |
| Producer allocator used / reserved, MiB | 725.2 / 896.0 | 593.0 / 832.0 |
| Initial geometry upload, MiB | 426.1 | 398.9 |
| First submission start to observed completion, ms | 1,237.0 | 259.7 |
| Meshlet clustering and packing, ms | — | 15,926.2 |
| Meshlet allocation and staging, ms | — | 36.4 |

The first completion interval starts after scene and device setup and
excludes clustering and packing. CPU submission p95 was within the
measured noise tolerance. Two mesh-path window intervals exceeded
16.67 ms, at 20.920 and 20.726 ms. All 150 R2.4 intervals were below
16.67 ms, with a maximum of 7.173 ms.

The R2.4 depth seed drew all 40 patches and Hi-Z retained all 40. Its
culling scope averaged 1.824 ms. That cost contributes to the whole-graph
improvement. The Forward geometry pass alone improved by 19.27% on its
mean. The prototype dispatched 6,510 task groups for 208,305 meshlets
and all 20 million source triangles. Per-meshlet culling was disabled.

The matching physical adapter was an RTX 4080 SUPER (`10de:2702`,
16,376 MiB), driver 617.42, Vulkan 1.4.351 on Windows 11 build 26300.
It reported `VK_EXT_mesh_shader` revision 1, both task/mesh feature bits
and mesh shader query support. Task and mesh workgroups supported
128 invocations each and preferred 32. The dispatch X and total-count
limits were 4,194,304 for both stages. Mesh output limits were
256 vertices, 256 primitives and 32,768 bytes. The task payload
limit was 16,384 bytes. The storage buffer range limit was
4,294,967,295 bytes, and graphics timestamps had 64 valid bits with a
1 ns period. The logical R2.4 device left the mesh extension disabled.

The Release image check matched every object ID and all RenderStats
counts. RGBA8 changed in seven pixels. The maximum delta was 3 at one
verified shared source edge and 1 everywhere else. The earlier strict
whole-image limit of 2 failed on this edge. The Debug PNGs matched the
corresponding Release PNGs byte for byte. Both Debug devices reported
active Khronos synchronization validation and zero validation warnings
or errors at teardown. The two bounded CPU tests and strict targeted
Clippy passed on Windows.

The measured source was `e3c742c638412cce9780acae2bd095fe6e9d516c`
with source digest
`b4621afd2ee4da956359eca730c54d8d207761d52e1ca4d416ba8cbc4f3ec972`.
Release and Debug Editor SHA-256 were
`5a38bfcf1a04b88baaa1d0bec3cba5775b4a84e5744c6099e7c5ee8a71d98291`
and `c836e56d9ce35ec92ce4ffd3d2db7b91d2c5d9d46c90fbcc695f3dcc5f7af73b`.
The queue retained both executables, the original logs and samples, and the
failed comparisons with the previous gate. [Implementation and review](https://github.com/myjname/ModuleEngine/pull/56)

The result supports continued Tier B experiments using precomputed geometry.
The measured build cost is too high for ordinary scene loading. Keep the
existing production renderer and the Tier A path. This result covers one
camera on one Windows GPU. It gives no frame-time guarantee across other
views, physical Tier A devices or Linux GPUs.
