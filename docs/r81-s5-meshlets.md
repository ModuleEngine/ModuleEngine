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
