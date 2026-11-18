# Engine composition and editor direction

[English](engine-composition-direction.md) | [Русский](ru/engine-composition-direction.md) | [Documentation](README.md)

<a id="status"></a>

## Agreement and status — 13 November 2026

The user accepted this direction after reviewing the difficulty of understanding
the editor and its graphs. First clarify engine/game responsibility; use that
boundary to make the engine-author workflow understandable. The first route is
**understand the engine's composition → choose a module → configure it**.

The current scope is **one configurable ModuleEngine with multiple game
projects**. Different module sets, render paths and settings are configurations
of this engine, not independently managed Engine products. Independent Engine
catalogues, switching and shared-version publication/update workflows are
deferred; they are not prerequisites for the next UI work.

This records product and architecture direction. It does not implement a new
editor, independent Engine document, package selector or universal scheduler.
The conversation's interactive screen is a design proposal, not a screenshot of
implemented UI or a complete specification. Existing runtime, permissions and
source-bound acceptance remain governed by their current contracts.

The long-term proof remains [5–10 different finished games](portfolio-roadmap.md).
A common framework is a realistic target; arbitrary compatibility with every
possible game or third-party architecture is not established by this agreement.

<a id="ownership"></a>

## Responsibility boundaries

| Concept | Responsibility |
| --- | --- |
| Framework | Module contracts, dependency checks, lifecycle, execution, time, data exchange, resource ownership and error handling. |
| Technical engine module | A technical capability such as rendering, audio, input, world storage, animation playback or physical queries. |
| Engine configuration | The selected technical modules, implementations, connections and configuration policies of the one engine; it can also restrict which gameplay packages a delivery permits. |
| Reusable gameplay module | Game-level mechanisms such as combat, health, equipment, progression or campaign rules. Reuse across games does not make them technical engine systems. |
| Concrete game project | Selected gameplay mechanisms, its own rules/code, scenes, assets, behavior, balance and permitted configuration. |

An executable system is a unit of work. A module can supply several systems,
schemas, assets or editor tools. Pure helper libraries, native services and
scheduled systems have different extension contracts; they need not all become
graph nodes. Registration in the same binary, use of `SystemRegistry`, an
existing crate name or availability in an Engine profile does not determine
semantic ownership.

| Technical engine capability | Game-level meaning |
| --- | --- |
| Shape intersections, ray queries and generic movement constraints | Whether an attack hits an enemy, affects allies or inflicts damage. |
| Clip playback, blending and animation events | Attack windup, combos, cancellation and invulnerability rules. |
| Input devices and action mapping | What Attack means and when the player may use it. |
| Versioned storage and reliable publication | What survives death, whether Continue restores a checkpoint or an exact puzzle attempt. |
| Audio playback, gains and spatial placement | Which cue accompanies a hit, a death or victory. |
| Geometry, materials, lighting and GPU passes | A hero's appearance, boss warnings and the visual treatment of loot. |

Combat, inventory, XP, quests, crate-pushing policy and victory conditions belong
to the gameplay layer, even when delivered as reusable packages. Generic
infrastructure must not interpret their concepts. This extends the ownership
explanation in [optional gameplay](optional-gameplay.md); it does not move or
rename existing crates. Core and production Host already have separate package
boundaries; that fact alone does not certify every current integration.

<a id="framework"></a>

## Framework quality

The framework must provide explicit compatibility/dependencies, predictable
execution, isolated instance state and clear resource lifetimes. Errors and
rejected candidates preserve authored work and the last accepted version;
Cancel, Stop and module teardown respect ownership. UI work must retain the
existing authored/Play separation and Full/Team restrictions.

The framework necessarily makes architectural choices. Keep mandatory rules
small and explicit; do not encode a genre's requirements as universal rules.
Unnecessary requirements include networking in an offline game, spatial
transforms for every logical record, physics for moving a UI card, checkpoint
semantics for every save, or a G-buffer for every renderer. Game rules may react
to commands while animation/UI continue updating. Supporting such execution
policies is direction, not a claim that the current fixed-step game session
already supports every policy.

<a id="composition"></a>

## Composition and limits

A module's availability does not activate it. Selection should bring only its
actual dependencies and applicable settings/tools into the corresponding
runtime and delivery. Distinguish installed authoring capabilities from the
selected game's composition and exported native features.

Connections require compatible contracts. Replacing ordinary geometry handling
with a virtualized approach may require new asset preparation and a connected
set of passes. Allow adapters or replacement of that set rather than promising
that every implementation can replace one node without other changes.

Do not split every operation into a module or require dynamic libraries, IPC,
generic messages or format conversion at every system boundary. Module
boundaries should represent independently useful responsibilities and permit
efficient execution. Reuse is demonstrated by real consumers and smaller
compositions, not by the number of nodes or registry entries.

<a id="graphs"></a>

## Graph roles and current limits

The reviewed baseline is `120910656383e2579c87de1fd2d7119ee1c91ba5`:

- `Module Blueprint` serves the retained root route
  `World → scene.extract → renderer → viewport.output`.
- The GPU subgraph describes rendering passes and resource flow.
- `Gameplay Systems` separately describes staged gameplay execution.

These are separate mechanisms. Audio, persistence and package installation also
use other extension contracts. The root graph is not a complete engine
architecture map or a universal execution graph. Its main route is concerned
with producing an image; interpreting it as the whole engine creates confusion.
See [architecture](architecture.md#два-маршрута-исполнения).

An editable graph is appropriate when changing a connection or order changes
meaningful data flow or behavior. Render passes are a clear example. Selecting
an audio service or inspecting installed modules need not require manual node
placement and mandatory wiring. A high-level map must reflect real contracts;
it must not imply execution support that does not exist.

<a id="editor"></a>

## Accepted editor direction

1. **Engine overview is the main entry point.** Show the selected composition,
   each part's purpose, implementation, state, dependencies and configuration
   entry point. Separate technical engine systems from gameplay packages.
2. **An architecture map is an additional view.** Show actual relationships;
   deriving it from the selected composition is the preferred proposal. Exact
   map editing and layout behavior remain to be designed.
3. **Focused editable graphs belong to suitable subsystems.** Open the relevant
   render or execution graph in a clearly identified context. Animation/behavior
   editors discussed for G2 are future work, not existing capabilities here.

At every level, identify the owner, what the user is editing and the scope of a
change. Expand abbreviations such as AO/AA with plain explanations. Distinguish
available, active, pending, applying, failed and ready states using actual
operations. Preserve flexible Engine/Project pane layouts; introducing a new
entry point must not silently replace a saved personal layout.

<a id="engine-project"></a>

## One engine and multiple game projects

Currently, **Engine** and **Project** switch workspaces of one open document.
`GraphDocument` retains Engine configuration/profile and game data together. A
project-local binding locates an installed source/SDK/backend package.
**File → Open Project / Recent Projects** opens another game document. New
Project can inherit the current Engine or choose a prepared profile. Publishing
an Engine profile updates the working document locally; it is not a shared
multi-project update service. See [profiles](engine-profiles.md) and
[workflow](editor-workflow.md).

In the intended UI, **Engine** is the workspace for configuring this engine's
technical composition; **Project** is the workspace for the selected game's
content and rules within its permissions. Show the current game/path, active
configuration and editing rights. Opening another game project is a project
operation, not switching to a different engine. No independent Engine selector
or catalogue is required in the current scope.

Existing project-local configurations, profile/contract versions and installed
package bindings retain their current meaning and checks. This scope change
does not choose one global mutable configuration for all games or make edits
propagate to other projects automatically. It requires no data-format change,
rebinding, migration or arbitrary native code hot replacement.

Ownership and editing permission are separate: a frozen Team schedule can
contain gameplay rules. Classifying them as gameplay does not unlock their
editing. A future permission change needs its own task and verification.

<a id="next-work"></a>

## Priority and future verification

The [saved implementation plan](editor-clarity-plan.md) breaks this
direction into bounded tasks. Stage 1's [capability metadata contract](engine-capabilities.md)
defines its read-only evidence sources; actual completion is tracked in current status.
The new [stage 2A](editor-clarity-plan.md#stage-2a) adopts the native UI after
the completed Overview, before the rendering route. Implementation was authorized
on 15 November; [actual progress and limits](editor-ui-rollout.md) are recorded separately.

Before implementing the new UI, clarify responsibility, overview metadata and
the configuration being edited within the current project. A useful first
authoring route must let the user identify the game and active configuration,
explain their composition, locate the
renderer, understand an AA option/dependency and reach its relevant graph without
confusing it with gameplay execution. Detailed labels, metadata sources and
layout are implementation work still to be specified.

Future changes require checks of actual operations, omitted-module behavior,
permissions, authored-data/layout preservation and affected G1/other consumers.
Use the existing performance criteria when runtime behavior changes; this
documentation adds no benchmark or runtime acceptance result.

<a id="open-decisions"></a>

## Open decisions

Stage 1 supplies the [source metadata and state-evidence contract](engine-capabilities.md)
over the existing registries. That initial metadata-source question is resolved
within its documented scope; future extensions declare their own descriptions.

- What exact overview, map navigation and module-settings layout best supports
  the first route while preserving existing layouts and permissions?

The remaining layout question is also listed in [open questions](questions.md). This
record does not authorize migration, package rebinding or a new implementation stage.

<a id="deferred"></a>

## Deferred beyond the single-engine scope

Managing independent Engine documents/products, choosing between their shared
published versions, adopting cross-project updates and selecting among separate
engine installations are deferred. Their earlier storage/identity questions do
not block the one-engine UI. Ordinary version and compatibility checks for the
current engine remain necessary. The modular architecture and multi-game
portfolio goal remain in scope; no future multi-engine product is required now.
