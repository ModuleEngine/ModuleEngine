<a id="vulkan-и-custom-hlsl"></a>

# Vulkan and Custom HLSL

[English](gpu-hlsl.md) | [Русский](ru/gpu-hlsl.md) | [Documentation](README.md)

The renderer executes raster and compute passes on Vulkan. In the editor, DXC compiles HLSL to SPIR-V; the renderer creates pipelines, images and barriers. Player export precompiles shaders. The two built-in routes differ in their actual sequence of GPU work.

| Pass | Input/result |
| --- | --- |
| gpu.forward | Scene3D → HDR color, world normals, linear camera depth. |
| gpu.gbuffer | Scene3D → linear albedo, normals, depth. |
| gpu.deferred | G-buffer → HDR lighting. |
| gpu.depth_normal | Scene3D → normals/depth. |
| gpu.ssao | HDR + normals/depth → HDR with ambient occlusion. |
| gpu.tonemap | HDR → linear LDR; ACES/Reinhard modes, exposure. |
| gpu.fxaa | Linear LDR → Linear LDR, spatial AA of the current frame. |
| gpu.color_encode | Linear LDR → rgba8unorm/srgb_display. |
| gpu.custom | Custom raster/compute pass with params.manifest and source_id. |

`gpu.ssao` reconstructs camera-relative world positions from camera-linear depth
and the shared camera basis. A neighbor contributes only above the receiver's
tangent plane and within the distance falloff; an unobstructed oblique plane
must not darken itself. The eight screen offsets and strength remain the narrow
built-in approximation, not a full physical occlusion model. See the
[plane/halo regression and delivery](release-042-ssao-verification.md).

<a id="редактирование-в-engine"></a>

## Editing in Engine

Select GPU Forward or GPU Deferred, then double-click to open the renderer's nested graph. Select a pass: the inspector shows its parameters and resource contracts. Replace with editable Custom HLSL copies the shipped shader text and manifest into the project, preserving the pass ID and external connections.

Edit the text in the HLSL tab and press Apply HLSL. An invalid candidate remains in the project with diagnostics; the previous working configuration is preserved. Changes to resources, bindings and entry points must agree with the manifest.

Standalone templates are in assets/gpu/custom_compute.hlsl, custom_raster.hlsl and custom_fullscreen.hlsl. They are available as ordinary sources.

![Nested GPU-pass graph](media/editor-render-v02.png)

<a id="manifest-прохода"></a>

## Pass manifest

Example params for an HDR compute filter:

~~~json
{
  "source_id": "my_filter",
  "gain": 1.0,
  "manifest": {
    "stage": "compute",
    "entry": "main",
    "workgroup": [8, 8, 1],
    "parameters": ["gain"],
    "inputs": [{
      "name": "color", "format": "rgba32float",
      "semantic": "hdr_linear", "extent": "viewport",
      "samples": 1, "access": "sampled_read"
    }],
    "outputs": [{
      "name": "color", "format": "rgba32float",
      "semantic": "hdr_linear", "extent": "viewport",
      "samples": 1, "access": "storage_write"
    }]
  }
}
~~~

source_id points to shader_sources. stage is compute or raster. For raster, raster_mode selects scene/fullscreen; entry specifies the fragment entry, and vertex_entry an optional user vertex entry. Scene raster accepts exactly one Scene3D; fullscreen/compute do not accept it.

ResourceSpec contains name, format, semantic, extent, samples, access. Image formats: rgba32float, r32float, rgba8unorm. Built-in pass semantics: hdr_linear, ldr_linear, albedo_linear, world_normals, camera_linear_depth, srgb_display; a scene has format:scene/semantic:scene3d. Input images use sampled_read, raster outputs color_write, compute outputs storage_write. Compatibility requires matching format, semantic, extent and samples.

Unknown manifest/ResourceSpec fields are rejected. Pass params allow only declared parameters, enabled:bool and Custom's supporting source_id/manifest. Up to 16 scalar parameters; parameters order determines value placement in push constants. Built-in tonemap separately supports a string mode.

## HLSL ABI

Descriptor set 0: input bindings first in inputs order, then compute output bindings in outputs order. Texture2D<float> and Texture2D<float4> are read through Load; sampler/buffer bindings are not yet present in this manifest. A storage image requires an exact vk::image_format attribute, for example rgba32f. Reflection checks compiled image types, bindings, storage format and LocalSize.

Push constants are 128 bytes: four camera float4s and four parameter float4s. Use the shipped [[vk::push_constant]] ConstantBuffer<Config> declaration; parameters occupy values, for example the first scalar is values[0].x.

Compute workgroup must match numthreads in SPIR-V, z=1, and the product of dimensions must be from 1 to 256. Check image bounds before Store. Scene vertex ABI: position float3, normal float3, color float4, ids float2, UV float2, atlasRect float4, locations 0–5, stride 72 bytes. The first 48 bytes preserve the previous data layout: an existing Custom shader can use only locations 0–3. An example vertex/fragment pair is in custom_raster.hlsl.

A scene raster pass with `scene_materials: true` uses the original material ABI on the compatibility path. Set 0 gains one sampled image after the explicit inputs for the base-color atlas from `SceneSnapshot`. UV (location 4) uses perspective interpolation. The flat atlasRect (location 5) supplies the origin and size in pixels for `Texture2D.Load`. The compatibility built-ins decode sRGB explicitly. Depth/normal and other Custom passes receive material resources only with an explicit declaration. [Import details](content.md).

Forward and G-buffer also declare `bindless_materials: true`. Built-in → Custom copies the complete manifest and both HLSL branches. For the indexed fragment the compiler defines `MODULE_ENGINE_BINDLESS_MATERIALS`. Set 1 has `Texture2D<float4>[]` at binding 0, `SamplerState` at binding 1 and `StructuredBuffer<uint>` at binding 2. The index buffer has one row per scene item. Add the flat, one-based `ids.y - 1` to `asuint(values[3].w)` to address the row. Use `NonUniformResourceIndex` on the stored image index and `Sample` with unwrapped UVs to preserve derivatives. The sRGB image view decodes the color. A dual-variant pass accepts only a Scene3D input and up to 15 scalar parameters. Push constants still occupy 128 bytes and the vertex layout stays unchanged.

The [resident stock path](hardware-tiers.md#r23-scene) uses an internal
vertex shader with storage buffers in set 0 at bindings 1–4 for positions
and UVs, indices, instances, and colors. It produces the stock varyings,
including the existing triangle and object IDs. The indexed fragment
interface in set 1 stays the same, with the R2.2 image indices now read from
the persistent scene buffer. A resident draw run uses `values[3].y/z` as
unsigned bit patterns to restore the object and triangle IDs for the current
published order. The row base at `values[3].w` maps the published object ID
back into its persistent material slot. Changing a stock pass to Custom selects the
published expanded vertex interface. Player export seals the internal vertex
stage alongside the stock compatibility stage.

Exports seal both fragment variants. An existing Custom pass declaring only `scene_materials` keeps its atlas binding and all 16 parameter slots on every device. [Device selection](hardware-tiers.md#r22-materials).

Inline HLSL searches for literal #include in assets/gpu of the selected package, then at that package's root. A nested quoted include first searches the containing file's directory; an angle include uses assets/gpu→root. JSON from a separate Full/Team package uses its marker/resource root. Standalone JSON without a marker does not make its directory a new compiler root; source_id is not a path. Export gathers these dependencies recursively. Resolved include contents are read before pipeline selection and included in the fingerprint: changing a header invalidates the compile/pipeline cache even when inline text is unchanged. This was verified by changes in real GPU pixels. A macro in place of an include name is rejected during dependency collection so export cannot lose unknown files.

Ready ACES/Reinhard and FXAA/off can be exposed to the Project author as allowed variants; Team does not change the graph/HLSL. DLAA/SMAA are not yet available.

<a id="заранее-скомпилированный-player-и-кеши"></a>

## Precompiled Player and caches

`precompile_graph_at(graph, sources, source_root, destination)` compiles and validates every stage of the selected graph, including vertex, and writes `destination/.spirv`. When `.spirv` is present, the renderer uses only these artifacts: DXC is not launched and there is no fallback to authoring tools. A missing/corrupt shader produces diagnostics and requires re-export. Include files remain in the package because their contents are checked against the same fingerprint. For other allowed graphs, the exporter calls precompile separately into the same directory.

In the editor, the persistent `.work/shader-cache` reuses SPIR-V by source, entry/profile and include-closure contents. A complete artifact is published by renaming a temporary file; an unfinished result must not be read by a concurrent compiler.

Root HLSL text uses the same shader identity with LF, CRLF or mixed LF/CRLF line endings. Both the direct source key and the root-text part of its dependency fingerprint canonicalize CRLF to LF; LF text keeps its previous key. Compilation and authored files retain their original bytes. Lone CR, other source changes, entry/profile and the resolved include paths still affect identity; included files contribute their exact bytes, including their line endings. This prevents an exporter and a separately compiled Player from disagreeing solely because their embedded root text has different Windows line endings. The hash is an internal cache key, not a stable external ABI.

An older self-contained Player package keeps its own executable and pinned artifacts unchanged. If a rebuilt runtime expects a different key from an older sealed package, re-export it with a matching current Editor/Player pair; do not rename `.spirv` files or relax package pins. A Team installation carrying an older selected Player variant needs a new supported Team delivery built from matching Full sources. Missing artifacts still fail without invoking DXC or falling back to authoring tools.

The backend retains pass images and any compatibility vertex buffer between frames with the same configuration/size; explicit CPU rendering also reuses its readback buffer. The GPU material atlas is uploaded when the decoded texture set changes and is reused. Native presentation returns a GPU lease without CPU RGBA pixels. Explicit CPU rendering with `set_detailed_stats(false)` reads only RGBA8 without depth, normals or color-ID reads. Ownership/coverage counts are not measured in this mode; zero does not mean an invisible scene. Submitted counts remain available.

Resource identity includes pass and output IDs, not just the pipeline set: removing AO and adding an identical pass with another ID requires a new image set. Otherwise a reused `ao.color` does not match the new `pass_id.color`. The GPU plan is checked separately from the image cache; settings contracts are checked against metadata without rereading shaders. For an unchanged frame, the plan is reused and external includes are checked at 250 ms intervals; explicit preparation always reads current includes.

An invalid external include preserves the previous working GPU plan and returns diagnostics; the window retains the last successful frame. Repeated frames use the saved error, and dependency bytes are checked after the interval. An unchanged failing set does not launch DXC again; fixing or restoring a file allows the next attempt without editing inline HLSL. The same dependency monitoring allows retrying rejected background Game/Scene preparation. Explicit `prepare` makes a fresh attempt regardless of this limit.

Temporary HLSL/SPIR-V files are deleted even when compilation fails. Dependency revision is rechecked before SPIR-V publication; saving an include during compilation rejects the stale result. The same pass fingerprint is checked before adding a new pipeline so mixed stages cannot be recorded under the previous key.

In the explicit CPU/diagnostic route, the readback buffer prefers `HOST_VISIBLE | HOST_COHERENT | HOST_CACHED` memory. Simply taking the first host-visible memory on a discrete GPU may yield a CPU BAR mapping that is slow to read. If a cached coherent type is unavailable, the backend uses the previous compatible host-visible coherent type. The transfer→host barrier and queue completion remain.

The renderer is measured separately with `cargo run --locked --release -p module-engine-vulkan --example gpu_benchmark`: 20 imported textured models, the complete standard Forward/SSAO/tonemap/FXAA/encoding chain, 1920×1080, 10 warm-up frames and 120 measured frames. On RTX 4080 SUPER after the readback-memory fix, the mean was 3.11 ms, p95 3.98 ms. This is a diagnostic renderer measurement with CPU readback, without game subsystems or egui output; a separate complete measurement corresponds to the finished Player criterion. JSON is written to `.work/gpu-benchmark.json`.

<a id="r25-graph"></a>

## R2.5 graph image lifetime and execution

The renderer derives image uses from the existing pass manifest, graph links
and ordered resource commands. Raster attachments, sampled and storage
images, bypass copies, explicit clear/copy and output consumers keep their
current formats and HLSL bindings. With synchronization2 the recorder emits
image barriers at the use sites and a memory barrier between images sharing
one backing range. A device without synchronization2 uses legacy Vulkan
barriers and separate ranges.

A frame slot can bind device-local viewport images to the same range when
the lifetimes are disjoint, their memory requirements have a common
device-local type and neither requires dedicated memory. Uses in the same
pass overlap. Explicit unprofiled diagnostic copies take each plane at its
last use. With timestamp profiling the copies stay after the timed graph.
The native window output has a separate allocation and stays alive through
the painter's final GPU use, with independent lifetimes for other frame slots.

When synchronization2, timeline semaphores and a distinct compute queue are
available, the compute suffix after the last raster pass runs on that
queue. A compute-only family is preferred, followed by a second queue in
the graphics family. Cross-family graph images and diagnostic staging
buffers use concurrent sharing. The compute submission waits for the
graphics prefix. Completed window frames join the graphics queue before
publication, and cancelled frames retire through the producer completion.
The compatible route uses the combined graphics+compute queue. Profiling
uses that queue so timestamps come from one domain.

`MODULE_ENGINE_GRAPH_COMPUTE=graphics` selects the single-queue control.
See the [S4 capture and memory check](bench.md#r25-checks)
and [Vulkan synchronization rules](https://docs.vulkan.org/spec/latest/chapters/synchronization.html).

## Native GPU presentation

The native Vulkan viewport publishes an opaque GPU frame lease. The
window painter samples the graph's RGBA8 image on the same logical device
and premultiplies each texel before filtering. The shader uses the encoded
values in the UNORM image. Normal presentation uses the image directly,
with no CPU pixel readback, texture upload, or intermediate display copy.
A failed frame preserves the last good image and reports diagnostics.

The Vulkan device layer admits up to two GPU submissions on [tier A with timeline support](hardware-tiers.md#r21-device) and one on the compatibility path. The normal Player and editor Host each track one outstanding frame transaction. Four leased output images bound producer and consumer reuse independently of the command slots. `render_presented` returns `RenderSubmission::Pending(ticket)` after admission. `poll_presented(ticket)` checks completion without waiting and publishes that request at most once. `Busy` admits no work. `Ready(Frame)` supports immediate CPU/software output. The previous image, identity, camera and provenance remain current until completion. The slot retains command resources, atlas and pipeline references until the GPU completes their use. The GUI retains the shared image lease until actual consumer GPU completion, including when a surface is minimized. Explicit CPU render, Glow diagnostics and PNG capture can synchronously wait for readback. A conflicting CPU render is rejected while native work is pending.

A valid graph/HLSL candidate may compile and warm caches while a request is pending without selecting the candidate for that request. In-flight commands pin every referenced pipeline/layout and their scene/atlas/extent resources until completion; invalid candidates preserve that request and the accepted wrapper. Submitted and completed GPU-frame counters are distinct.

Cancellation discards publication and retains the job until its GPU completion is observed. A dropped renderer hands over busy slots to a shared deletion queue, which polls each pending slot's timeline or fence off the render thread. Application shutdown drains this queue after renderer owners have stopped. `finish_gpu_retirements` reports consumer retirement and nested producer registered/joined/completed/unconfirmed/failed counts. A timeout (30 s per queued producer), device error, panic or worker start failure reports an error and retains unconfirmed resources and the device pin.

New async sources require separately bound checks and an independent conclusion in the [async report](gpu-async-20261018.md); passing individual current gates does not establish no regression or final acceptance. The [historical GPU report](gpu-presentation-20261018.md) retains Windows source15/Linux source11 results and earlier failures, including the cold gate open at that time. The [18 October continuation](p0-continuation-20261018.md) separately records the later normal cold PASS 40.906/39.9128549 s, Windows22/16 and WSL Linux23/16 and scoped native/package/lifecycle checks for its own sources. These snapshots do not verify the new async implementation or certify hardware Linux, real device loss or physical scanout.

<a id="ограничения-и-требования"></a>

## Limitations and requirements

Full source validation requires `crates/render-vulkan/src/retirement.rs`. A Full package omitting that module from its source inventory must be rejected before enabling Engine authoring.

Requires Vulkan 1.3, dynamicRendering, a graphics+compute queue, host-visible coherent memory and device-local images. RGBA32_SFLOAT/R32_SFLOAT/RGBA8_UNORM formats must support the color attachment/sample/storage/transfer operations in use; D32_SFLOAT must support depth attachment/transfer src. Presence of vulkan-1.dll does not prove adapter capabilities: actual backend creation checks them.

Buffer/Image receive an RAII owner immediately after the first Vulkan handle is created. Failure in memory selection/allocation, binding or image view creation releases already created parts. The native regression injects failures at intermediate stages, checks handle/device-memory balance and a subsequent successful GPU frame; it does not require deliberately exhausting video memory.

Viewport images have one layer, samples=1; maximum 4096 on each axis and 4 million pixels. MSAA/resolve and temporal AA are deferred. The window shell owns the swapchain and samples the graph image. Explicit CPU and diagnostic output reads pixels synchronously.

Clear/copy commands address pass_id.output_name and execute after the producer in submission order. An invalid resource, incompatible copy or write after the first consumer is rejected. A portable Rust enabled override takes priority over params.enabled; disabling is allowed only with a compatible single-image bypass.

For built-in geometry, the renderer reads visible IDs and measures ownership. An arbitrary custom scene shader has submitted triangles and coverage available; zero ownership counts mean this measurement is absent, not proof that visible geometry is absent.

<a id="punctual-lighting"></a>

## Stock punctual lighting

The Vulkan composition publishes the shared `light` schema with the Engine lock.
The component accepts `directional`, `point` and `spot`. It has linear RGB,
directional illuminance in lux or local intensity in candela, a local range,
and inner and outer cone half-angles in degrees. An authored transform sets
position and aims the local −Z axis after parent transforms. A light without
a `mesh` attachment contributes no geometry. Scene, the retained Game
snapshot and the exported Player consume the same committed light records.

Stock Forward, the paired G-buffer and Deferred route, and the stock transparent
pass use one GGX BRDF and punctual attenuation. With no `light` attachments,
existing scenes keep their default lighting. Cluster compute assigns local
lights to 64×64 pixel tiles and 16 logarithmic camera-depth slices using
conservative sphere intersections. Directional lights are global. Each
cluster uses a bitset over the complete local light list. The current stock
path accepts at most 1,024 lights and reports an over-limit scene as an error.

The two cluster buffers are private stock shader resources. They are not part
of the Custom manifest or its push constants. Both the compatibility atlas and
indexed paths can shade the clusters. If the device cannot fit the extra storage
descriptors alongside indexed or resident resources, the renderer selects a
compatible path. Use [the S4 check](bench.md#r52-checks) for an offscreen
measurement on the reference GPU.
