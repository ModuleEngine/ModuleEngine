<a id="архитектура-04"></a>

# Architecture 0.4

[English](architecture.md) | [Русский](ru/architecture.md) | [Documentation](README.md)

The project stores its architecture, authored scenes, and source code in editable JSON. `ModuleRegistry` describes root world/render modules; `SystemRegistry` installs executable gameplay subsystems. Neutral snapshots and scene updates separate world storage from rendering. Owned commands and state separate Rust Custom from specific storage implementations and Vulkan.

Technical engine capability, reusable gameplay and concrete game rules have
different owners. Combat, health, equipment and victory remain gameplay even
when reusable or registered through shared infrastructure. The accepted
[composition/editor direction](engine-composition-direction.md) clarifies this
boundary and graph roles. Current scope is one configurable engine with multiple
game projects; independent Engine management is deferred.
[Engine Overview](engine-overview.md) reads the capability inventory and navigates
to existing tools; it does not add an execution path. The architecture map remains
planned work.

<a id="пакеты-и-границы"></a>

## Packages and boundaries

| Package | Purpose |
| --- | --- |
| module-engine-core | GraphDocument, registries, Object/ECS, WorldObject/commands, SceneSnapshot/Material, authored scenes and prefabs, SystemExecutor, and GameSession. Does not depend on Vulkan, UI, image import, or rustc. |
| module-engine-content | Static glTF/GLB, PNG/JPEG, decoded cache, material/geometry, and asset dependencies of the selected project. |
| module-engine-vulkan | Raster/compute GPU graph, resource contracts, HLSL/DXC and SPIR-V validation, actual textured Forward/Deferred passes. |
| module-engine-custom | Partial visual representations of Rust, source-span edits, and preservation of opaque text. |
| module-engine-script | Rust worker compilation, persistent process, bounded JSON-lines IPC, and explicitly declared state with validate/commit/reject. |
| module-engine-project | Document history, checkpoints, diff/merge, migration, asset manifests, and dependency export. |
| module-engine-game-kit | Independently selected pure grid, history, campaign and action features. No Core, window, renderer or game dependency; consumers own their rules and payload validation. |
| module-engine-host | Shared GameHost/GameWorker, content, Custom, renderer and explicitly requested services; independent of Editor and game packages. |
| module-engine-host-services | Optional owned audio and versioned persistence services. The application chooses the writable state root; games own opaque save payloads. |
| module-engine-player | Separate feature-selected native game application, UI contribution dispatch and composition-root native package metadata. No production dependency on Editor. |
| module-engine-editor | Native egui/eframe UI, scene/execution editor and Full/Team policy; uses shared Host and Player presentation contracts. Registered immutable tools propose validated authored edits. |

The [G1 module plan](g1-modular-plan.md) distinguishes helper-library features,
registered executable systems, optional native services and source-installed UI/tools.
They are different extension contracts, not interchangeable graph nodes. Native
export resolves required owner/presentation/service metadata through a shared
catalog; generic export does not identify games by their presentation names.

<a id="r41-editor-extensions"></a>

## Game-owned editor extensions

[R4.1](product-roadmap.md#rebuild-r4) defines the registration contract. R4.2
moves the existing game-specific adapters to their owning packages. R4.3
selects which game packages are linked into each Editor build.

### Installation and activity

Game packages own their rules, schemas, presentation and authoring callbacks,
verification fixtures and assertions. An example project also owns its
content and tuning. The application composition root calls each selected
package's runtime installer, then calls its optional UI and verification
installers with the relevant registries. The Editor composition consumes authoring and
verification entries. Editor Game and standalone Player consume the same
game-owned presentation entry through a shared dispatcher. The Player
composition installs only the runtime and presentation entries it uses.

Registration types and callbacks used by both applications belong to a
game-independent contract layer using egui and Core data. The game `ui`
features and the two shells depend on that layer. Core and GameHost keep their
UI-free runtime contracts.
The generic Editor and Player dispatchers resolve registry keys independently
of the selected game's crate, title, and content directory. Registration
happens during application assembly, before callbacks run. Validate each
registration against its installed owner and reject duplicate keys without
replacing existing entries. IDs remain stable across builds and independent of
display labels. Existing personal layouts retain their pane identities.

Installation makes an entry available for selection. Authoring checks the
canonical `CompositionContext::activity` for the current document and the
current Full/Team permissions. A consumed schema can bring its owner into
authoring activity. A Play presentation is selected by the scheduled
activation modules of its runtime `GameContribution`. The shared Host permits
zero or one primary control presentation. Selection preserves the authored
execution order.

### What the package registers

| Entry | Key and owner | Callback and application boundary |
| --- | --- | --- |
| Panel or tool | Stable pane ID, installed module ID, display metadata and Project or Full Engine scope | Read the authored document and composition. A tool can keep a local draft and return a candidate using the `ModuleToolContext` and `ModuleToolResponse` contract. Editor owns docking, draft acknowledgement and publication. |
| Inspector | Exact component ID, owning module and schema version | Read the selected authored object and its effective component fields, with the authored values and their origin available separately. Propose edits to that component on the selected stable object ID. Editor uses the schema-generated Inspector when there is no matching adapter. |
| Verification scenario | Stable scenario ID, installed owner, required features/profile and inputs | Run only on explicit request in the verification host. The game supplies the fixture, actions and assertions. The runner checks eligibility, invokes the scenario with isolated inputs and a separate output directory, and reports the result or why the scenario is unavailable. |
| Play hooks | Active `GameContribution.id` and optional presentation callbacks | Read accepted session snapshots and app-provided UI status. Supply input, HUD, controls and optional aim or capture callbacks through the shared presentation contract. Return `GameInput` and any permitted application intent via `GameUiResponse`. Optional enter/leave hooks manage session-local presentation state. |

Panels and inspectors submit drafts or edit proposals through Editor's normal
validation and history. Edition, active owner and compatible schema must
still match when an edit is applied. The Editor checks exact owner/schema/SDK
pins, sources, asset references, and Project policy. Apply requires Stop.
Rejected proposals and Cancel preserve the accepted authored data. A no-op
Apply adds no history or local prefab override. The normal component edit
path preserves unknown JSON, nulls, exact keys and unrelated owners' data.

Play UI is dispatched against the accepted session. A successful Play start
enters the presentation once. Pause and Resume keep that session. Restart and
scene transitions are reflected in the next accepted snapshot. Stop leaves
the presentation once and releases its session-local state. Failed, cancelled
and superseded preparations never enter Play. The application routes input
through the existing focus and worker rules and owns session and window
controls. Editor sets `can_close_application` to false. Player honors close
requests according to the window capabilities it supplies. Game logic and
transactional tick hooks remain in the runtime installer.

### Omission and export

A composition built without a game leaves its registrations absent. A neutral
or compatible project still has the generic scene tools and schema-driven
Inspector. A persisted pane that lacks its registration is unavailable for
that build. Play with no presentation adapter retains the generic controls,
including Restart. Headless execution uses the runtime contracts alone.

A missing required owner or service or a mismatch in a pinned version fails
validation for the operation that needs the contract. Inactive opaque data
stays in the authored document. Full and Team retain their normal source and
policy checks. Player feature selection resolves required owners (including
active schema owners), presentations and requested services through the
native feature catalog. Full builds the selected set and Team uses a verified
prebuilt variant for that set.

### Verification routes for the split

Use the existing scenarios as reference routes and preserve their CLI entry
points when moving their implementations. For each run, record the fixture
and output path, source or artifact identity, enabled features and result.

| Package | Checks |
| --- | --- |
| G1 / Crate Garden | `g1.rules` owns the registered level palette. Exercise Apply, Undo/Redo and save/reopen under Full and Team. `g1.presentation` provides status-aware controls and menus in Game and Player. `--g1-check` exercises the campaign and its saves. |
| Puzzle | The generic Inspector edits active puzzle schemas. `puzzle.presentation` provides input and HUD. `--puzzle-check` exercises authoring, save/reopen and both world routes. A separately selected Puzzle Player covers delivery without ARPG. |
| ARPG / Two Arenas | ARPG owns mechanics, schemas and `arpg.presentation`, including projected aim, HUD and capture input. Two Arenas owns its content and encounter tuning. The Editor's `--game-check` provides the game-specific progression and defeat scenarios. |
| Neutral and Player | The minimal Player CI build checks the dependency graph with `--no-default-features`. For the Editor split, check the normal/build dependency graphs of the generic Editor and a composed build with an omitted game. In that build, open a neutral project with the usual tools, check that the omitted entries are unavailable, and verify that a project requiring the missing contracts is rejected without changing its data. |

At the R4.1 baseline, Editor still links the concrete games. The Editor wires
G1's palette, the games supply schema-driven Inspector data, and the Player
composition code assembles the three game UI contributions. See the [module
tool registration](../crates/editor/src/panes.rs), [shared presentation
dispatcher](../crates/player/src/presentations.rs), [Player export
contracts](engine-profiles.md#экспорт-windows-player) and [minimal Player
checks](ci.md#что-выполняется).

<a id="два-маршрута-исполнения"></a>

## Two execution paths

The retained demonstration `Runtime` path serves `World → scene.extract → registered renderer → viewport.output`. Additional inputs accept WorldCommands/RenderCommands; an optional Scalar graph controls speed. It serves one viewport and a limited set of routes. An ordinary root Node descriptor does not, by itself, create arbitrary subsystem behavior. This path is used by the old `project.json`, 3D Preview windows, and Object Tree/Inspector.

The game path uses `GraphDocument.game`: GameProject, scenes/prefabs, and ExecutionGraph. `SystemRegistry` registers a descriptor together with an `ExecutableSystem` factory. `SystemExecutor` creates instances and executes saved stages/nodes in the specified order. Resource types, required ports, `requires`, and `after` are validated before execution; the executor does not gain a branch for each new module name. `GameSession` owns the Play copy, a fixed 1/60 clock, and the canonical `GameState.world: EcsWorld`. The ECS path has no second provider; Object and explicitly connected third-party providers serve as compatible projections. `module-engine-host` connects the session, content, persistent Custom, a registered `FrameRenderer`, and explicitly requested services. It is shared by the Game / Scene window and Windows Player; the shared crate does not depend on the editor, Vulkan, or gameplay packages.

Game order lives in `game.execution`; it is not inferred from node positions or the legacy Scalar chain. In the new `game-project.json`, scene components define actor speed; the old scalar controller is not its game loop. A shared native-system API does not mean that a universal scheduler already executes all root Graph modules. The application registers Vulkan through its adapter; the shared host has also been verified with an independent CPU renderer and in headless mode. Other GPU APIs and a dynamic native binary ABI are not implemented yet. See the [shared host](shared-game-host.md).

Interactive `GameWorker` now has separate simulation and presentation schedules:
fixed 60 Hz gameplay and a 120 Hz render target using genuine interpolated poses
between completed states. Core provides safe renderer-neutral pose interpolation.
For the retained ECS path Host keeps current object poses and the changed
endpoints from the preceding tick. Compatibility paths use two raw snapshots.
Both attach content after interpolation. This leaves
authored/state data discrete, requires explicit lifetime/discontinuity boundaries,
and adds about one tick of visual delay. Extra renders never execute gameplay or
Custom. Static sub-tick samples are suppressed. Frame provenance and the actual
camera stay attached to completed pixels. Native evidence is source- and
scope-bound; a render target is not measured FPS. See [current GPU results](gpu-presentation-20261018.md). See [Host](shared-game-host.md)
and [worker](game-worker.md#interpolated-presentation).

<a id="мир-и-контент"></a>

## World and content

In the old Runtime, Object stores an actual recursive hierarchy, while ECS uses independent component tables. In the game path, mutable transforms, parents, colors, references, and dynamic components belong to one ECS. Native systems use column queries, and object DTOs are extracted for UI, IPC, and checks; see the [contract and lifetime](ecs-world.md). `from_objects` creates both stores from the resolved authored scene without demo angular velocities. Their commands and snapshots are equivalent for identical data. The renderer receives vertices/UVs, matrices, color, and Material; it does not query ECS or object storage. Matrices are row-major, and vectors are column vectors.

ContentLibrary attaches imported primitives by stable object ID. glTF local matrices compose with the object matrix; TextureImage is shared between snapshots through Arc. Import and export are restricted to the selected project/package root. Opaque base-color material runs on the GPU; skeletons, transparency, full PBR, mipmaps, and streaming are not implemented yet.

<a id="авторство-и-сохранённые-данные"></a>

## Authoring and saved data

EngineConfiguration defines active feature modules, exposed ranges, and ready-made rendering variants. ProjectConfiguration stores permitted values/conditions. Legacy PropertySchema filters fields by the actual participation of an active module/component. In Game / Scene, component schemas are registered separately from the systems that consume them. Membership is computed from the selected composition, including explicitly declared Custom `params.components`; the connected renderer activates mesh/material. Fields, defaults, constraints, and references come from the shared schema rather than an arbitrary prefab. Inactive authored values remain in the author's document; the runtime and newly stripped Team/Player data do not receive them.

Full Engine requires a complete compatible native source manifest. Team is always Project-only: its blueprint pins the architecture, game ExecutionGraph, bindings/source IDs, shaders, and versions; scenes, active prefabs, and permitted gameplay Rust remain game data. Modified Full sources remain editable. Team's `.sdk` is required for compiled Custom; the package tools/assets root is separate from the SDK source root. Standalone Player has ready-made workers and `.spirv` and does not require SDK/Rust/DXC. These are delivery policies, not a sandbox for arbitrary native/Rust code.

Personal Engine/Project dock layouts are stored outside the game. The initial Game layout is created only when no saved personal layout exists; an existing layout is retained. Game/Scene can be selected manually from its window list. Legacy Preview areas have independent cameras/selections while sharing a Runtime step. The game scene viewport displays a separate Play copy. Pause preserves the runtime; Stop destroys it and restores authored data. The execution graph is edited in a local draft; Apply requires Stop and validates the entire candidate. Its canvas coordinates are stored separately in `.user/full/game-systems-positions.json`.

<a id="gpu-и-измерения"></a>

## GPU and measurements

`render.vulkan` has an external Scene3D → Frame3D contract. Its subgraph connects real geometry, Forward/G-buffer/Deferred, SSAO, tone mapping, FXAA, and encoding. ResourceSpec defines format, semantic, extent, samples, and access; DXC/SPIR-V reflection validates bindings and actual image formats. Custom scene raster can explicitly enable the material ABI through `scene_materials`.

The normal native Vulkan viewport publishes an opaque GPU frame lease instead of CPU pixels. Windows bridges the separate Vulkan producer to WGPU/DX12 through shared D3D12 resources; Linux bridges separate Vulkan devices through dedicated OPAQUE_FD memory and requires exact device and driver UUIDs. The GUI bridge copies and premultiplies RGBA8 on the GPU into its own native `Rgba8Unorm` texture, preserving the byte rounding of the previous egui conversion without sRGB decoding. Normal presentation performs no pixel readback to CPU and no CPU texture upload. Unsupported or failed interop reports an error and preserves the last good image; it does not silently select CPU transfer.

The Vulkan device layer admits up to two GPU submissions on [tier A with timeline support](hardware-tiers.md#r21-device) and one on the compatibility path. The normal Player and editor Host each track one outstanding frame transaction. Four leased output images bound producer and consumer reuse independently of the command slots. `render_presented` returns `RenderSubmission::Pending(ticket)` after admission. `poll_presented(ticket)` checks completion without waiting and publishes that request at most once. `Busy` admits no work. `Ready(Frame)` supports immediate CPU/software output. The previous image, identity, camera and provenance remain current until completion. The slot retains command resources, atlas and pipeline references until the GPU completes their use. The GUI retains the shared image lease until actual consumer GPU completion, including when a surface is minimized. Explicit CPU render, Glow diagnostics and PNG capture can synchronously wait for readback. A conflicting CPU render is rejected while native work is pending.

Cancellation discards publication and retains the job until its GPU completion is observed. A dropped renderer hands over busy slots to a shared deletion queue, which polls their latest timeline value or compatibility fence off the render thread. Application shutdown drains this queue after renderer owners have stopped. `finish_gpu_retirements` reports consumer retirement and nested producer registered/joined/completed/unconfirmed/failed counts. A timeout (30 s per queued producer), device error, panic or worker start failure reports an error and retains unconfirmed resources and the device pin. The Linux consumer retains its existing quarantine for ownership-release failure.

The renderer-only diagnostic benchmark measures passes and explicit readback; Host and actual windows include different costs. The native window adds GPU interop/copy/conversion, HUD, egui/WGPU, vsync and OS scheduling. [GPU/HLSL](gpu-hlsl.md), [game verification](game-verification.md) and [Player](player.md) retain separate measurement criteria. New async sources require separately bound checks and an independent conclusion in the [async report](gpu-async-20261018.md); passing individual current gates does not establish no regression or final acceptance. The [historical GPU report](gpu-presentation-20261018.md) retains Windows source15/Linux source11 results and earlier failures, including the cold gate open at that time. The [18 October continuation](p0-continuation-20261018.md) separately records the later normal cold PASS 40.906/39.9128549 s, Windows22/16 and WSL Linux23/16 and scoped native/package/lifecycle checks for its own sources. These snapshots do not verify the new async implementation or certify hardware Linux, real device loss or physical scanout.

<a id="кандидаты-состояние-и-отказ"></a>

## Candidates, state, and failure

Rust compiles in the background. A compatible version transfers explicitly declared state; an incompatible one remains a candidate until reset or migration JSON is provided. Legacy Preview and game instances have separate owned state. A persistent worker eliminates process startup on every tick; after a timeout/panic/invalid packet, it restarts with the last accepted JSON. Globals are not a persistence/migration contract and are not portable between versions.

The game executor runs initialize/step for all systems and validates all staged outputs; the host transaction then validates and applies the world command batch, and only then are system commit hooks run. An error invokes rollback in the participating systems and host transaction, restoring world/resources/events/owned state. GameSession retains one ECS between fixed ticks and direct commands. Its [fixed-tick journal](ecs-world.md#lifetime-and-transactions) is applied to canonical storage after all commit hooks succeed. If a compatible external WorldProvider is selected, its instance is retained too; a new instance is created on scene entry, transition, or Restart. The provider must implement validate_commands without side effects and apply_commands atomically. To roll back after publication, the host applies an inverse portable batch; hidden state and external native effects require their own transactional hooks. Writing a file does not automatically become reversible, and rollback failure is reported explicitly. After a scene transition, scene systems are recreated; the persistent player carries components across while using the destination's local transforms/references.

Applying a compatible render graph/HLSL or permitted Project controls preserves the live game. Changing the native ExecutionGraph requires Stop; ordinary native world/renderer/UI source code requires a build. Legacy Runtime separately carries its provider across when world ID/type/parameters match, and clears incompatible renderer controls. These mechanisms should not be confused with saving game progress to disk.

<a id="история-и-объединение"></a>

## History and merging

Undo/redo and checkpoints preserve GraphDocument, including scenes, prefabs, graphs, source code, and parameters. They do not preserve live objects, timers, GPU resources, or Custom runtime state. Restoring a document does not rewind Play.

Merge normalizes ordinary Graph nodes by ID and edges by input. Scene object arrays and ExecutionGraph stage/node arrays do not yet have element-by-element ID-aware merging: overlapping changes to such an array produce a whole-array conflict with base/ours/theirs. On conflict, preview retains the base. This is a boundary of the current algorithm, not automatic merging of two scenes.

More detail: [format](graph-format.md), [module execution](module-execution.md), [scenes](scenes.md), [game editor](editor-game.md), [content](content.md), [Custom state](custom-state.md), [history](history-assets.md).

The precise format 4 contracts, explicit profile publication, migration into a copy, and separate feature-selected Players are described in [Engine profiles](engine-profiles.md). The shared Host does not depend on the editor; Player and the editor install the same source contracts through composition.

Authored scene bodies are hydrated in Core; versioned source IO belongs to
ProjectTools. [Separate scene files](scene-files.md) define project bindings,
accepted disk generations, copy independence, source trash/history and the
embedded Player projection. UI selection never defines runtime ownership.
