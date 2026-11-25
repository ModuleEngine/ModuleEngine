# R3.1 Vulkan editor prototype

[English](editor-vulkan-spike.md) | [Русский](ru/editor-vulkan-spike.md) | [Documentation](README.md)

Both painters run the current Editor root UI and style in one Win32 host
using winit 0.30.13 and egui-winit 0.36.2. The isolated fixture opens the
Engine workspace with Module Blueprint and Module Library. The R2
constructor selects a graphics and compute queue that can present to
that window and enables `VK_KHR_swapchain` on the same device.
See the [R2 device contract](hardware-tiers.md#r21-device).

| Painter | Draw and texture path |
| --- | --- |
| [egui-ash-renderer 0.13.0](https://docs.rs/crate/egui-ash-renderer/0.13.0) | Receives the caller's Ash device and command buffer and records dynamic rendering draws. The application schedules the library's managed texture uploads and releases. User textures can be supplied as compatible descriptor sets. |
| Thin painter | Streams egui vertices and indices, applies managed texture patches, and records clipped draws. The fragment shader multiplies premultiplied sRGBA values for a UNORM framebuffer. |

The GUI keeps one outstanding frame in both arms. A fence protects
buffer and texture reuse, and each swapchain image owns its present
wait semaphore. Both painters use direct allocations and synchronous
texture uploads. The fixed-window workload covers editor UI drawing.
Production shell migration and Scene and Game image sampling belong to
[R3.2 and R3.3](product-roadmap.md#rebuild-r3).

## Reproduce

Run from the matching Full checkout on the reference Windows desktop.
Use the pinned Rust toolchain, the Cargo registry with the locked
dependencies, the pinned DXC and a target directory owned by the check.
The thin painter resolves DXC through the Full package root. An installed
DXC can also be supplied with the existing `MODULE_ENGINE_DXC` setting.

Set the native monitor scaling before each series. Keep the
1580×940 physical client visible without input or competing builds.
The executable checks the winit scale and egui's pixels per point
at unit egui zoom. Both series use `IMMEDIATE` presentation. Set
`$releaseExe` to the saved Release executable path before the two series.

```powershell
cargo build --offline --locked --release --target x86_64-pc-windows-msvc -p module-engine-editor --features r31-spike
python -B scripts/verify_r31.py --exe $releaseExe --dpi 1.0 --out outputs/r31/dpi100
python -B scripts/verify_r31.py --exe $releaseExe --dpi 1.5 --out outputs/r31/dpi150
```

Save the exact Release EXE and its hash before a diagnostic rebuild. Use
the Khronos layer and the release package override below, and run a
separate diagnostic pair at the available native scale. The script enables
synchronization validation and reads the layer's startup and teardown
messages. It checks for uninstrumented Release execution in timing mode.
Set `$validationExe` to the saved diagnostic executable path.

```powershell
cargo --config 'profile.release.package.module-engine-vulkan.debug-assertions=true' build --offline --locked --release --target x86_64-pc-windows-msvc -p module-engine-editor --features r31-spike
python -B scripts/verify_r31.py --exe $validationExe --dpi 1.0 --validation --out outputs/r31/validation
```

Each timing series includes one unscored pair and three paired repeats
with alternating order. A process records 240 intervals between successful
returns from `vkQueuePresentKHR` after 30 warmup frames. Capture reads the
next swapchain image at its native extent, outside sample collection.
The PNG and raw interval data accompany the process logs, executable
SHA-256 and fixture SHA-256. The measurement covers the application's
window cadence. For the timing gate every scored interval must be at
most `1000/60` ms. The captures must show legible fine text at 1:1 at
both native scales. The diagnostic pair requires zero validation errors
through teardown. See the [benchmark gate](bench.md#r24-culling-checks)
and [runner](../scripts/verify_r31.py).

<a id="r31-results"></a>

## Windows results — 25 November 2026

The scored snapshot was `25cfddf4c3abb545b749985ae2f9e6f65b803e57`
with source digest
`ad2d4f9cfda8a19f1b0951ddc8597b0a8c21e0b5faddf6e90318882b27563fdd`.
The same Release executable served both series. Its SHA-256 is
`c711ec8fc788c5e70a8b81eaa913f007ccfbf45f8d7e3cd95b6830a53c57de90`.
The independent verification queue ran the paired series on Windows
26H2 build 26300.10075, an RTX 4080 SUPER with driver 617.42 and a
144 Hz monitor. Both arms used BGRA8 UNORM and `IMMEDIATE`. Actual
winit scale and egui pixels per point were 1.0 and 1.5 at unit egui
zoom. The graph and tessellated workload matched between painters
within each DPI. At 100% each scored frame drew 24,246 vertices.
At 150% it drew 13,476 vertices. Timed frames used the existing texture
atlas without uploads.

| Native DPI | Painter | Median of run means, ms | FPS from median | Median of run p95s, ms | Worst interval, ms | Original PNG |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| 100% | egui-ash-renderer | 1.792 | 558 | 2.4955 | 3.2967 | [100% ash](evidence/r31/dpi100-ash.png) |
| 100% | Thin | 2.009 | 498 | 2.9539 | 7.3014 | [100% thin](evidence/r31/dpi100-thin.png) |
| 150% | egui-ash-renderer | 1.682 | 595 | 2.3278 | 3.3463 | [150% ash](evidence/r31/dpi150-ash.png) |
| 150% | Thin | 1.446 | 692 | 2.1733 | 3.2538 | [150% thin](evidence/r31/dpi150-thin.png) |

All 2,880 scored intervals met the `1000/60` ms budget. The table
summarizes application presentation intervals. The original 1580×940
PNGs show legible small labels in Module Blueprint and Module Library at
both scales. At 150% the canvas clips the right edge of the Viewport node
in both captures. The four published PNGs come from the first scored
pair at each DPI. Each painter and DPI combination produced identical
PNG hashes across its three repeats.

At native 100% DPI both diagnostic runs reported active Khronos
synchronization validation and ended with zero warnings and errors.
The separate diagnostic executable SHA-256 is
`c0ca3f19894a3e230a0decdcaf6b67fa5fad954bae96243027c3ffbd937094a3`.
The versioned [metrics and hashes](evidence/r31/metrics.json) include
every scored run. The full logs, raw intervals and builds are
preserved in the local verification receipt. A subsequent Clippy
correction changes the iteration over four-byte pixels in the
post-measurement capture. The scored series and Vulkan validation above
are bound to the original binaries.

The timing advantage changed with DPI. R3.2 will start with
`egui-ash-renderer` for the editor mesh and atlas path. The application
owns the R2 device, window, swapchain and frame lifetime. See the
[painter decision](decisions.md#r31-editor-painter).
