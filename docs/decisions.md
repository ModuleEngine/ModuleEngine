<a id="решения-и-рабочие-допущения"></a>

# Decisions and Working Assumptions

[English](decisions.md) | [Русский](ru/decisions.md) | [Documentation](README.md)

This log separates the accepted product scope from selected implementation details. A capability is considered implemented only together with its limitations and verification.

<a id="принято-для-02"></a>

## Accepted for 0.2

| Decision | Consequence |
| --- | --- |
| The first GPU backend for game rendering is Vulkan. | Real raster/compute passes and resources, forward/deferred graphs. Other game-rendering graphics APIs come later. |
| Custom HLSL is ordinary text for a standalone pass. | The manifest describes the stage, entry points, and resources; code is available in the project. |
| Rust Custom really compiles and applies without restarting the editor. | rustc diagnostics, a separate candidate, and preservation of the previous working version. |
| Rust controls execution through portable typed commands. | The same logic works with both worlds and a registered renderer; direct Vulkan calls from ScriptContext are not provided. |
| Visual Rust support expands gradually. | Unsupported constructs are preserved as opaque text and can compile as ordinary Rust. |
| Local undo/redo, checkpoints, diff, and three-way merge are needed now. | Conflicts are preserved explicitly; live coediting is deferred. |
| The first AA is current-frame FXAA. | Temporal AA and temporal resources are deferred. |
| Export includes files, tool versions, and restoration with downloads. | A full offline SDK is not packaged; internet access during restoration is permitted. |
| The full source project remains editable. | Role-based distributions were deferred until 0.3; the implemented Full/Team scope is described below. |

<a id="выбранные-детали-реализации"></a>

## Selected Implementation Details

- Native Vulkan presentation uses opaque Core GPU leases and explicit application-supplied targets to remove the GPU→CPU→GPU pixel roundtrip while keeping Host/Core independent of native APIs. Windows and Linux use the same logical Vulkan device for graph production and egui sampling. Four producer leases and actual consumer GPU completion bound reuse. The [Player contract](player.md#native-gpu-presentation) defines publication, cancellation and diagnostics. The [R2.1 device decision](#r21-device-prototype) defines the producer's slots and deletion. The [historical October async stage](gpu-async-20261018.md) and [GPU presentation report](gpu-presentation-20261018.md) give source-bound checks and their limits.
- Rust compiles into a worker executable and exchanges owned JSON packets without a Rust DLL ABI. In 0.2/0.3, a worker was created on every tick. In 0.4, the process is persistent, state is explicitly declared and owned by the host; a timeout stops the worker, and the next tick restores the last accepted state. This does not isolate OS access or promise migration of globals.
- The root Cargo.lock is authoritative for worker dependencies as well. The generated worker build checks package versions and sources before an offline --locked build; its own package has the technical version 0.0.0.
- Tools are pinned in toolchain-manifest.json: Rust 1.99.0, DXC 1.9.2609.5. Doctor does not modify the system; explicit Restore installs tools into the project directory and verifies DXC hashes.
- History stores document snapshots, and merge normalizes nodes by ID and connections by input. A conflict leaves the base value in the preview document and both alternatives in the report.
- Resources use AssetId, a relative path, and SHA-256. HLSL include dependencies are exported recursively; a macro in place of an include name is rejected.
- The 0.1 software renderer is retained as a separate module.


<a id="r81-meshlet-prototype"></a>

## R8.1 S5 meshlet choice — 25 November 2026

Keep R2.4 as the production path and retain the task/mesh path as an
explicit Tier B prototype. On the same S5 scene and RTX 4080 SUPER,
the median per-run mean GPU graph time fell from 4.552 to 2.461 ms and
peak process working set fell from 2,391.8 to 1,560.4 MiB. Building and
packing the meshlets consumed 15.93 seconds. A later Tier B design needs
precomputed geometry before ordinary scene loading. The same view had
two slow window intervals on the mesh path, so the experiment supports
further performance work without establishing an R8.3 cadence guarantee.
[Conditions, costs and image evidence](r81-s5-meshlets.md#r81-result)

<a id="rv-c-arpg-tick"></a>

## ARPG typed access and the role of Bevy — 24 November 2026

Keep Bevy as Core's ECS storage for native rows and change tracking and
as the executor of the single-threaded authored schedule. Stable IDs,
the staged ECS journal and validation remain Core contracts. Start typed
gameplay access with ARPG-owned `Actor`, `Health`, `Melee` and `Ai`, alongside
Core's existing native `Transform`. The ARPG package owns its gameplay types
and admits them through registered schemas. The saved documents, Editor and
Rust IPC continue to use the portable representation. Verify typed access
against the same authored values, rejection and retry contracts.

In the 20-enemy Release encounter, those four types accounted for 97.96%
of 206,149 reads through ARPG's component JSON decoder. This is an access
count. The measured time concentration was in `ArpgFrame`: encode, decode
and the decode input clone were 48.31% of the original fixed tick. Prioritize
retaining a typed `ArpgFrame` across the ARPG systems within a tick and
converting at the portable boundary. Measure that path and its rejection
behavior before changing the storage strategy for other components.

Keep the current rollback representation for this delivery. Sharing resource
and module data payloads and deferring `control` copies reduced checkpoints
from 36.92 to 13.00 µs. The seven round means had medians of 966.0 and
962.6 µs for whole ticks, with paired changes from −5.80% to +3.32%. Both
versions preserved the previous state on late rejection and matched the
fresh accepted tick after retry through `GameHost`. The result supports
targeting repeated serialization and hot access before a broader storage
change. [Measurement, prototype and rollback](evidence/rv-c-arpg/README.md)

<a id="r21-device-prototype"></a>

## R2.1 Vulkan device choice — 22 November 2026

Use gpu-allocator 0.28 for producer-owned resources and two graphics slots
with a timeline semaphore on tier A devices with timeline support. The
second slot allocates frame resources only when a caller has two requests
outstanding. Keep the single-slot fence path for other compatible adapters
and keep the GUI's external allocations dedicated as required by interop.
Select a dedicated transfer family for atlas uploads when one is exposed.
Retain the graphics queue fallback, the current graph and shader bindings,
the direct-mapped vertex stream, and the existing native presentation lease
contract. Resource bindings and the GPU scene representation belong to R2.2
and R2.3.

The prototype used native S3 at 1920×1080 on an RTX 4080 SUPER, driver 617.42,
with WGPU/DX12 GUI, Vulkan producer, and Immediate present mode. Each of
three modes ran five interleaved scored processes with 30 warmup and 240
measured frames after one prewarm per mode. Validation and profiling were
off during timing. The launcher checked for competing builds and Editor
processes between runs. The table gives medians of per-run means and p95s.

| S3 metric | 99884e6 original | Allocator, one slot | Allocator, two slots |
| --- | ---: | ---: | ---: |
| Frame interval mean / p95, ms | 26.938 / 35.262 | 27.812 / 31.390 | 24.504 / 26.815 |
| CPU render and submission mean / p95, ms | 20.732 / 22.473 | 21.962 / 23.972 | 21.587 / 23.376 |
| Peak process working set, MiB | 306.2 | 315.6 | 380.4 |
| Allocator Vulkan blocks, in use / reserved, MiB | — | 2, 248.7 / 320 | 4, 497.4 / 640 |

The two-slot run reached two pending tickets and reduced the observed
median frame interval by 9.03%, within the tolerance from this series'
baseline spread. CPU submission changed by +4.12%. Peak working set changed
by +24.26%. The existing comparison rule flagged that memory result as
worse and borderline. This prototype series was run once and establishes
the tradeoff for R2.1, with no general performance or memory parity claim.
One-slot allocation alone did not show a frame-time gain in this series.
Retain two slots to allow CPU recording and GPU work to overlap, and keep
the normal caller's single outstanding request where it owns one frame
transaction. Allocator reserved bytes include host-visible heaps and do not
measure total process VRAM.

Baseline HEAD was 99884e6794737a3e40f3128fe14d3a919388dc23. The prototype
was built at that HEAD with source digest
`fd3b3fb0fd0826edb0895b1ef3f1ddb3305d721ef87f3a6dc64b1deeec16ca1a`.
The local `outputs/r21/s3-matched/` contains the source snapshot, binary
hashes, comparison, raw reports and GPU telemetry.
`outputs/r21/prototype/` preserves the prototype executable, changed sources
and launcher. This decision precedes migration of the default device path.
The numbers above do not cover dedicated transfer or final validation.


<a id="r31-editor-painter"></a>

## R3.1 Vulkan editor painter — 25 November 2026

Use `winit` and `egui-winit` with `egui-ash-renderer 0.13.0` as the
starting point for R3.2. The spike establishes that either painter can
borrow the logical device created by R2 for a native editor window.
Both meet the timing and text criteria, and the order of median timing
reverses between 100% and 150%. Use the library's mesh and atlas
implementation to reduce the rendering code maintained by ModuleEngine.
Retain application ownership of WSI, queue submission and frame
retirement. Keep the thin painter in the
opt-in spike for comparison. Direct Scene and Game image sampling
belongs to the R3.3 resource and synchronization contract.
See the [Windows results](editor-vulkan-spike.md#r31-results) and
[library API for caller-owned textures](https://docs.rs/egui-ash-renderer/0.13.0/egui_ash_renderer/struct.Renderer.html#method.add_user_texture).

The R3.2–R3.3 Editor and Player windows require a Vulkan device to paint the UI, including a Player built with only the `ui` feature. Window creation reports an error if no compatible device is available. Game-graph Vulkan renderer registration still depends on the `vulkan` feature and the project’s module selection. Headless builds without `ui` do not require a UI device.

<a id="рабочие-допущения"></a>

## Working Assumptions

The following rules were adopted by the team for implementation; the user did not select every detail individually:

1. Changing the renderer and sources preserves the world/time when the world's ID, type, and parameters match. Explicitly replacing the world resets it.
2. An invalid candidate preserves its text and the last working version, and diagnostics show its rejection.
3. Migration does not overwrite the source during loading; writing an existing file creates a backup.
4. Module IDs and their ports must remain stable. An incompatible change will require explicit migration.
5. Rust node positions can be transferred only when matching is unambiguous. Current limitations are described in Custom; full persistent AST IDs are not yet promised.

Further product decisions are in [open questions](questions.md).

<a id="принято-для-03-ответы-2428"></a>

## Accepted for 0.3 (Answers 24–28)

| Decision | Implementation and boundary |
| --- | --- |
| Active modules contribute their own typed fields/settings. | Descriptor.properties, active participation, and defaults; physical removal of all unused code from the binary comes later. |
| Object/Component fields belong to actual participants. | Selected object and attach/detach; Global/Module fields are in Project Settings. |
| Independent Blender-like Engine/Project areas. | Split/join/resize/type selection, personal persistence, separate cameras/navigation with a shared runtime. |
| Engine owns architecture and exposure. | Project changes declared ranges/ready variants/conditions; new/reordered passes are prohibited. |
| Graph/HLSL/Custom Rust hot apply is available now. | Native world/renderer/UI Rust requires a build; hot native modules come later. |
| Team never includes Engine. | Explicit package marker, frozen blueprint and gameplay whitelist, .sdk for compiled Custom. |
| Full authoring requires a complete compatible source inventory. | Version/required files/active module paths are checked; legitimate source edits are marked modified. Tool availability is separate. |

Working details: .module-engine-package.json; personal .user/<profile>/<workspace>.json outside the document; the gameplay whitelist covers only active custom.rust.update; the SDK lock is a subset of the full lock by name/version/source. Team backend pins cover the HLSL include closure and tools; ordinary game assets remain game data. The mode is policy, not an adversarial security boundary.

Historical 0.3 boundary: the inactive archive was preserved by Full/local history and excluded from runtime/Team; Networking was a schema test fixture; FXAA/off and ACES/Reinhard worked. Transform/Color in the old Object Inspector changed the preview; authored game scenes and an importer did not yet exist. In 0.4, new game scenes are saved separately; the previous windows remain as legacy. Known first-party pins from older stages are updated through an explicit compatibility list; third-party conflicts are preserved; current migration details are in [graph-format.md](graph-format.md).

<a id="принято-для-игрового-этапа-04"></a>

## Accepted for the 0.4 Game Stage

| Decision | Implementation and boundary |
| --- | --- |
| The verification game has a 3D ARPG view, WASD, and mouse controls. | Two separate arenas with a short transition: 20 small enemies, then a boss. Angled overhead camera, one melee attack toward the cursor, each enemy takes damage once per swing. |
| The boss has a telegraphed heavy attack. | Windup, a marked area, and dodging through movement; no additional phases or dash. |
| The graph defines stages and subsystem order. | Shared ExecutableSystem factories, typed resource contracts, and before/after/production checks; a new module requires no branch in the executor. Native registrations require a build. |
| Play uses a copy of the authored scene. | Pause preserves the copy; Stop destroys it; runtime spawn/death is not automatically written into the source. |
| Linked templates have separate instance overrides. | Default changes propagate; local Transform/ID/references and explicitly overridden fields are preserved. |
| Custom static models, textures, and simple materials are needed now. | glTF/GLB, PNG/JPEG, and opaque base-color on the GPU; skeletons, transparency, and full PBR are deferred. |
| Collisions block movement; attacks use areas. | Controlled bodies/rectangles, without dynamic rigid bodies or navmesh. |
| Basic audio plays in response to events. | A separate audio system and Windows WAV endpoint, one voice; spatial audio/mixer later. |
| Rust Custom preserves explicitly declared state. | Persistent worker and transactional JSON packets; compatible hot apply preserves state, while an incompatible change requires explicit reset/migration. |
| The standalone game targets Windows first. | A Player package with prebuilt workers/SPIR-V, without SDK/Rust/DXC on the player's machine. Native editor code is not yet physically excluded from the shared executable. |
| The criterion is 60 FPS, 1080p, up to 20 active enemies on the current PC and fixed content. | Processing and real window measurements publish load, mean/p95/max, and disclose scope; unit tests or renderer-only FPS do not substitute for verification of the finished game. |
| Documentation accompanies changes. | API/contracts, the purpose of each module, data, actual editor actions, checks, and boundaries are described in docs. |

Working assumption: long-term game saves are outside this stage. Linux, seamless streaming, networking, temporal AA, and physical exclusion of unused native code remain future tasks. The glTF/GLB and PNG/JPEG assumption was chosen for implementation; the user did not specify particular formats individually. [next-stage.md](next-stage.md) connects the decisions to the implemented subsystems.

## Accepted long-term portfolio goal — 17 October 2026

| User decision | Consequence |
| --- | --- |
| Develop 5–10 distinct finished games to demonstrate engine configurability. | Use several playable projects as the product-level proof of reuse; existing technical examples do not automatically satisfy finished-game acceptance. |
| Cover 2D, 2.5D and 3D, pixel art, polygonal, realistic and simple mobile-style graphics, with different genres and mechanics. | Track visual, spatial and gameplay coverage explicitly; variants of one example are insufficient. |
| Mobile-style currently concerns the game, mechanics and graphics on PC. | Mobile ports are future work, with their OS targets still to be chosen; this portfolio does not require current device ports and does not claim phone support. |
| Fix the current problems before starting development of the portfolio games. | P0 includes audit, repair of all confirmed defects and measured performance problems in existing features, and final rechecks. P1 implementation starts after P0 passes; unchanged ARPG/cold-start criteria and required checks cannot remain failed or unverified. Missing future subsystems stay in later phases. |

The [portfolio roadmap](portfolio-roadmap.md) proposes a minimum five-game matrix,
up to five additional candidates, capability dependencies and common acceptance
gates. Those exact genres, game briefs, order and budgets are planning proposals,
not additional user selections. The finite portfolio demonstrates the named
cases, not universal suitability for every possible project. Each implementation
increment still needs a bounded goal and checks; this decision does not reopen all
future subsystems in unrelated tasks or mark existing performance targets passed.

## Accepted EN-01 language scope — 22 October 2026

Before portfolio-game implementation, all engine-owned shipped Editor, Player and CLI display text is English. This includes menus, panes, tooltips, confirmations, progress, errors, schema/module display metadata, dynamic subsystem diagnostics and the display defaults of newly created templates. Public documentation retains complete separate English and Russian versions.

Existing authored names and content, paths, stable IDs, API/JSON keys and module/schema/SDK pins are preserved. Raw external compiler/OS diagnostics and OS-owned native file-dialog controls retain their original language. Unicode regression fixtures and opaque source comments are not translated. No localization framework, locale selector, dependency, format migration or runtime redesign is introduced. Existing projects are not rewritten merely to translate their authored text.

Acceptance requires a classified inventory of product text, applicable tests and actual current Editor/Player/CLI workflows, rendered-label checks and independent source/artifact-bound review. Historical P0 evidence is retained under its original scope; a translation alone does not recertify timing, hardware Linux or physical input-to-display behavior. See the [UI language contract](editor-workflow.md#ui-language).

## Accepted P0 presentation change — 17 October 2026

The user approved separating fixed 60 Hz simulation from more frequent, genuine
interpolated presentation to address the measured Player cadence failure.
Conventional interpolation between completed states adds about one fixed tick
of visual delay. It must preserve authoritative gameplay, input/event/Custom
execution, authored data and the existing mean/p95/load criteria. Additional
sub-tick frames must use changed evaluated poses or camera; repeating the same
presentation or changing counters/timestamps cannot establish a performance pass.
History requires explicit lifetime/discontinuity boundaries, including a module
signal for teleports, scene changes and accepted code/configuration replacements.
Implementation and final native verification belong to P0; this decision alone
does not close the failed gate. See the [shared Host](shared-game-host.md).

<a id="engine-direction-20261113"></a>

## Accepted engine composition and editor direction — 13 November 2026

The [direction record](engine-composition-direction.md) contains the complete
agreement and separates current behavior from future work:

- Use a common framework with selected technical modules; keep reusable gameplay
  and concrete game rules separately owned. Combat/health/equipment remain
  gameplay regardless of reuse, registration or crate naming.
- Prioritize that responsibility boundary and the route to understand the
  Engine's composition, choose a module and configure it.
- Use an Engine overview as the main entry point, an additional architecture map
  and focused editable graphs where connections carry meaningful behavior.
  The retained root route is not a universal engine execution graph.
- Make Engine configuration/profile, installed package, current game and rights
  identifiable; distinguish workspace navigation, project opening and Engine
  replacement. Preserve authored data, personal layouts and Full/Team rules.
- Require real dependency/ownership/lifecycle contracts and efficient execution;
  optional modules must not introduce unrelated required settings or behavior.

This documentation does not implement the UI or authorize a runtime/schema
migration. The subsequent single-engine scope below defers shared Engine
storage/version adoption and package-selection planning. Overview metadata
remains an [open decision](engine-composition-direction.md#open-decisions).
The proposed screen is not a completed implementation; existing acceptance
reports retain their own sources and scope.

<a id="single-engine-20261113"></a>

## Scope refinement: one configurable engine — 13 November 2026

Work with **one ModuleEngine and multiple game projects** for now. Different
module sets, render paths and settings are configurations of this engine.
Engine is the technical-configuration workspace; Project is the selected game's
workspace. Retain modularity, the technical/gameplay boundary, overview-first
navigation and focused subsystem graphs.

Defer independent Engine products/documents, their catalogue/selector, shared
version publication and cross-project update adoption. Those earlier identity
questions no longer block the next UI work. Existing project-local data,
profile/contract checks, package bindings and Full/Team rights retain their
current meaning; this decision does not introduce global settings propagation
or authorize a migration. The [direction record](engine-composition-direction.md)
is updated in both languages. This change is documentation only.

<a id="engine-rebuild-20261121"></a>

## Accepted engine rebuild priority — 21 November 2026

| Decision | Consequence |
| --- | --- |
| The [engine rebuild track](product-roadmap.md#rebuild) is the current priority: `bevy_ecs` world storage, a GPU-driven Vulkan renderer on `ash`, the editor on Vulkan, `rapier3d`/`kira`, skeletal animation, PBR, mesh shaders and hybrid ray tracing. | Work follows phases R0–R10 with their acceptance criteria and benchmarks. Vulkan stays the only game-rendering API. |
| The 21 November decision paused the 0.5 brief and new UI work while R1, R3 and R4 were in progress. | Bug fixes to accepted 0.4.2 remain allowed. The agreed 0.5 scope is preserved. The [current roadmap](product-roadmap.md#compose) records the updated start conditions. |
| "Not worse than R0.6" and similar criteria follow the [not-worse rule](bench.md#not-worse): the baseline commit and the candidate run interleaved in one session (five runs each), medians are compared with a per-metric tolerance of max(5 %, 2 × measured spread), a borderline result is repeated once, peak memory has a 1 % floor and p99 is tracked only. | `scripts/bench.py --compare` returns the verdict; archived report numbers are not compared directly. |

<a id="ecs-storage-r11-20261122"></a>

## R1.1 storage decision — 22 November 2026

Proceed to R1.2 with hybrid `bevy_ecs` storage. Native values use typed
queries, reflection and lossless project codecs. Schema-defined payloads
use JSON columns. In the matched S3 series, hybrid reached 34.82× and
dynamic JSON reached 4.59× against the 5× gate. The [storage decision](ecs-storage-decision.md#r11)
records the authored-data cases, lifecycle costs, and limits of the spike.
