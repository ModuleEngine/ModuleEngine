# R0.4 — Vulkan hardware tiers

[English](hardware-tiers.md) | [Русский](ru/hardware-tiers.md) | [Documentation](README.md)

**21 November 2026.** The desktop **GeForce GTX 1660 SUPER (6 GB)** is the
candidate for minimum Windows Player hardware at tier A. R0.4 verified one
configuration, the Windows reference system with a **GeForce RTX 4080 SUPER**.
The tier requirements describe the [rebuild](product-roadmap.md#rebuild-r0).
Device selection in the current code comes from
`b50e76d20006598cae423bc8e8d3a78f8007d147`.

## Device requirements and fallbacks

| Tier | Required device capabilities | Path and fallback | RTX 4080 SUPER on Windows |
| --- | --- | --- | --- |
| A — base raster | Vulkan ≥1.3 with a graphics + compute queue and `dynamicRendering`, `synchronization2`, `bufferDeviceAddress`, `descriptorIndexing`, `shaderSampledImageArrayNonUniformIndexing`, `runtimeDescriptorArray`. These use core Vulkan. The current Player also needs the platform interop described below. | The current path uses fixed descriptor bindings per pass, a texture atlas, conventional draws and barriers. R2 builds the GPU-driven raster path. On devices missing the new A capabilities the existing path remains available during R2 parity where its own requirements pass. The final Vulkan baseline has no promised lower tier. | All listed bits reported. The existing raster path and DX12 bridge ran S1/S2. R2's path is pending. |
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
1.3, checks the device API and graphics + compute queue, and queries and
enables `dynamicRendering`. Renderer-only mode prefers a qualifying discrete
GPU and enables no device extensions. With a native presentation target, the
producer must match the GUI adapter. Among the tier bits above, the producer
enables only `dynamicRendering`. There is no A/B/C selection in this version.

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
