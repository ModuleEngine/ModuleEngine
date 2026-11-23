<a id="открытые-вопросы-после-04"></a>

# Open Questions After 0.4

[English](questions.md) | [Русский](ru/questions.md) | [Documentation](README.md)

Decisions accepted for 0.2/0.3/0.4 are recorded in [decisions.md](decisions.md); the game-stage scope is in [next-stage.md](next-stage.md). Scenes, static content, persistent Custom, and Windows Player are already implemented with the documented limitations. This list concerns further development.

The [current single-engine scope](engine-composition-direction.md#deferred)
defers independent Engine catalogues, switching, shared-version storage and
cross-project update adoption. They are not prerequisites for the next UI work.

| Question | What the decision changes |
| --- | --- |
| Which overview, map navigation and module-settings layout best supports the first route while retaining existing layouts and rights? | Stage 2 and later UI design. Stage 1 now supplies the [source metadata/evidence contract](engine-capabilities.md); no universal graphical executor or permission change is presumed. |
| Which tasks should next receive a full visual Rust representation: control flow, functions/types, or collection handling? | The priority of the next subset and test examples. For now, unknown fragments remain editable text. |
| How should ASTs be matched after ambiguous code movement, copying, and renaming? | Rules for preserving manual layouts and persistent IDs. The current reparse may reset them. |
| Which graphics backend is needed after Vulkan, and what limitations of the portable renderer API are acceptable? | Portability of shaders, resources, and commands. |
| When are a native swapchain and several simultaneous producer submissions needed beyond the current single pending submission/four output leases? | Full-frame profiling, latency/resource budgets and evidence for further parallelism. Normal native presentation already avoids CPU pixel readback; explicit diagnostic readback remains separate. |
| What scene scales and geometry types are needed for virtualized geometry using a Nanite-like approach? | GPU buffer contracts, mesh loading, culling/LOD, and streaming. The current Custom manifest describes images and Scene3D; arbitrary buffer/geometry ports and virtual geometry are not yet implemented. |
| What temporal resources and motion vectors are needed for TAA/frame history? | New resource contracts; current FXAA does not retain the previous frame. |
| Which operations do lightweight artist, game author, and engine developer roles actually need? | Refinement of separate future artist/designer profiles. Version 0.3 already has Full/Team with an authoring boundary; full source export is preserved. |
| Are third-party dynamic plugins distributed separately as binaries needed? | A stable ABI, version management, and loader. Extensions currently build from source. |
| What content extensions are needed after static glTF/GLB and PNG/JPEG? | Skeletal animation, transparency, full PBR, and other formats. Initial static models/textures/materials are already implemented in 0.4. |
| What rules are needed for long-term progress saving and state versioning? | Explicit Custom state between frames and hot apply are already implemented; writing game saves and long-term migration remain a separate task. |
| Is execution of untrusted third-party Rust required? | A separate, real sandbox model; the current worker has ordinary process permissions. |
| Which AI provider and which project data may [AI-assisted live authoring](product-roadmap.md#ai-authoring) use? | The prototype's external service, what leaves the machine, and whether generated Rust needs the sandbox above. Decided by the owner before AI-A, which is not before R1/R4/R6. |
| When will simultaneous live editing by several users be needed? | A server, operation authorship, and conflict strategy. Local history and file merge are currently selected. |
| How should the scene format develop further? | Version 0.4 has an authored scene, stable IDs/references, templates, and overrides; next are extended-component versioning, nested templates, and game saves. |
| How should multiple installed versions of one module and automatic migrations be supported? | A multi-version resolver and third-party module adapters. Currently, a pin must exactly match the installed implementation; known formats 1–3 support explicit migration into a copy with a verified contract snapshot. |

Additional technical limitations of the current version are listed in custom-rust.md, gpu-hlsl.md, and history-assets.md; they are not promised capabilities.
