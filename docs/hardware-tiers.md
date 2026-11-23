# R0.4 — Vulkan hardware tiers

[English](hardware-tiers.md) | [Русский](ru/hardware-tiers.md) | [Documentation](README.md)

**21 November 2026.** The desktop **GeForce GTX 1660 SUPER (6 GB)** is the
candidate for minimum Windows Player hardware at tier A. R0.4 verified one
configuration, the Windows reference system with a **GeForce RTX 4080 SUPER**.
The tier requirements describe the [rebuild](product-roadmap.md#rebuild-r0).
The R0.4 snapshot came from
`b50e76d20006598cae423bc8e8d3a78f8007d147`.

## Device requirements and fallbacks

| Tier | Required device capabilities | Path and fallback | RTX 4080 SUPER on Windows |
| --- | --- | --- | --- |
| A — base raster | Vulkan ≥1.3 with a graphics + compute queue and `dynamicRendering`, `synchronization2`, `bufferDeviceAddress`, `descriptorIndexing`, `shaderSampledImageArrayNonUniformIndexing`, `runtimeDescriptorArray`. These use core Vulkan. The current Player also needs the platform interop described below. | R2.1 adds [two frame slots](#r21-device). R2.2 adds [indexed material images](#r22-materials) on qualifying devices. The compatibility path retains the atlas. During R2 parity it remains available wherever its own requirements pass. The final Vulkan baseline has no promised lower tier. | All listed bits reported. The Windows DX12 bridge and R2.2 indexed material path were exercised on this adapter. |
| B — mesh shaders | A plus `VK_EXT_mesh_shader` with `meshShader` and `taskShader` for R8's task/mesh pipeline. | R8 falls back to compute culling and indexed indirect drawing on A using the same meshlet data. | Extension and both bits reported. R8 and its fallback are pending. |
| C — hybrid rays | A plus `VK_KHR_acceleration_structure` (`accelerationStructure`), its dependency `VK_KHR_deferred_host_operations`, and `VK_KHR_ray_query` (`rayQuery`). A separate ray-tracing pipeline also needs `VK_KHR_ray_tracing_pipeline` (`rayTracingPipeline`). | R9 falls back to raster shadow maps, SSAO, screen-space reflections and IBL as applicable. B and C are independent upgrades over A. | All listed extensions and bits reported. R9 and its GPU timings are pending. |

Query and enable the individual features used by each path. The
`descriptorIndexing` bit alone does not enable its subfeatures. The reported
Vulkan API version does not establish the optional indexing bits or the B/C
extensions. Check the binding mode and descriptor limits in R2.2.
For example, sparse bindings use
`descriptorBindingPartiallyBound`, variable-sized bindings use
`descriptorBindingVariableDescriptorCount`, and nonuniform storage-buffer
arrays use `shaderStorageBufferArrayNonUniformIndexing`. Update-after-bind
uses the bit for the descriptor type, such as
`descriptorBindingSampledImageUpdateAfterBind`. Also check
`timelineSemaphore` for R2.1 and `drawIndirectCount`, `multiDrawIndirect`
and `maxDrawIndirectCount` for R2.4's batches. Those checks belong to the
features that consume them.
See Khronos [Vulkan 1.2 features](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceVulkan12Features.html),
[Vulkan 1.3](https://docs.vulkan.org/refpages/latest/refpages/source/VK_VERSION_1_3.html),
[mesh shaders](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_mesh_shader.html),
[acceleration structures](https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_acceleration_structure.html),
[ray queries](https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_ray_query.html)
and [ray pipelines](https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_ray_tracing_pipeline.html).

## What the current Player uses

The [Vulkan producer](../crates/render-vulkan/src/vulkan.rs) requests Vulkan
1.3 and a graphics + compute queue with dynamic rendering. It prefers a
qualifying discrete adapter without a presentation target and matches the GUI
adapter with a target. [Device selection and enabled features](#r21-device)
are separate from the native presentation import contract.

- **Windows.** WGPU uses DX12 for the window. The Vulkan device must match the
  D3D12 adapter by valid LUID, expose and enable
  `VK_KHR_external_memory_win32`, and import the shared D3D12
  `D3D12_RESOURCE` image. The bridge checks import support for optimal-tiling
  `R8G8B8A8_UNORM` with sampled and transfer source/destination usage.
- **Linux.** WGPU uses Vulkan for the window. The two devices must match by
  device and driver UUID, enable `VK_KHR_external_memory_fd`, and support
  dedicated `OPAQUE_FD` export on the GUI device and import on the producer
  for the shared RGBA8 image.

Actual shared-image allocation and presentation must succeed. Interop failures
report an error with no automatic CPU fallback. The explicit software and
diagnostic routes have their own contracts. Vulkan-only game presentation is
planned for R2.6. See the [import checks](../crates/render-vulkan/src/presentation.rs),
[GUI bridge](../crates/player/src/gpu_presentation.rs) and [Player contract](player.md).


<a id="r21-device"></a>

## R2.1 device layer and verification

The producer queries individual A, B and C features on the selected adapter.
B and C are independent. The renderer enables the bits it uses. Tier A
with `timelineSemaphore` activates two command and descriptor slots,
synchronization2 and the gpu-allocator path. Other adapters that pass the
existing Vulkan 1.3 and dynamic rendering checks use one slot with a fence
and standalone resource allocations. Descriptor indexing, device addresses,
mesh shaders and rays are reported capabilities at this stage.

The allocator suballocates producer buffers and images and honors mandatory
dedicated allocations. Imported external memory stays dedicated. Vertex writes
and explicit diagnostic readback use coherent mapped memory. New atlases use a
mapped staging ring. The last graphics completion value controls reuse of each
range and the lifetime of replaced staging buffers. The producer sends whole
atlas uploads to a dedicated transfer family when exposed, releases the image
there, and acquires it on the graphics queue after a GPU-side timeline
dependency. Otherwise the graphics command buffer records the upload. Slots
hold GPU resources and pipeline references up to confirmed completion. A shared
deletion worker polls detached busy slots. The normal native producer returns
`Busy` when the slots or leased output images are full. Explicit diagnostic
readback can wait for its own submission.

Run on an idle Windows desktop with DXC and the Khronos validation layer
installed or extracted locally. Point `VK_LAYER_PATH` at the directory
containing the layer manifest. Keep the benchmark window unobstructed and run
the scenes sequentially with no competing builds. The Cargo override enables
Vulkan diagnostics in the optimized Editor's Vulkan crate.


```powershell
$env:VK_LAYER_PATH = (Resolve-Path 'work/verification/r21/layer').Path
$env:VK_LOADER_DEBUG = 'layer'
$env:VK_LAYER_ENABLES = 'VK_VALIDATION_FEATURE_ENABLE_SYNCHRONIZATION_VALIDATION_EXT'
$env:MODULE_ENGINE_VULKAN_TRACE = '1'
$env:MODULE_ENGINE_BENCHMARK_ARM = 'automatic'
$env:MODULE_ENGINE_BENCHMARK_FRAMES = '240'
$env:MODULE_ENGINE_BENCHMARK_PROFILING = 'off'
$env:MODULE_ENGINE_GUI_VSYNC = 'off'
cargo --config 'profile.release.package.module-engine-vulkan.debug-assertions=true' build --locked --release --features bench -p module-engine-editor
if ($LASTEXITCODE) { throw 'Editor build failed' }
$out = New-Item -ItemType Directory "outputs/r21-repeat-$(Get-Date -Format yyyyMMdd-HHmmss)"
$exe = (Resolve-Path 'target/release/module-engine-editor.exe').Path
foreach ($scene in 'S1','S2','S3') {
    & $exe --bench-scene $scene (Join-Path $out.FullName $scene) *> (Join-Path $out.FullName "$scene.log")
    if ($LASTEXITCODE) { throw "$scene failed" }
}
git rev-parse HEAD
Get-FileHash $exe
```

The loader log should show insertion of `VK_LAYER_KHRONOS_validation`.
Every producer summary should show `validation=active` and `errors=0`
through teardown. Check 240 measured frames and GPU-only native presentation
in each scene's JSON report. In the log, `Vulkan frame begin`, `submit`,
`complete`, and `wait` identify the renderer and queue serial. On the
timeline path the serial is the semaphore value. `previous` records the
old use of a slot, and `completed` is the graphics timeline counter when
CPU recording begins. A previous serial must have completed before a new use.
S3 should reach two outstanding tickets on the timeline path. Normal S1–S3
should show no `Vulkan frame wait`. The explicit diagnostic route logs its
synchronous wait as `reason=diagnostic`. The `Vulkan transfer submit` event
records the dedicated upload signal when that family is available.
Local evidence, including the [S3 prototype measurements](decisions.md#r21-device-prototype),
is under `outputs/r21/`. The published hardware qualification limits below
still apply.

<a id="r22-materials"></a>

## R2.2 indexed materials

The indexed path requires the R2.1 timeline and allocator path,
`shaderSampledImageArrayNonUniformIndexing`, `runtimeDescriptorArray` and
`samplerAnisotropy`. The renderer checks the regular sampled-image limits
per fragment stage and descriptor set for 1,024 descriptors, the aggregate
stage limit, room for a sampler and a storage buffer, and two bound sets.
`R8G8B8A8_SRGB` must support optimal-tiled sampling, linear filtering,
transfer and blits. The sampler uses repeat addressing, linear filtering
between texels and mip levels, and up to 16× anisotropy within device limits.

A material descriptor set contains 1,023 usable image slots and a white
fallback. The graphics queue uploads the original level and generates the
remaining mip levels by linear blits in sRGB images. Each image uses its own
device extent limit. A replacement descriptor set shares unchanged images.
On the snapshot path each set retains its own object-to-texture index buffer.
Submitted frames keep their set, images and staging buffer until GPU completion. Descriptor sets
are populated before submission and stay immutable, so this path needs no
update-after-bind, partially-bound or variable-count binding features.

The atlas path applies on devices that fail the indexed checks, for Custom
passes using the original material ABI, and when a scene exceeds the indexed
table capacity. An atlas must fit within 8192 pixels on each axis. See the
[pass ABI](gpu-hlsl.md#hlsl-abi) and
[reproduction](bench.md#r22-material-checks).

<a id="r23-scene"></a>

## R2.3 GPU-resident scene

The resident scene requires the indexed material path, `multiDrawIndirect`,
`drawIndirectFirstInstance`, and room for four vertex-stage storage buffers
and five storage descriptors across the pipeline layout. An eligible device
reports `resident_scene: true` in the Vulkan capabilities. A retained scene
uses the resident path when its active scene raster passes are stock Forward,
G-buffer or Depth/Normal, its data fit within the storage and material
limits, and its transforms can be represented safely. Direct snapshots, Custom scene passes, and compatibility hardware
use the existing vertex stream. Custom fullscreen and compute passes can run
after resident geometry.

Vulkan pools indexed mesh payloads for the lifetime of the device. The
vertex stage reads local positions, UVs and triangle indices, applies the
instance transform and computes the world-space face normal. A persistent
instance buffer holds the transforms and material row indices. Persistent
material rows hold the effective color and the R2.2 image index. The renderer
keeps texture slots for unchanged images when it builds a new descriptor set.
The mesh pool grows by copying its populated prefix on the GPU and recording
only new mesh payloads from the CPU. Each stable object ID and primitive
ordinal keeps its GPU slot while that drawable is present. Removing one
drawable omits its slot from the ordered draw runs. Insertion can reuse a
vacant slot, and changing a mesh updates the rows of that object. Unchanged
buffer regions are copied on the GPU when an allocation grows.

Draw runs preserve the published order and IDs. A run covers consecutive
physical slots that share the same differences between physical and
published object and triangle IDs. Three stock-pass constants restore the
published IDs and map material reads back to physical slots. A structural
edit still traverses the ordered scene on the CPU to assemble runs. Churn
can fragment runs and increase the number of indirect commands on the R2.3 path. The
[structural benchmark](bench.md#r23-scene-checks) measures those boundaries.

Both frame slots use the same resident buffers. Queue barriers order
previous vertex, fragment and indirect reads before transfer writes and
make those writes visible to subsequent draws. A frame retains the buffers
and descriptor generation it references through graphics completion.
Cancellation suppresses publication and the submitted scene edits remain
valid for the next frame. See the [S3 reproduction](bench.md#r23-scene-checks).

<a id="r24-culling"></a>

## R2.4 GPU culling

Vulkan reports `gpu_frustum_culling` when the R2.3 resident path can
also enable `drawIndirectCount` and the device supports six storage buffer
descriptors in the compute stage and descriptor set. The scene must fit the
device's `maxDrawIndirectCount`, a 65,535 × 128 invocation dispatch and the
storage buffer limit. A larger scene uses R2.3 draw runs. Direct snapshots,
Custom scene raster passes, and devices outside the resident requirements
use the existing expanded stream and material compatibility paths.

The compute stage reads pooled mesh bounds, accepted instance transforms and
draw runs. It rejects bounds outside the current camera, then compacts the
remaining commands in published order. Every stock scene raster pass issues
one `vkCmdDrawIndexedIndirectCount`. The color pass uses an ascending index
stream shared by all meshes to address triangle corners in the existing mesh
pool. This preserves flat face normals, published object and triangle IDs,
and the first surface at equal depth. Custom pass bindings stay compatible.

`hiz_occlusion_culling` also requires a sampled D32 depth attachment.
The first phase renders a depth-only seed of opaque candidates whose
padded screen bounds are at least 32 pixels wide and tall. The second phase
compares the remaining candidates with a max-depth pyramid of this seed.
Uncovered samples contribute far depth, odd image edges are included, and
bounds intersecting the near plane stay candidates. Occlusion requires a
strict depth separation across the entire projected bound. The pyramid uses
the current camera and transforms on every frame, so a moved or removed
occluder cannot leave stale depth behind. When the sampled image or the
pyramid exceeds the frame's limits, the renderer uses frustum culling alone.

The graphics queue orders both phases with the resident uploads and graph
draws. Scratch commands and depth belong to each completed producer slot,
and pending or retired flights keep their pipeline generation alive.
The count and commands stay on the GPU. The optional diagnostic reads three
32-bit counters after completion without waiting on the render thread.
Source-free Player exports include the internal SPIR-V variants.
See the [reference and native S3 checks](bench.md#r24-culling-checks)
and Khronos [indexed indirect count requirements](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdDrawIndexedIndirectCount.html).

<a id="reference-gpu"></a>

## Reference GPU and measured scope

The capture from the available Windows x64 host (OS build `10.0.26300.10075`)
shows a single Vulkan physical device, RTX 4080 SUPER (`10de:2702`,
16,376 MiB reported by `nvidia-smi`), NVIDIA driver **617.42**, device Vulkan
**1.4.351**, loader instance **1.4.341**. The application requests 1.3.
The [full vulkaninfo dump](evidence/r04-vulkaninfo-windows-full.txt) contains
the original stdout. The [excerpt](evidence/r04-vulkaninfo-windows.txt) records
the relevant rows and the full dump's SHA-256. Stderr and the OS and driver
captures are in the local `outputs/r04-hardware/` directory.

- **R0.2.** On RTX 4080 SUPER / 617.42 / Windows build 26300, S1 and S2 met
  their frame budgets at 1920×1080. Across the three scored runs, means were
  8.396–8.407 ms for S1 and 8.656–8.727 ms for S2. S1 peaked at about
  260 MiB of process working set. S5 completed the 20-million-triangle stress
  fixture at a 2.73–2.75 s mean frame interval. See the
  [accepted R0.2 series](bench-r02-20261121.md).
- **R0.3.** The single diagnostic pass at commit `c716b19595b6` on the same
  GPU, driver and Windows build completed S1–S5. S1/S2 both reported WGPU/DX12,
  GPU-only presentation and 240 profiled frames each. Mean GPU graph times
  were 0.336 and 0.223 ms respectively. These cover the producer graph
  only. The pass did not evaluate repeatability. Raw evidence is in
  `outputs/bench/2026-11-21/185842-136055/` (`manifest.json`,
  `comparison.json`, `run-01/`).
- **Linux.** The [CI route](../.github/workflows/ci.yml) uses Mesa software
  Vulkan for offscreen contracts. Physical Linux GPU presentation remains open.

The RTX 4080 SUPER is the R0.4 reference for the later R2.4 (scaled S3),
R5.2 (S4 lighting) and R8.3 (S5 mesh shaders) goals. These goals still need
their respective render paths and measurements.

<a id="minimum-gpu"></a>

## Minimum GPU candidate and pre-release checklist

The desktop GTX 1660 SUPER is the candidate for Windows tier A qualification.
It is a Turing card with 6 GB GDDR6 and DX12 API support, and NVIDIA lists
the model in its Vulkan 1.4 driver release notes. These are reasons to test
the current dual-API Player path on this card. The pre-release checks below
cover exact feature bits, shared-image interop and performance. The 6 GB
capacity is this card's specification. The minimum VRAM requirement has yet
to be measured. B/C remain optional for base games. S5 is a stress fixture
with its own phase goals.
See the [NVIDIA card specifications](https://www.nvidia.com/en-gb/geforce/graphics-cards/compare/?section=compare-specs)
and [NVIDIA Vulkan driver support](https://developer.nvidia.com/vulkan-driver).

- [ ] On the physical GTX 1660 SUPER with the monitor connected to it, record
   OS build, CPU, RAM, driver, display mode and power profile. Save full
   `vulkaninfo` and `nvidia-smi` output. Check the A bits and Win32 extension,
   then verify that both GUI and producer use that adapter.
- [ ] Export the G1 and ARPG fixtures as standalone Vulkan Players. Launch both
   from their relocated package directories with fresh profiles on the target
   PC. Enter Playing, resize, and confirm normal GPU-only presentation without
   interop errors. The packages should run without the Editor or compilers.
   Retain the smoke-test result and package hashes. Use the ARPG package's
   `--benchmark` to collect a native Player report. See [Player](player.md).
- [ ] Measure only S1 and S2, in serial on an idle desktop, with the release
   `bench` Editor's `--bench-scene S1|S2 <new output folder>` entry and the
   [benchmark protocol](bench.md). Request `MODULE_ENGINE_GUI_VSYNC=off`
   and set `MODULE_ENGINE_BENCHMARK_FRAMES=240`,
   `MODULE_ENGINE_BENCHMARK_ARM=automatic` and
   `MODULE_ENGINE_BENCHMARK_PROFILING=off`. Use the default native GUI
   backend. Retain three complete runs per scene in fresh processes, with
   30 warmup and 240 measured frames per run. Record physical 1920×1080,
   Playing, 20 active
   enemies in S2, effective present mode, GPU-only route, source and binary
   hashes, and OS peak process working set for S1. Each run must meet mean
   and p95 ≤16.67 ms in S1 and ≤16.667 ms in S2, and S1 must peak at ≤1 GiB.
   Retain S2 tick timings too. Repeat on the rebuilt A path before a release
   that relies on it.

Public minimum-hardware support remains open until those checks pass on the
recorded configuration. Other GPU/driver pairs and hardware Linux need their
own runs.
