# Native Vulkan UI

[Русский](README.ru.md)

The Editor and Player use this crate to present egui windows with Vulkan. The root window and detached viewports share a logical device. A graph renderer attached to a window target can hand its completed output image to `FrameBridge`. The registered texture retains the image while the UI uses it.

The MIT-licensed copy of egui-ash-renderer 0.13.0 in `../vendor/egui-ash-renderer` distinguishes graph textures from egui textures. Graph output contains straight-alpha RGBA8. Its fragment shader premultiplies each sample before interpolation to match egui compositing. After editing the fragment shader, rebuild the checked-in binary from the vendor's `src/shaders` directory:

```sh
glslangValidator -V shader.frag -o shader.frag.spv
```

Run the window check from the repository root on a graphical desktop. It renders three alpha levels, drops the producer and compares pixels read from the swapchain with the expected composited values.

```sh
cargo run --locked -p module-engine-player --features ui,vulkan --example gpu_presentation_check -- work/window-pixels.json --window
```
