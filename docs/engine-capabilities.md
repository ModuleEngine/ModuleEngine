# Source capability inventory

[English](engine-capabilities.md) | [Русский](ru/engine-capabilities.md) | [Documentation](README.md)

This is the stage-1 data foundation of the [editor clarity plan](editor-clarity-plan.md#stage-1).
The inventory describes installed capabilities and authored participation. It is
not a module loader, a new executor or a report of
running/ready instances. The [Engine Overview](engine-overview.md) uses this data
for read-only discovery and navigation. Verification and independent-review status are recorded
in [current status](current-status.md).

## Ownership and kinds

`CapabilityMetadata` is source-owned, non-authored metadata: readable `label`,
`purpose`, semantic `owner`, capability `kind`, `source_package`, configuration
navigation hints and optional helper-library references. It is registered
alongside the existing contracts; no fields are added to `GraphDocument`,
`ModuleDescriptor`, `SystemDescriptor`, component schemas or exact lock payloads.

| Owner | Meaning and current examples |
| --- | --- |
| `engine` | Technical infrastructure: world storage, scene extraction, software/Vulkan rendering, scalar/Custom hosts, audio/storage services and general-purpose grid/history/action helpers. |
| `gameplay` | Reusable game mechanisms: ARPG movement/combat/health/AI/event policy and the campaign-progression helper. ARPG collision is its game-specific obstacle resolver, not a general physics service. |
| `project` | Concrete game/example rules and tools: Crate Garden, the original limited-turn Puzzle, the rotation example and legacy motion data. |
| `unknown` | A source has not declared ownership. Namespaces, crate names and native features do not supply a guess. |

Kind is separate from owner: source contract, world storage, renderer, render
pass, executable system, component data, host contribution, library, service or
editor tool. A library is not installed as a fake executable system or graph
node. A host contribution describes registered camera/input/scene/event/service
hooks; its presence does not imply that optional UI callbacks are compiled.

## Actual inventory and configuration sources

| Current capabilities | Source and configuration hints |
| --- | --- |
| `world.ecs`, `world.objects`, `scene.extract` | Core `ModuleRegistry` factories/contracts and source metadata; root graph. The game route still owns canonical ECS; alternate providers do not establish arbitrary world interchangeability. |
| `render.software3d`, `render.rasterize`, `viewport.output` | Existing software/viewport contracts; root/nested preview graph. |
| `scalar.value`, `clock.time`, `interface.input`, `interface.output` | Existing scalar/interface contracts, with root/render graph hints where applicable. |
| `custom.rust`, `custom.rust.update` | Generic compiled Custom hosts; Rust source and the corresponding root/execution graph. The user's code keeps its own ownership. |
| `game.motion` | Legacy example component data; component fields. |
| `render.vulkan`, `gpu.forward`, `gpu.gbuffer`, `gpu.deferred`, `gpu.depth_normal`, `gpu.ssao`, `gpu.tonemap`, `gpu.fxaa`, `gpu.color_encode`, `gpu.custom` | Vulkan source installer, root/render graph and HLSL hints. Descriptions explain each actual pass, including AO and current-frame AA. Registration opens no device. |
| `game.input`, `game.ai`, `game.movement`, `game.collision`, `game.combat`, `game.health`, `game.portals`, `game.audio` | ARPG source installer; execution graph, components and native source. The last entry adapts game events; native audio playback is separate. |
| `puzzle.turn`, `puzzle.result` | Original Puzzle source installer; execution graph and components. Its turn limit is not changed by G1's unlimited policy. |
| `g1.rules` | Crate Garden source installer; execution graph, registered level tool and native source. |
| `example.spin` | Application source-extension installer; execution graph and Rotation component fields. |
| `arpg.presentation`, `puzzle.presentation`, `g1.presentation` | Actual `SystemRegistry` game contributions and separately supplied descriptions. Activation follows their declared schedule modules. |
| `audio.wav`, `persistence.json` | Actual `HostServiceRegistry` factories when their features are installed. Source request/configuration code remains the entry point; a generic audio-settings pane is not implied. |
| `game-kit.actions`, `game-kit.grid`, `game-kit.history`, `game-kit.campaign` | Compile-selected pure game-kit APIs, reported through installed source consumers' declared dependencies. In the current composition G1 declares these four dependencies. Native source/build configuration, not a runtime toggle. |
| `editor.panel.100`, `editor.panel.110` | Actual Editor panel registrations: Participating Modules and Crate Garden Level Palette. Metadata declares purpose/ownership; availability retains owner-activity and Engine-right checks. |

These are installation families, not a promise that every binary contains every
row. Smaller feature compositions omit their source packages, services and
consumer-linked helpers. Framework code such as content import, history, worker
compilation, project validation and built-in editor windows remains documented
by its existing packages/contracts; this inventory does not fabricate optional
runtime nodes for every framework function. Unmapped linked library code is not
claimed as a complete linker inventory.

## State evidence

`CapabilityRecord` keeps `installed`, optional declared `version`, `available`
and `participation` separately. A `CapabilityFact` is an optional Boolean plus
its `evidence`: `null` means unknown, not disabled. There is no `running`,
`compiled` or `ready` claim. Version is absent where the registry supplies no
separately versioned contract, including services, tools and host hooks.

| Kind | Availability and participation evidence |
| --- | --- |
| Module/system | Installed contracts from `CompositionContext`; offered IDs from an Engine profile. Participation comes from `CompositionContext::activity`, including connected graph modules, enabled schema features, scheduled systems and active schema owners. |
| Host contribution | At least one offered activation module; participation follows the contribution's existing authored-schedule rule. `activated_by_any` edges are alternatives, not all-required dependencies. |
| Service | Installed factory and source-declared platform/implementation support, without probing devices/files. Participation means an explicit request from the active contributions' existing pure `service_requests` discovery hooks. |
| Library | A compile-selected helper reported by an installed declaring consumer; participation means a declared dependency of a participating module, not observed function execution. |
| Editor panel/tool | Actual registration; owner participation and existing `engine_only` permission checks determine availability. An active owner does not mean the pane is open or its callback ran. |

The Core view accepts an optional offered-ID set from its caller. The Player
join reads `EngineProfile::from_document` and applies the existing portable
`validate_document` checks before supplying that set. Missing/invalid profile
evidence leaves module availability unknown. This is not native package/SDK
attestation, an editing permission grant or a full project validation result.

Failed activity leaves participation unknown, including service and helper
participation; it does not turn failures into an empty active set. A known empty
offered set differs from missing evidence. Unknown modules and missing service
factories remain visible with diagnostics; undescribed metadata retains unknown
ownership/package instead of guesses. Requested but unavailable
audio can therefore have `participation=true` and `available=false`. On Linux
the existing lack of native WAV playback remains explicit.

Authored participation does not mean that a pass is enabled, the graph is fully
valid, a shader compiled or Play is running. Pass enable state, worker progress,
device readiness and authoring rights remain their own contracts.

## APIs, dependencies and preservation

- `ModuleRegistry::describe_module` and `SystemRegistry::describe_module`
  attach source metadata to installed IDs. Missing owners and conflicting
  descriptions are rejected. Composition mirrors a described system's label and
  purpose without changing its executable descriptor, factory or version.
- `SystemRegistry::describe_contribution` describes actual registered hooks.
- `CompositionContext::capability_inventory` builds the module view. The Player
  `capabilities::capability_inventory` joins profile, contribution, service and
  declared helper evidence. No renderer/system/service factory, tick or UI
  callback is invoked; pure service-request discovery is the existing exception.
- `HostServiceRegistry::capability_inventory` inspects factories and requests
  without resolving assets, reading saves, creating directories or opening audio.
- Editor `module_panel_capabilities` describes actual panel registrations without
  calling tools or allocating new tool state. It is the read-only seam for the
  overview; it does not execute panel callbacks.

Dependency kinds retain their meaning: `active_module` comes from existing
module dependencies, `preceding_system` from the executable descriptor,
`library` from source-declared helper usage and `activated_by_any` from host
activation. Provided schema IDs and statically declared consumed component IDs
are also listed. Per-instance Custom `params.components` affects canonical
activity, not that static descriptor list. Conditional field and schema
requirements remain in their existing contracts; the report is not a flattened
replacement for all validators.

Configuration hints identify tool categories, not guaranteed available buttons.
Engine Overview resolves targets and permissions through existing operations.
Ordinary native source changes still require a build. Unknown third-party
metadata is preserved as unknown instead of being classified by ID prefix.
Inspection does not save, repin, migrate or change the document. No new
dependency, dynamic binary ABI or authored format is introduced.

## Read-only diagnostic example

With the existing configured native toolchain/cache:

```text
cargo run --offline --locked --target x86_64-pc-windows-msvc -p module-engine-player --all-features --example capability_inventory -- <fixture.json> <source-root>
```

The example prints a JSON inventory; it does not write the project or create
the unused diagnostic state root. Choose features for the actual binary being
inspected. Without `--all-features`, omitted packages are not invented. This
source diagnostic does not resolve the Editor's project-package binding or
replace Full/Team/export validation, UI inspection or runtime acceptance.

<a id="stage1-verification"></a>

## Stage-1 verification — 13 November 2026

The six-stage plan is saved; tasks 1.1–1.5 are implemented. The existing checkout
at base HEAD `120910656383e2579c87de1fd2d7119ee1c91ba5` retained the prior twelve
documentation edits and authored data. Detailed logs, source snapshots and the
independent review are retained in
`outputs/reviews/editor-metadata-stage1-20261113/`; the final source-bound review
governs acceptance. No commit or publication is part of this task.

| Actual check | Result and scope |
| --- | --- |
| Windows Core library | 115 passed, including factory-free inspection, unknown versus empty, preservation and third-party ownership/conflict cases. |
| Windows host-services library, all features | 32 passed, 4 ignored; new request/factory inspection cases run, ignored native probes are not credited. |
| Windows Player library, all features | 54 passed; repeated on the final code candidate after the extra platform-availability assertion. |
| Windows ARPG / G1 / game-kit / Puzzle libraries | 49 / 42 / 12 / 15 passed; G1 has 1 ignored probe. These are CPU/package regressions, not new Player gameplay acceptance. |
| Windows Editor panel/tool suite | 13 passed, 1 ignored; actual registrations, callbacks not invoked during inspection, document preservation and existing palette/layout/permission scenarios. |
| Windows Editor source/binding guards | 8 passed, including rejection of omitting either new Core/Player capability source file. The first wrong filter executed zero tests and is excluded; the corrected filter ran the eight tests. |
| Smaller Windows Player metadata builds | 3 passed without optional features; 4 passed with G1 but without native services. Omitted source/helpers stay omitted; requested missing services are explicit. |
| Formatting and strict static checks | `cargo fmt --all -- --check` and workspace/all-target/all-feature Clippy with `-D warnings` pass on native Windows. |
| Existing Ubuntu/WSL native CPU metadata checks | Core 3 and Player `g1,audio,persistence` 4 passed on Rust 1.99.0, `x86_64-unknown-linux-gnu`; the inventory correctly reports absent native WAV support. No Linux window/GPU or real audio output was checked. |

Commands use the existing caches with `--offline --locked` and explicit native
targets. Check selection followed the inspected verification plan. Feature-matrix
runs overlap; these are not counts of distinct tests or a full-workspace run.
The early zero-test filter and initial sandbox-denied WSL probe remain recorded;
the corrected delivery run and permitted existing-WSL checks are separate passes.

The current diagnostic example
`ec0bfed91c4b32cdd0f9872f81488277edd8c42398d85935aa9d9a8630c0d27f`
produced two actual 44-record inventories from the existing G1 and Two Arenas
verification fixtures: 27 engine, 10 gameplay and 7 project-owned capabilities,
no unknown owner or discovery diagnostic. The selected subsets differ correctly:
G1 requests its rules/helpers/storage; ARPG selects its combat/health/AI stack.
Both fixture byte hashes stayed unchanged and no diagnostic state root was
created. This report does not include Editor panels; their two normal registrations
are verified separately by Editor tests.

The independent reviewer did not author the code and inspected the new APIs,
source declarations, original execution/permission paths, complete EN/RU plan
and inventory, preserved baseline and evidence. The reviewed scope has no
confirmed unresolved finding. Final documentation rebinding retains identical
Rust/manifest inputs to the tested code candidate; old snapshots/artifacts keep
their own bindings. This is metadata-foundation acceptance, not an implemented
Overview, a native screenshot, GPU/image correctness, FPS/cold-start evidence or
clean-machine/hardware-Linux certification. At that stage-1 closure stages 2–6
were unstarted; the subsequent [Overview implementation](engine-overview.md#stage2-verification)
has its own UI evidence and review scope.
