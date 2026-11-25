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
at unit egui zoom. Both series use `IMMEDIATE` presentation.

```powershell
cargo build --offline --locked --release -p module-engine-editor --features r31-spike
python -B scripts/verify_r31.py --exe <saved-release-exe> --dpi 1.0 --out outputs/r31/dpi100
python -B scripts/verify_r31.py --exe <saved-release-exe> --dpi 1.5 --out outputs/r31/dpi150
```

Save the exact Release EXE and its hash before a diagnostic rebuild. Use
the Khronos layer and the release package override below, and run a
separate diagnostic pair at the available native scale. The script enables
synchronization validation and reads the layer's startup and teardown
messages. It checks for uninstrumented Release execution in timing mode.

```powershell
cargo --config 'profile.release.package.module-engine-vulkan.debug-assertions=true' build --offline --locked --release -p module-engine-editor --features r31-spike
python -B scripts/verify_r31.py --exe <saved-validation-exe> --dpi 1.0 --validation --out outputs/r31/validation
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
