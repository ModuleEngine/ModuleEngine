<a id="статические-модели-текстуры-и-материалы"></a>

# Models, Animation, Textures, and Materials

[English](content.md) | [Русский](ru/content.md) | [Documentation](README.md)

Content is loaded by a separate `module-engine-content` library. The world stores objects and their stable IDs; the library attaches geometry and materials to the extracted `SceneSnapshot`. Object and ECS therefore use the same files, and the renderer receives one neutral structure.

<a id="что-загружается"></a>

## What Is Loaded

Supported formats are glTF 2.0 `.gltf` and `.glb` with indexed and nonindexed triangle primitives, node hierarchies, translation/rotation/scale or matrix transforms, UV set 0, `baseColorFactor`, and `baseColorTexture`. External `.bin`, PNG/JPEG, embedded bufferView images, and base64 data URIs are read. A composite mesh retains separate primitives and materials. glTF column-major matrices are converted to the engine's row-major convention and combined with the game object's transform.

The importer rejects morph targets, sparse accessors, nontriangle primitives, required extensions, transparent materials, and texturing through another UV set. The renderer uses a flat triangle normal and simple built-in lighting with base color. Metallic, roughness, normal, emissive, and occlusion maps are outside this lighting path.

PNG/JPEG can be loaded separately and assigned to existing geometry. The material factor and object color multiply the linear RGB sample. The indexed Vulkan path samples per-texture sRGB images with repeat addressing, mipmaps generated during GPU upload, and anisotropic filtering. The compatibility atlas path uses a nearest texel and explicitly decodes sRGB in HLSL. The importer still uses UV0 and the renderer uses its own sampler settings. [Vulkan material paths](hardware-tiers.md#r22-materials).

<a id="gltf-animation"></a>

## glTF Animation

The importer loads skins used by the default scene or the first scene if the asset has no default. It retains joint order, their hierarchy, inverse bind matrices, and paired `JOINTS_n`/`WEIGHTS_n` attributes. Missing inverse bind matrices default to identity. Integer weights use normalized accessors. Each vertex's nonnegative weights are normalized together across its influence sets. Joint matrices deform the vertices into glTF scene space and the engine then applies the authored object's transform.

Clips retain their source order, optional names, and duration. Translation, rotation, and scale tracks support `STEP`, `LINEAR`, and `CUBICSPLINE`. Linear rotations use shortest-path quaternion interpolation. A track holds its first or last key outside its own range. Sampling loops at the clip duration. `LoadedModel::animation_clips()` lists the clips and `LoadedModel::sample_clip(index, seconds)` evaluates one zero-based clip into neutral geometry. `LoadedModel::primitives` provides the rest pose for asset previews.

Assign the model through the ordinary `mesh.path` reference. GameHost automatically plays the first clip with tracks in the selected scene at presentation time derived from the accepted game clock. Scene previews at time zero show the first sample. Pause holds the current time and Restart returns it to zero. Each pose uses CPU-deformed positions with Vulkan's expanded vertex stream. GPU skinning, blending, and animation graphs belong to later stages.

<a id="ссылка-из-сцены"></a>

## Scene Reference

In the components of an authored object or template instance:

~~~json
{
  "mesh": {"path": "assets/models/demo_crystal.glb"},
  "material": {
    "base_color": [1.0, 1.0, 1.0, 1.0],
    "base_color_texture": "assets/textures/stone.png"
  }
}
~~~

The `material` component is optional: glTF materials are used without it. Override fields are also optional. Unchanged built-in objects use cubes; an imported object draws its own vertices and triangles. `demo_crystal.glb` is a small original test crystal with an embedded PNG texture.

<a id="api-и-жизненный-цикл"></a>

## API and Lifecycle

`ContentLibrary::new(package_root)` fixes the root of a particular project/package. `load_model(path)` returns a cached `Arc<LoadedModel>`. `enrich_snapshot(snapshot, bindings)` replaces cubes for objects with the specified IDs with model primitives, preserving the object's transform and color. Multiple primitives have the original object ID. `load_texture(path)` returns a cached `Arc<TextureImage>` for a material override. Textures are shared between snapshots through Arc; they are not decoded again every frame.

`invalidate()` clears the decoded cache after resource changes. Opening another game creates another library with its own root. Switching the world model does not change file-reading rules or the material ABI.

`import_model(source, destination_directory)` validates the model and all relative dependencies, then copies them while preserving their structure. The import source root is the selected model's directory; dependencies outside that directory are rejected during import. If the source uses a neighboring directory through `../`, first place the resource set in the project and load it from the project root. `import_texture(source, destination_filename)` first decodes PNG/JPEG and only then writes the file into the project.

<a id="владение-файлами-и-экспорт"></a>

## File Ownership and Export

Game resource paths are relative to the selected package. Absolute paths and URLs are not used. External glTF URIs are resolved relative to the glTF directory; percent-encoding is decoded before validation. The canonical path must remain inside the package, including symbolic link targets. During loading, relative `../` is allowed only if the result remains inside the project root. Copying also checks an existing file target.

`dependencies(model_paths)` returns the complete set of model, buffer, and image files for export. For a `.glb` with all content embedded, this is one file; for an external `.gltf`, the document and its actual dependencies. The exporter adds separate material override textures from component references. Player uses its own files and does not access the authored checkout.

Loading limits: 128 MiB per model/buffer/encoded texture file; textures up to 4096×4096, decoding limit 128 MiB; up to one million vertices per primitive and three million accessor elements; up to 4096 nodes and 128 hierarchy levels. Binary ranges are validated before iterator creation. The indexed renderer uses distinct GPU images within device limits. The compatibility atlas still allows up to 8192 pixels on each axis. Large-game texture streaming is a separate concern.

The per-model limits for animated content are 32,768 joint bindings across referenced skins, eight million vertex influence entries, and eight million scalar animation sample components.

<a id="проверка"></a>

## Verification

~~~powershell
cargo test --locked -p module-engine-content -p module-engine-vulkan
~~~

Tests import the bundled GLB, an external glTF with `.bin` and PNG, copy the complete set, and load a standalone JPEG. They check node matrices and rejection of external/encoded path traversal. A separate Vulkan test confirms that the imported mesh's UVs and texture change real Forward and Deferred pixels; a repeated frame after cache use matches.

The skin fixture checks joint ancestry, inverse binds, normalized integer weights, and rejection before copying when a weight accessor has the wrong shape. The native playback test requires the original Khronos GLBs from revision `e124e8d8d7330a5372bc2b4d00d39a4a9d7997ea`, Vulkan, and DXC. Put both files in the directory named by `MODULE_ENGINE_R71_SAMPLES` and set `MODULE_ENGINE_R71_OUTPUT` to a new evidence directory with an existing parent. The test verifies the file hashes, samples all clips, and saves PNGs from GameHost at ticks 0, 27, and 54 for each model's default clip and the static crystal. It checks deformation, differing pixels, a held pose, restart, and identical static pixels. [CesiumMan GLB](https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Assets/e124e8d8d7330a5372bc2b4d00d39a4a9d7997ea/Models/CesiumMan/glTF-Binary/CesiumMan.glb), [Fox GLB](https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Assets/e124e8d8d7330a5372bc2b4d00d39a4a9d7997ea/Models/Fox/glTF-Binary/Fox.glb).

~~~powershell
cargo test --offline --locked -p module-engine-host --test gltf_playback -- khronos_glb_import_and_play --ignored --exact --nocapture --test-threads=1
~~~

The format is implemented from the [Khronos glTF 2.0 specification](https://github.com/KhronosGroup/glTF/tree/main/specification/2.0); the supported-feature list above defines the actual boundary of this version.
