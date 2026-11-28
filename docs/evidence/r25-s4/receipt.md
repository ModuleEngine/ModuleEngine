# R2.5 S4 verification

[English](receipt.md) | [Русский](../../ru/evidence/r25-s4/receipt.md) | [Reference](README.md)

The verification queue tested `71a040f1594581f427f668f411bc0f1edcdac9f0`
with source digest `82c3ade2b20e9188e04b320de8b664589012fe4410859d996221d55f394abfb8`.
The snapshot matched before and after. The earlier [reference](README.md)
binds the matching S4 conditions and published PNGs to main at `9aa0e76`.

| Graph | Reference bytes | Candidate bytes | Reduction | Raw candidate ranges |
| --- | ---: | ---: | ---: | --- |
| Forward | 203,489,280 | 115,015,680 | 43.48% | [Async](async-forward-memory.json), [graphics](graphics-forward-memory.json) |
| Deferred | 238,878,720 | 115,015,680 | 51.85% | [Async](async-deferred-memory.json), [graphics](graphics-deferred-memory.json) |

Each of the four Release offscreen runs replayed the same 32 camera poses at
1920×1080 on the RTX 4080 SUPER with driver 617.42 and indexed materials.
The slot, image names, formats and dimensions match the reference. The
union counts hardware depth and encoded output. Both queue configurations
passed the 30% gate independently. `image_quality_check --compare` reported
zero changed RGBA8 pixels and maximum channel delta zero for Forward 000
and Deferred 031. The two queues also produced matching PNG SHA-256 values
for all 32 frames of each graph.

Windows 26H2 build 26300.10075, Rust 1.99.0 MSVC x64, DXC 1.9.2609.5.
Khronos 1.4.328 reported synchronization validation active in all four
runs. The raw logs contain zero Khronos warnings or errors, VUIDs or
synchronization hazards. The Vulkan loader recorded the explicit layer
configuration and system implicit layers. The author separately passed
44 Vulkan/DXC tests on each queue configuration, including the retained
window image, resize and cancellation regression. The S4 memory figures
cover a completed offscreen slot.

The verifier bound the executables through Cargo JSON and SHA-256 before
using them and verified the preserved copies afterwards.

| Artifact | SHA-256 |
| --- | --- |
| Release Editor with `bench` | `66269666b23d375ce8b8514e32458dc8f4a3c6a4197c942e8f751341e64af06f` |
| Release `image_quality_check` | `26dbb21a3e6cb3beabfeeed72ea2cd968abb856b977844c79e72326fb9226f25` |
| Full verifier receipt `verification.json` | `fabacc3f8b36b167b66158ec3cf9c75a6133c990c81f2bf43e63d7d64fcb8c10` |

The local verification delivery preserves the complete commands, raw stderr,
capture manifests and PNGs under `outputs/verify-queue/r25-71a040f1/`.
Use the [reproduction commands](../../bench.md#r25-checks).
