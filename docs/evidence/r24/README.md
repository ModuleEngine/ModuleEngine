# R2.4 acceptance evidence

[English](README.md) | [Русский](../../ru/evidence/r24/README.md) | [Documentation](../../README.md)

R2.4 passed its original Windows acceptance on 23 November 2026. In S3-100k,
the mean window interval was **1.764 ms** across three Hi-Z runs and
**1.733 ms** in the one culling-off control. Hi-Z was about 0.031 ms slower in
this comparison. One control run is insufficient to establish a regression.
S3-100k kept all 100,000 candidates and selected zero depth occluders. It
exercised submission at scale, while the visibility fixture exercised rejection.

| S3-100k run | Mean, ms | p95, ms | Intervals above 1000/60 ms | Long interval, ms (sample) |
| --- | ---: | ---: | ---: | ---: |
| Hi-Z 1 | 1.76938 | 2.1571 | 1/240 | 20.6348 (96) |
| Hi-Z 2 | 1.76430 | 2.2025 | 1/240 | **20.8540** (97) |
| Hi-Z 3 | 1.75716 | 2.2129 | 1/240 | 20.6695 (99) |
| R2.3 control, culling off | 1.73256 | 2.2149 | 1/240 | 20.7628 (103) |

These are application-observed intervals, with sample numbering starting at 1.
The original gate used the mean interval. The three scored runs reached
565–569 FPS by that measure. Each submitted one `vkCmdDrawIndexedIndirectCount`
for the scene raster pass and a separate one for the Hi-Z seed. The native
path used up to two producer slots and read back zero pixel bytes to the CPU.

## Visibility and image comparison

On the 4,099-drawable scene with the wall hiding the targets, **2,052 → 4**
is the number of indirect records from frustum candidates to the Hi-Z result,
in both Forward and Deferred at 641×361. In each graph one Vulkan invocation
consumed the compacted list and one populated the depth seed. Across the
8-, 257- and 4,099-drawable cases, all 36 comparisons matched the R2.3 RGBA
and render statistics exactly. They include orbit and moved-wall poses. The
separate 1920×1080 S3-100k readback also matched exactly. The saved PNG pairs
were byte-identical, with seven pairs per visibility run. No paired frame-time
measurement with culling enabled and disabled was made on the occlusion scene.

The Vulkan implementation tests against a max-depth pyramid built **each
frame** from a depth-only seed of large opaque occluders. It renders the seed
using current camera and transforms, then tests the remaining bounds. See the
[renderer contract](../../hardware-tiers.md#r24-culling) and [replay commands](../../bench.md#r24-culling-checks).

## Source, bench and review

- Original native measurement and independent review used
  `df279ba180ef085208929897e87ad2c2c9fe0ea8`. `/root/r24_review` checked the
  source digest, EXE hashes, raw distributions and rendered frames, and closed
  the review after the readback alignment fix. The reviewer accepted the original
  criteria on the reference Windows configuration. The formatting follow-up and
  main synchronization were reviewed separately, with the final checked PR head
  `33068733787412b5bc1664c22032c6f34e18fb17`. [PR #43](https://github.com/ModuleEngine/ModuleEngine/pull/43) · [Required CI](https://github.com/ModuleEngine/ModuleEngine/actions/runs/45127626276)
- Bench conditions were Windows x64 `26300.10075`, Ryzen 9 3900X, RTX 4080
  SUPER `10de:2702` (16,376 MiB), driver `617.42`, device Vulkan `1.4.351`,
  Rust `1.99.0` (`x86_64-pc-windows-msvc`) and Release Editor with `bench`.
  At 1920×1080 the WGPU/DX12 window used the Vulkan interop bridge and
  `Immediate` present mode, with vsync off. The display was 3440×1440 at
  144 Hz and 100% scale. Each run had 30 warmup and 240 scored frames, with
  validation and profiling disabled. Active Khronos synchronization validation
  ran separately and reported zero errors. [Bench fixture and modes](../../bench.md#r24-culling-checks)
- The pre-R2.5 Windows review assessed `main` at
  `c93f919695b98e93162b045721f0137227b49961`. It independently recomputed the
  stored native series and repeated visibility on the same GPU with the
  preserved validation EXE, from the `3306873` test checkout. The latter
  differed from the reviewed `main` only in the two roadmap files. It did
  not resample the window series. The repeated 36 comparisons and seven
  pairs agreed, with zero Vulkan validation warnings and errors.
  [Reviewed main](https://github.com/ModuleEngine/ModuleEngine/tree/c93f919695b98e93162b045721f0137227b49961)
- The combined review of 24 November reconciled that Windows result with the
  Linux source review at the same `main` snapshot. It upheld R2.4 on the original
  criteria and identified evidence availability, explicit timing criteria and
  measurement before further architectural work. The Linux review supplied no
  hardware GPU measurement. The record does not cover another GPU, physical
  scanout, near/far-plane contact, shear, reflected transforms or removal of the
  occluder. [RV-C scope](../../product-roadmap.md#rebuild-rv-c)

[Metrics and SHA-256 manifest](metrics.json) gives the retained report, EXE,
PNG and review hashes. The scored EXE SHA-256 is
`0eb0205106515d1d4e85ea928241a599afd5aa1cc3084191f9416271abeaf19c`.
The original reports, executables and complete capture series are preserved
in the R2 worktree's `outputs/r24/` and `outputs/reviews/r24-gpu-culling/`, and
in the main checkout's `outputs/reviews/project-c93f919-20261123/`.
