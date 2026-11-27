# ModuleEngine Product Roadmap

[English](product-roadmap.md) | [Русский](ru/product-roadmap.md) | [Documentation](README.md)

Recorded project roadmap and planning baseline, updated 17 November 2026.

The final demonstration is concrete: you assemble the capabilities you need. A colleague reproduces that configuration from its description and changes a second game. The first game still builds and plays as before. A player receives only what their game needs.

This document records the reviewed sequence and its full acceptance expectations. **The owner closed 0.4.2 on 17 November 2026. On 21 November 2026 the owner started the [engine rebuild track](#rebuild) and paused the 0.5 brief and new UI work until its core phases R1, R3 and R4 are accepted. The 0.5 milestone remains paused on `main`.** See [accepted rebuild stages and available next moves](#rebuild-status). The 0.4.1 implementation and historical 0.4.2 engineering evidence remain recorded below. Future version numbers and milestone scopes are planning labels: they do not mean those versions have shipped, authorize every implementation, or promise dates. Existing specifications, ownership boundaries and budgets remain in force. Use the agreed [0.5 composition and dependency scope](#compose) for the next brief and the [owner check cards](product-roadmap-checks.md) for short personal checks.

[Version map](#map) · [Baseline](#baseline) · [Starting handoff](#start) · [Independent use](#usability) · [Release decisions](#decision) · [Engine rebuild](#rebuild) · [AI-assisted live authoring](#ai-authoring)

<a id="map"></a>

## The complete route

| Version / planning label | New evidence | Direction |
| --- | --- | --- |
| [0.4](#baseline) | Starting point: one engine, working G1 on the current Windows PC and a separate Player. | Existing baseline |
| [0.4.1](#render) | First complete route from ordinary launch to the image in Player, with compact settings and permitted choices in the game menu. | Editor |
| [0.4.2](#image-and-docking) | Image, panes, stabilization and compact chrome; closed by the owner’s decision. | Accepted 17 Nov 2026 |
| [R0–R10](#rebuild) | Engine rebuild track: `bevy_ecs` world, GPU-driven Vulkan renderer, editor on Vulkan, `rapier3d`/`kira`, skeletal animation, PBR, mesh shaders and hybrid ray tracing. | Engine core · proposed 21 Nov 2026 |
| [AI-A–AI-B](#ai-authoring) | Build a game by plain-language requests while it runs; accepted changes are saved as an ordinary project. | Idea · not before R1/R4/R6 accepted |
| [0.5](#compose) | Add, remove or replace a capability without unrelated settings getting in the way. | Paused until R1/R3/R4 |
| [0.6](#pin) | Identical starting configurations for A and B; changing B does not require updating A. | Stability |
| [0.7](#extend) | Two distinct tasks: configure an existing function and write a new one. | Extensions |
| [0.8](#restore) | One entry description, available pinned sources/resources and an understandable refusal. | Handoff |
| [0.9a](#combat) | A playable segment and a second consumer without progression or loot. | G2 foundation |
| [0.9b](#campaign) | Three locations, a complete campaign and independent delivery. | G2 game |
| [1.0](#author) | One bounded, honestly declared product version for authors. | Tool acceptance |
| [1.1](#pixel) | Sprite-built levels without mandatory spatial-scene or ARPG components. | G3 breadth |
| [1.2](#flight) | A spatial course, orientation, camera and collisions. | G4 breadth |
| [1.3](#real) | A game environment with PBR, lighting and controlled cost. | G5 breadth |
| [1.3.x](#portfolio) | One common release and a separate decision for each game; games six to ten only answer a new question. | Portfolio |

The work is not one long serial pipeline. After owner-accepted 0.4.2, the next planned editor milestone is 0.5; work will begin after a separate brief. Preserve three parallel results: understandable composition and update boundaries (0.5 → 0.6); safe standalone G1 delivery on a clean PC (start checking against the existing baseline); and the first G2 combat encounter (the 0.4.1 route and necessary navigation suffice; do not wait for all 0.5–0.8 tools). Combat needs determine which extension in 0.7 comes first. The complete campaign follows acceptance of G2's foundation. Before 1.0, tool readiness, full-environment handoff in 0.8 and real games converge. After 1.0, test breadth; do not postpone the first useful editor until all later games exist.

There are **two finish lines**. 1.0 answers “can a team work independently in the declared configurations?” The 1.3.x result answers “has that been demonstrated by five substantially different finished games on one foundation?” One answer does not establish the other. After 0.4.1, record actual days spent on tooling, game content and fixes before estimating later work. Without those observations, a calendar estimate for G2 or G5 would be speculation.

<a id="baseline"></a>

## Baseline / 0.4 — use what already exists

**Editor.** Stages 1, 2 and 2A, accepted within their recorded scope, provide composition descriptions, Engine Overview and the native UI. Separate scene files, Hierarchy and working layouts exist. Stages 3–6 are not accepted; part of stage 4 was implemented in 2A. Check overlapping requirements instead of repeating a redesign.

**Configuration.** There is one ModuleEngine and several game documents with local settings. Engine/Project tabs are layouts, not game selectors or the source of rights. Full with validated sources can configure architecture; Team receives only changes permitted to the project. Profile versions, exact bindings, Inherit Current, explicit profile publication within the current document and a separate Player already exist. JSON alone cannot install an absent native implementation.

**Games.** G1 Crate Garden has ten finished levels and an implementation accepted on the current Windows PC. Launching a relocated package on that same machine was checked; a clean environment remains a separate condition. Two Arenas is a technical example, not finished G2. The G2 decisions from 6 November remain in force and are [recorded below](#g2-agreement). G3–G5 are proposed ways to demonstrate the agreed diversity.

**Still to prove.** Independent work by an unfamiliar author, restoration of the full environment on a clean machine, coexistence of old and new native implementations, and a common release for five games. Old reports do not establish hardware Linux, physical DPI/input or new FPS results. Mobile-style means a PC game, not a phone port.

Basis: [current status](current-status.md), [G1 evidence and acceptance boundaries](g1-verification-20261101.md), [editor clarity plan](editor-clarity-plan.md), [composition direction](engine-composition-direction.md).

<a id="start"></a>

## Next brief — 0.5 composition and understandable dependencies

**Entry.** The owner closed 0.4.2 on 17 November 2026. V0.5.0 may audit existing behavior before R3 acceptance and the owner's separate go. Implementation of 0.5 and new UI work remain paused until both. The separate brief follows the [agreed 0.5 scope](#compose): current capabilities, examples with and without audio, rights and actual Player-composition evidence.

1. Preserve the working UI and authored data. Reconcile stage 4 with implemented 2A and complete only confirmed gaps instead of repeating a redesign.
2. In Full, show the audio consumer and removal consequences. First refuse clearly while the request is needed; after removing the request, carry the change through UI, runtime and a real Player package. Provide restoration of previous values.
3. Give the owner two identified examples and a short card: attempt to remove a required capability → resolve the dependency → run without the service → restore it. Team receives permitted settings only; technical composition evidence stays separate.
4. The parallel G1 card relocates the reference package to clean Windows with declared drivers. Launch without checkout, Editor or compilers; complete a level, hear sound, exit, continue and check score/progress. On the developer machine, separately change a level wall, save, reopen and export; observe the new wall on the player machine. Without a second PC or agreed clean image, record “not checked”; G1 retains its previous acceptance scope.

This leads into a separate 0.5 brief; it is not implementation authorization or a report of implementation. The parallel G2 foundation retains its agreed entry conditions and need not wait for all tools.

<a id="render"></a>

## 0.4.1 — I change anti-aliasing myself and see the result

Anti-aliasing already exists. If an author cannot understand this route, ten more settings will only increase the problem.

**Entry and dependencies (original 0.4.1 scope).** The 0.4 baseline, Overview and 2A exist. This milestone targets editor-plan tasks 3.1–3.6.

1. **Prepare a recognizable example.** In a separate project, place high-contrast diagonal edges; fix camera and resolution and prepare FXAA/off variants. Produce corresponding Full and Team deliveries. Launch by the same route the owner uses. Obtain two reference frames without changing the scene.
2. **Show where I am and what I can change.** In Full, lead from Engine Overview to the connected renderer and its exact graph. Explain “edge anti-aliasing,” the selected pass and permitted values beside it. In Team, lead through permitted Project settings; do not offer the blocked Overview. Reuse existing commands and support an old saved layout.
3. **Connect choice, graph and image.** One change updates the authored value and actual rendering. Distinguish “the pass exists, effect disabled” from “the pass is absent.” Remove/restore AA or AO in Full without dangling variants or links. Where the operation is one edit, one Undo restores its related data.
4. **Show progress and the way back.** Distinguish draft, Apply, readiness and Save to disk. Explain before the operation whether Stop or rebuild is required. Introduce an incompatible link or HLSL error in a tutorial copy; show its cause, preserve the last accepted frame/version and retain access to a correction route.
5. **Reach the delivered game.** Check Undo/Redo, save, close, reopen and export. Compare the same view in Editor and Player. Check keyboard interaction and a narrow panel without relying on hover help.

**Settings follow-up.** The owner gave positive feedback on settings, quickly found Project Settings and noted excessive permanent text; full human acceptance remains open. Compact sections/fields, help on request and concise statuses are implemented. The author binds their game menu through Custom Rust to a permitted setting by stable ID; choices come from the current configuration. Actual separate Player processes verify FXAA → Off → full exit/restart with Off retained, FXAA in a separate clean profile and unchanged authored/package files. The player's preference is stored separately for that game. The author and player need not know a rendering-pass ID. See the [brief](release-041-plan.md#player-settings) and [scoped engineering evidence](release-041-menu-verification.md); this is not unqualified release acceptance.

**Owner check.** Start with Full and Team copies of “High-contrast edges,” the ordinary shortcut and a verified package. In Team, disable anti-aliasing, compare edges, then enable it again. Save the choice, reopen and launch Player. In Full, remove the pass and restore it with Undo. The change is visible; the choice survives reopening and reaches the game. You can explain disabling versus removal. An error leaves the previous image working.

**Team evidence.** Two comparable actual frames; Full/Team rights; AA/AO removal and restoration; error and reopening. A screenshot of a menu does not close the milestone.

Basis: [editor stages 1–6](editor-clarity-plan.md), [Engine Overview](engine-overview.md), [settings and valid variants](settings.md). Historical scope: [0.4.1 plan](release-041-plan.md).

<a id="image-and-docking"></a>

## 0.4.2 — I trust the image and move panels in one gesture

**Owner decision — 17 November 2026:** “I checked it; everything works and looks fine for now. Let’s close 0.4.2.” The owner had also personally confirmed that jitter disappeared. This is general manual milestone acceptance and grounds to plan 0.5. See the [decision, accepted final03 and automation limits](release-042-ui-density-verification.md#owner-acceptance-20261117). No detailed per-scenario transcript was recorded; mixed-DPI/Linux and final automated captures are not declared completed.

**Original scope and protocol (historical).** Build on 0.4.1 without reopening its history. One milestone has two independently accepted results: image quality and editor gestures. Existing native egui APIs and the narrow Text/Choice script binding are adequate for this scope. A general UI toolkit is a future demand, outside 0.4.2.

1. **Establish the actual image.** Record physical-pixel viewport/render-target dimensions, display scaling and a fixed reference scene. Compare FXAA/off in static views and controlled camera/object motion in Editor and Player. Check scaling/filtering before changing AA. Ghosting is unconfirmed: reproduce and identify its cause before claiming a defect or fix.
2. **Make settings and panel movement direct.** Provide `Edit > Project Settings`. Drag the free area of a tab/header continuously into a native detached window and back to a dock target. Show a visible docking preview, commit on release and cancel with Escape. Retain panel contents, selection, drafts and layout state through detach/re-dock and cancellation.
3. **Accept each result separately.** Bind image/motion evidence and gesture checks to the delivered build; record the owner's decisions for each result. Positive 0.4.1 settings feedback does not establish independent human acceptance of either result.

**Owner check.** Open the reference scene at the declared display scaling, compare FXAA/off while stationary and moving, then open settings through Edit. Without releasing the mouse between steps, pull a panel out by its free tab/header area, move it as a native window and return it to the visible docking preview; release to dock. Repeat with Escape. The image uses the intended physical pixels, the AA comparison is understandable, and the panel returns without losing working state. Record image and gesture acceptance separately.

Original detailed brief: [0.4.2 plan](release-042-plan.md). The protocol above is not a completed per-scenario transcript; the owner’s general acceptance above closes the product milestone.

<a id="compose"></a>

## 0.5 — I assemble what I need and understand the consequences

**Status — implementation and new UI work remain paused. R1 and R4 are accepted. Start only after R3 is accepted and the owner gives a separate go for the 0.5 brief. V0.5.0 may audit existing behavior before that decision.**

Disabling a button while retaining its service, resource requirement and Player dependency does not remove unnecessary engine capabilities.

**Entry and dependencies.** The owner has closed 0.4.2. Preserve the 0.4.1 route. V0.5.0 is preparatory verification of existing behavior and can precede R3 acceptance and the separate decision to start. The separate 0.5 brief follows the scope below. The parallel G1 and G2 checks retain their own entry conditions.

**Plan items (revised 27 November 2026).** `V` marks the version route; effort is a rough guess for one developer. The former items 1 and 2 are replaced by an audit: 2A already covers the window/project split and PR #67 (R4.2–R4.3) already selects linked games.

| ID | Action | Measurable criterion | Depends on | Effort / status |
| --- | --- | --- | --- | --- |
| V0.5.0 | Audit existing behavior: the Open/Recent, hidden-draft, Cancel and error transitions against 2A, and how an author finds input, audio and the other existing functions for the actual composition and rights. | Record the observed routes and confirmed gaps for both parts of the audit. Each failing route has an automated or scripted check. No new UI or 0.5 implementation is part of the audit. | 2A | 2–3 d |
| V0.5.1 | Read-only map “who uses this / what stops working”, built from the Overview data. | For the two reference projects (with and without audio) the map matches the actual service requests; test. | R4.3 | Paused · 1 w |
| V0.5.2 | Refuse removal of a needed capability with the consumer named; after the request is removed, apply as a whole with Undo; inactivation keeps values. | First attempt changes no file; Undo restores the previous values exactly; Full only. | V0.5.1 | Paused · 1–1.5 w |
| V0.5.3 | Minimal package without an audio request. | The export has no operative audio settings, no service start, no optional audio feature and no unneeded WAV; checked in CI. | V0.5.2, R6.3 | Paused · 3–5 d |
| V0.5.4 | Run the parallel [G1 clean-Windows card](product-roadmap-checks.md#g1--player--clean-windows). | Pass only when the delivered Player runs on clean Windows without the checkout, Editor or compiler, a level is completed with the expected sound, and progress is restored on relaunch after exit. Verify the changed wall from the card. “Not checked” leaves this result and acceptance that depends on it open. | none | Paused · 2–3 d |

Do the audio part after R6.3 (`kira`): the tested audio scenario on branch `c199903` uses the current WinMM mixer, so doing it earlier means rewriting it. The numbered scope below stays as the description of intent.

1. **Remove confusion between windows and projects.** Compare 4.1–4.5 with implemented 2A: Scene edits the authored world, Game runs a copy, changing layouts does not change project or rights. Complete only failing transitions: Open/Recent, hidden drafts, Cancel and errors opening another project.
2. **Give each existing function one understandable home.** For input, show available actions and their game bindings; for audio, output and game-event-to-WAV bindings; for world, import and game packages, their real tools. A game author sees task-relevant capabilities by default. Full separately exposes installed but unused capabilities. Label unknown state explicitly.
3. **Explain dependencies before a change.** Use two real configurations: one project requests playable audio, another does not. Distinguish resource, service request, installed implementation and build feature. Build a map only for “who uses this?” and “what stops working?”, using the same data as Overview. The initial map is read-only.
4. **Carry addition, removal and replacement through.** Offer only variants compatible with the location and rights. First show affected components, graphs and data. If a module is still needed, name its consumer and offer dependency removal or Cancel. Inactivation preserves authored values. Apply a successful change as a whole, with Undo. Composition and rights changes require Full.
5. **Check a genuinely minimal package.** For a tutorial configuration with no audio request, check UI, runtime and export: no operative game audio settings, no service startup, no corresponding optional native feature, and no unnecessary WAV where the packager can establish that reliably. An archive of inactive data is not an active dependency.

**Owner check.** Start with delivered short scenes with and without an audio signal, using a genuinely optional capability. In Full, find the signal's consumer. Try removing the capability while it is needed; then remove the request and retry. Open in Team, export and launch, then restore the old composition. The first attempt explains the obstacle without damage. The second produces a working game without the capability and its unnecessary settings. Restoration recovers the signal and its previous parameters.

**Team evidence.** Composition reports for both Players, dependencies, actual service requests and preserved data. Follow the map to the implementation without activating a disabled capability.

Basis: [editor plan](editor-clarity-plan.md), [engine/game boundaries](engine-composition-direction.md), [profiles, pins and Player](engine-profiles.md), [assets and dependencies](asset-workspace-api.md).

<a id="pin"></a>

## 0.6 — I pin a working foundation and change only game B

This first directly demonstrates the freedom to decline an update. A copied JSON document or an old prebuilt Player does not establish it.

**Entry and dependencies.** Use compatible, verified variants from 0.4.1/0.5. First change configuration within one installed version. Native-code replacement is a separate 0.8 experiment.

**Plan items (revised 27 November 2026).** Much of the mechanism already exists: format 4 stores `engine_lock` and `module_versions`, **Publish Engine profile** is explicit, `--migrate-copy` never overwrites, and Open/Save/Play/export never update silently ([engine profiles](engine-profiles.md)). The open work is the end-to-end A/B proof.

| ID | Action | Measurable criterion | Depends on | Effort / status |
| --- | --- | --- | --- | --- |
| V0.6.1 | Passport = existing `engine_lock` plus a view of what is saved and what changed only in memory. | One document; no second version database; test. | R1 | 2 d |
| V0.6.2 | Script two independent projects A and B with separate scene folders and test saves. | Record the composition, reference gameplay frame and scenario for both games. Record hashes of A's authored files as supplemental evidence for the A/B check. | V0.6.1 | 4–5 d |
| V0.6.3 | Apply and save a compatible rendering change to B only, then roll B back. | After changing B, reopen, rebuild and launch A without a migration. Verify A's composition, reference gameplay frame and scenario against its baseline. Then roll back B and launch and verify both games. B recovers its frame and controlled assets, and A's composition, frame and scenario and both games' progress remain as expected. A's authored-file hashes supplement these checks. | V0.6.2 | 1 w |
| V0.6.4 | Refusal and write-failure cases on a copy. | No partial activation; last working point preserved. | V0.6.3 | 3–4 d |

1. **Record a known working point.** Its passport contains engine version/sources, profile and exact module/schema/SDK versions, settings, graphs, assets, tools, build target and results. Reuse existing `engine_lock`, bindings, manifests and checks instead of creating a second version database. Explain what is saved and what has only changed in memory.
2. **Create two independent projects.** Use existing inheritance to create A and B from one configuration on a supported compatible template. Use separate authored-scene folders and test player-save profiles; establish separation from actual paths. Preserve reference states for both games.
3. **Offer a change only to B.** Choose a compatible rendering change; show “this project,” graph differences, affected settings/assets and required rebuilds. Project changes, local-foundation publication and Save are distinct actions. Distribute nothing to other projects. Opening an old game does not update it.
4. **Check refusal and rollback.** Preserve the known working point before Apply. Interrupt before publication; try an incompatible candidate and a write failure on a copy. After a successful change, roll back B including controlled scenes/assets. Treat player progress separately: compatible, restored from a copy, or rollback refused with a reason. Never silently roll back other people's files.
5. **Build both games independently.** Reopen and rebuild A after B without silent migration. Compare its sources, composition, reference gameplay frame and scenario. Show B changed and subsequently restored. Verify equivalent behavior; do not assume bit-identical rebuilt EXEs.

**Owner check.** Start with two identified projects A/B, one recognizable scene and saved references. In B, replace the tone mapper with a prepared compatible variant and save. Open A from Recent, rebuild and launch. Return to B and restore the old variant. Only B changed appearance; A opens/builds without an update. B's rollback restores its image without touching A or unrelated progress.

**Team evidence.** Before/after authored hashes of A, selected modules, frames and both delivered launches. Error/Cancel in B preserves its last working point. This does not yet demonstrate coexistence of two native-module versions.

Basis: [profiles and pins](engine-profiles.md), [scene sources](scene-files.md), [engine/game boundaries](engine-composition-direction.md), [Full/Team delivery](distribution.md).

<a id="extend"></a>

## 0.7 — another developer adds their own capability

The owner needs control beyond a catalogue of existing buttons. An external author tests that control without having to guess where the demonstration game is embedded.

**Entry and dependencies.** Clear selection from 0.5 and a way to pin the starting variant. Work can begin with G2's first real consumer while environment restoration proceeds.

**Plan items (revised 27 November 2026).** Samples such as `gpu-custom-raster`, `gpu-custom-compute` and `crate-garden` and [extending](extending.md) exist, but there is no one-page route, no registration-point inventory and no handoff to an outside author.

| ID | Action | Measurable criterion | Depends on | Effort / status |
| --- | --- | --- | --- | --- |
| V0.7.1 | Two reproducible samples (HLSL post effect, Rust module) with a one-page “place → build → disable”. | An unfamiliar programmer builds and disables each from the page alone. | R2.5 | 1 w |
| V0.7.2 | Minimum extension contract and registration-point inventory. | Document plus a test that Core and shared Host contain no game names. | V0.7.1 | 3–4 d |
| V0.7.3 | Whole route and replacement: compatible and incompatible pass. | Compatible keeps neighbours and parameters; incompatible names the input and keeps the old result. | V0.7.2 | 1–1.5 w |
| V0.7.4 | Deep replacement of Deferred lighting evaluation. | Stepped lighting with geometry and output preserved. | V0.7.3, R5.2 | 1 w |

1. **Provide two short reproducible samples.** For a technical artist, a custom HLSL pass with a visible difference, such as posterization. To prove a deeper replacement, use one controlled chain: replace Deferred lighting evaluation from geometry data with stepped lighting while preserving geometry and output. If an input contract is missing, record and extend that specific seam; a post-effect alone does not close this check. For a programmer, provide a small independent Rust module with a parameter and observable reaction. Supply sources, sample data, SDK version and one page: “where to put it → how to build → how to disable.”
2. **Describe the minimum extension contract.** Each sample names owner, inputs/outputs, dependencies, data version, errors and preservation on disabling. Distinguish a library, service and executable system. Show allowed registration/build points for a native module; JSON cannot install absent code. UI indicates when rebuild is needed and when a candidate may be applied without restart.
3. **Connect the whole route.** Register the implementation, provide its description/parameter, expose it in the appropriate editor context, execute, save and include it in the selected Player. Connect any needed editing tool through the existing contract; do not add a game-name condition to Core.
4. **Demonstrate replacement with an understandable boundary.** Replace the active pass with an author-created compatible pass, then separately supply an incompatible resource type. Preserve neighboring links and parameters using a known conversion in the first case. Name the incompatible input and preserve the old result in the second. A different geometry-preparation method may require replacing a whole region; do not promise universal replacement of one node.
5. **Hand samples to someone who did not write the SDK.** They build the extension in Full, give a colleague a Team project with one allowed parameter, then exclude the extension from another Player. Repair the diagnostics/sample gaps where they stopped; check the corrected route with another participant.

**Owner check.** Start with a clean tutorial-project copy, full sources/SDK, extension-author instructions and Team delivery. An unfamiliar programmer adds the sample and changes its behavior from the instructions. You receive the allowed parameter, change appearance and launch without editing Rust. The difference reaches Player; a second project does not require the module. An error in the custom pass preserves the working game. Both roles understand ownership.

**Team evidence.** Artifacts with/without the extension, sample sources, exact SDK pin, updated registration-point inventory and no game-specific branches in Core/shared Host. These extensions are trusted; this is not a plugin marketplace.

Basis: [source extensions](extending.md), [GPU graph and HLSL](gpu-hlsl.md), [profiles/pins](engine-profiles.md), [G1 engine assessment](g1-engine-assessment.md).

<a id="restore"></a>

## 0.8 — a colleague restores the editor; the old game still builds

Delivering Player is simpler than restoring an author's environment. This milestone separately tests both operations and replacement of the native implementation itself.

**Entry and dependencies.** A/B passports from 0.6, a verified module and its build boundaries from 0.7. Begin the first clean-machine trial earlier against 0.4.

**Plan items (revised 27 November 2026).** `scripts/bootstrap.ps1` already checks (doctor) and restores pinned Rust and DXC with hashes ([toolchain](toolchain.md)). There is no configuration archive, no launcher and no check of MSVC, SDK or driver.

| ID | Action | Measurable criterion | Depends on | Effort / status |
| --- | --- | --- | --- | --- |
| V0.8.1 | Configuration description: folder or archive with relative paths, sources, packages, assets, tools. | Restores on a copy without the original checkout; missing items are listed first. | V0.6.1 | 1 w |
| V0.8.2 | Launcher or ordinary-entry restore with MSVC, SDK, driver checks; never “latest”. | Concrete correction and retry; working install intact. | V0.8.1 | 1 w |
| V0.8.3 | Handoff to another developer on a clean Windows machine. | Colleague restores, edits, builds, delivers Team and Player; second launch needs no author. | V0.8.2 | 3–4 d + PC |
| V0.8.4 | Experiment: native-code change v1→v2 for B only. | Verified copy conversion or refusal before publication; A stays on v1; rollback works. | V0.7.3, R1.7 | 1–1.5 w |

1. **Define the configuration description's promise.** One entry points to preserved complete sources, exact packages/SDK/backend, assets, tools and build rules. Separate included material, material obtainable from pinned links and OS prerequisites. The first format is a verified folder/archive with relative paths, without an extension server. Old dependencies must remain available: a hash does not replace their bytes.
2. **Make restoration understandable.** From ordinary editor entry or a small launcher, choose the description; show found/missing inputs before changing anything. Reuse existing `doctor` and `bootstrap -Restore` for pinned tools. Separately check MSVC/Windows SDK, driver, permissions and required file/network access. Provide a concrete correction and retry without damaging the working installation. Do not silently download “latest.”
3. **Hand off to another author.** On clean declared Windows without the original checkout, another developer receives the description, matching packages/assets and short instructions. They restore Full, open a known project, change it, build the module, check Rust/HLSL in the applicable configuration, and deliver Team and Player. The second launch needs no engine author. In a separate player environment, run only Player without compilers.
4. **Test an actual native-code change.** Keep complete v1/v2 sets available at distinct paths; A remains bound to v1. Update only isolated B: compare implementations, component structures, system-state version, assets, SDK and old saves. Either perform explicit verified conversion of a copy or refuse understandably before publication. Publish B/v2 only after a successful launch. Select a behavior change visible in the game.
5. **Practice failure and version recovery.** Remove a mandatory archive, provide a wrong hash and supply an incompatible save using test data. Do not partially activate the new version. Restore B/v1 from its complete set, including required scenes/data and the agreed player-save variant. Rebuild A/v1 and B/v1. Windows is the first proven experiment target; hardware Linux and its prerequisites are checked separately.

**Owner check.** Start with a second clean PC or agreed clean image with supported graphics, the configuration, both version archives and test saves. Do not use the user's project. Give the description/materials to a colleague; ask them to prepare the editor, make a visible edit and return the game. Separately move only B from native v1 to v2 and back; repeat with a missing archive. The colleague can continue development; the player needs only Player. A keeps building on v1, B rolls back with dependencies, and a missing file is named while the previous result remains available.

**Team evidence.** Clean-install protocol with declared prerequisites, input provenance/versions, independent A/B builds/launches and old/new saves. Compare agreed behavior and composition; do not demand bit-identical builds across OSes.

Basis: [tool restoration](toolchain.md), [Full/Team delivery](distribution.md), [profiles/pins](engine-profiles.md), [scene files](scene-files.md), [services and save versions](../crates/host-services/README.md).

<a id="combat"></a>

## 0.9a — one complete encounter proves the modules useful

Otherwise the editor could be polished for months before discovering that a real character cannot be imported or enemy behavior cannot be configured.

**Entry and dependencies.** Start after the complete 0.4.1 route and necessary navigation fixes. Use 0.5–0.8 mechanisms as they become available; do not wait for every tool before the first encounter. This checks G2's first goal without changing its brief.

**Plan items (revised 27 November 2026).** What exists: ARPG has `chase` and `boss` AI, melee combat, health, portals and audio events (`gameplay-arpg`, Two Arenas content), movement/collision on a horizontal plane, Rapier 3D physics (R6.1) and glTF skin/clip import with CPU playback (R7.1). Not in code: dash, projectile, a behavior graph editor, character animation in the game. The animation graph is tracked as R7.4 and weapon attachment as R7.5, not repeated here.

| ID | Action | Measurable criterion | Depends on | Effort / status |
| --- | --- | --- | --- | --- |
| V0.9a.1 | One real episode: arena, hero, one opponent, obstacle, checkpoint, with a redistributable model and clips. | Hero skeleton and clips play in the gameplay camera; asset licenses recorded; readability agreed from the game camera. | R7.1, R7.5 | 1 w |
| V0.9a.2 | Execution foundations: dash and projectile with independent cooldowns on the existing melee; movement and collision through Rapier. | Tests for miss, hit, wall and stop; the technical module reports events and combat decides damage. | R6.2, R7.3 | 1.5–2 w |
| V0.9a.3 | Behavior graph with a text source and visible active state, edited during paused Play. | Both animation and behavior graphs execute in gameplay. While Play is paused, edit an animation transition and an enemy condition, then resume and check the changes and Undo. Compatible state is kept, incompatible changes offer reset, and errors keep the accepted version. | R7.4, R1.5 | 1–1.5 w |
| V0.9a.4 | Close the loop through Player: telegraph, outcome, death, checkpoint return, early 20-enemy load. | S2 frame p95 [not worse](bench.md#not-worse) than R0.6 with animated enemies; a tester plays without schema help. | R6.3, R7.2 | 1 w |
| V0.9a.5 | Second independent composition without XP or loot. | Same combat in a second project, no Core/Host change; G1 regressions pass. | V0.5.2, V0.9a.2 | 3–5 d |

### Ordered work

1. **Take one real episode and assets.** A small arena, hero, one opponent, obstacle and checkpoint. Select a model/clips distributable with the source project; define skeleton, import, pose transition, hit event and weapon attachment. Agree readability from the gameplay camera, not merely the importer preview.
2. **Build execution foundations on that episode.** Movement/collision, melee hit, interruptible action, dash and projectile with independent cooldowns. The technical module plays the clip and reports an event; combat mechanics decide who takes damage; values and appearance belong to the game. Check miss, hit, wall and stopping.
3. **Let the author control the actual encounter.** Animation and behavior graphs each have a text source and visible active state. During paused Play, change a transition/condition. Preserve compatible state, offer explicit reset for incompatibility, retain the accepted version on error. A static mockup does not count. The animation graph is tracked as [R7.4](#rebuild); the behavior graph stays here.
4. **Close the loop through Player.** Audio/visual telegraph, combat outcome, death and checkpoint return. Build Player and let someone play without explaining its schemas. Compare the trial encounter with a 20-active-enemy load early, before multiplying content.
5. **Obtain a second independent composition.** The same shared combat works in a second project with fixed stats and no XP/loot systems, mandatory fields or dependencies. Finish and verify required reusable first-goal G2 contracts before full campaign production. Run affected G1 regressions.

**Owner check.** Start with two test projects and the arena Player; one project has no progression. Fight, dodge and reach a checkpoint. As author, change one animation transition and one enemy condition, observe effects and restore them. Run the same encounter in the second composition. Behavior visibly changes and returns after reversal. The second game needs neither XP/loot nor changes to Core/Host game rules.

**Team evidence.** Real skeleton import/frame, executable graphs/text, package/save, second consumer, original G2 load and G1 regressions. A new shared module has a consumer, not merely a description.

Basis: [portfolio/game gates](portfolio-roadmap.md), [G1 assessment](g1-engine-assessment.md), the preserved G2 decisions above.

<a id="campaign"></a>

## 0.9b — the author builds complete G2 on the verified foundation

One encounter shows how to extend the engine. Only a full game demonstrates that editor, content and saves withstand production.

**Entry and dependencies.** G2's second goal starts after its accepted first-goal foundation. 0.9a is the first complete example, not a substitute for the remaining required foundation.

**Plan items (revised 27 November 2026).** None of progression exists in code yet: no experience, pickups, inventory or slots, no three-location content, no save-slot route for G2. G2's 27 preserved decisions stay the source of detail.

| ID | Action | Measurable criterion | Depends on | Effort / status |
| --- | --- | --- | --- | --- |
| V0.9b.1 | Three locations, melee/ranged/heavy enemies, boss with three attacks and two phases, set up in the project. | Each element authored through project tools, not hardcoded; boss phases covered by tests. | V0.9a.1, V0.9a.2, V0.9a.3, V0.9a.4, V0.9a.5 | 2–3 w |
| V0.9b.2 | Progression: experience, level-up choice, pickups, bonuses, weapon/armor/amulet slots. | Data → UI → save route works; inventory pauses the game; equipment visible. | V0.9b.1 | 2 w |
| V0.9b.3 | User journey and saves: hints, start, victory, replay, checkpoint restart. | Death and exit keep XP and items; only the unfinished encounter repeats; playthrough measured at 20–30 min. | V0.9b.2 | 1.5 w |
| V0.9b.4 | Delivery and cost. | Mean and p95 distinguishable-frame interval ≤16.667 ms at 1080p with 20 enemies; edit reaches relocated Player; G1 checks rerun. | V0.9b.3, V0.5.4 | 1 w |

1. **Expand the agreed content.** Three sequential locations; melee, ranged and heavy enemies; a boss with three attacks/two phases. Configure exact behavior in the project using the new tools. Sword and two-handed hammer differ in tempo. Dash interrupts attacks/projectile preparation and is blocked by walls.
2. **Connect progression and combat.** Experience, level-up/upgrade choice, ground items/click pickup, one or two random bonuses and three slots: weapon, armor, amulet. Weapon/armor changes are visible. Inventory and upgrade choice pause the game. Carry the route from data through UI and saving.
3. **Finish the user journey.** Control hints, start, three locations, victory and replay. Death/exit preserve XP/items; the next launch starts at the checkpoint and repeats only the unfinished encounter. Use permitted asset licenses and honor the agreed Diablo III visual direction within the chosen content. Measure the 20–30-minute active-play target through an actual playthrough.
4. **Check delivery and cost.** Play the full campaign; change one parameter/content item in the ordinary editor, save and observe it in relocated Player. On the agreed PC/content, check mean and p95 distinguishable-frame intervals ≤16.667 ms at 1920×1080 with 20 active enemies. Callback frequency is not visible FPS. Follow the common clean-delivery procedure and rerun affected G1 checks.

**Owner check.** Start with the G2 candidate, player instructions, a checkpoint save, content-editing tools and Player package. Complete the locations, change weapon, die in the current encounter, close and continue. Defeat the boss. Change an enemy parameter in Editor, save and check the exported game. Progress/equipment survive; the correct encounter restarts. Edits reach Player. Full playthrough/load results belong to G2 itself, not old Two Arenas.

**Team evidence.** Campaign and both authored graphs; all 27 preserved brief decisions; asset redistribution; saves; measurement; clean delivery; G1 checks and independent acceptance. Do not add multiplayer or new classes by inertia.

Basis: [G2 decisions](#g2-agreement), [portfolio/game gates](portfolio-roadmap.md), [verification boundaries](verification.md).

<a id="author"></a>

## 1.0 — the owner and new developers work without accompaniment

“Someone explained it and I repeated it” does not establish an understandable product. Catalogue size and test counts do not measure independence either.

**Entry and dependencies.** Three lines converge: understandable configuration (0.4.1–0.7), handoff/two versions (0.8), and accepted game consumers G1/G2 (0.9). This is the planned 1.0 product boundary, not present implementation status.

**Plan items (revised 27 November 2026).** 1.0 is the convergence point; its items are mostly human work and do not scale with development speed.

| ID | Action | Measurable criterion | Depends on | Effort / status |
| --- | --- | --- | --- | --- |
| V1.0.1 | Assemble the handoff candidate: shortcut, EN/RU docs, neutral example, G1/G2, restoration description, previous stable set, known limits. | Real shortcut, Full, Team and Player checked, not only a build EXE. | V0.8.1, V0.9b.4 | 1 w |
| V1.0.2 | Independent attempts: owner plus four developers (two game authors, two extension authors), goals without button instructions. | On the shared U1–U3 tasks, the owner and at least 3 of 4 developers pass without observer hints. The mandatory role operations pass for 2 of 2 participants in each role. A separate recipient runs U6 in the clean environment under the [original protocol](#usability). Record stopping points. Data safety, A/B isolation and Team restrictions apply to every attempt. | V1.0.1 | 2 w (people) |
| V1.0.3 | Fix stopping points; repeat revised routes with a new participant. | Each fixed route passes with a new person in that role. | V1.0.2 | 1–2 w |
| V1.0.4 | Release scope: clean Windows G1/G2 delivery, update and rollback, budgets. | No data loss or rights violation. Existing mandatory gameplay and startup budgets apply. Compare metrics measured in R0.6 under the [not-worse rule](bench.md#not-worse). R0.6 has no open-to-first-frame measurement. Use a comparable pair under the R3.5 method with the same start and end events. V1.0.4 stays open without that pair. Verify every declared platform and record the boundaries for undeclared ones. | V1.0.3, V0.8.3 | 1 w |

1. **Assemble a handoff candidate.** Ordinary launch, public EN/RU documentation, a neutral example, G1/G2, restoration description, previous stable set and known limitations. Check the real shortcut, Full, Team and Player, not only an EXE in a build directory.
2. **Give people goals, not button instructions.** The owner plus four developers not previously involved in implementation: two game authors and two extension authors. Each has legitimate rights and delivered help, without observer hints or a specially highlighted route. Start with a shared task, then their role-specific task.
3. **Fix stopping points.** Record expected result, attempted actions and where the participant stopped. First correct the model, label, feedback or refusal; do not treat every problem with a new window or longer instructions. Check substantially revised routes with a new participant in the corresponding role.
4. **Release a defined scope.** Verify clean Windows G1/G2 delivery, author package/frozen set, update/rollback procedures and previous gameplay/startup budgets. Do not close with data loss, rights violations or an unreproducible mandatory route. Mark undeclared-platform boundaries; run real checks for every declared platform.

**Owner check.** Start with the 1.0 candidate, role cards, authors unfamiliar with internals and a separate clean recipient machine. Give a colleague the goal “change this game's appearance, leave the second game alone and send me the result” without naming buttons. Receive/check the game and ask them to restore the original. Ordinary entry and delivered help suffice under the [protocol below](#usability). The player needs no authoring tools. Mandatory unchecked routes are not declared successful.

**Team evidence.** Independent-attempt/difficulty records, repeat after fixes, candidate passport, independent report for that snapshot and traceable known limits. 1.0 acceptance does not establish every genre.

Basis: [independent review](review.md), [portfolio gates](portfolio-roadmap.md), [independent-use protocol](#usability).

<a id="pixel"></a>

## 1.1 — I make a real pixel-art 2D game

Flat mechanics in a 3D diorama do not establish convenient 2D level authoring.

**Entry and dependencies.** Proposed first new profile after G1/G2. The genre is a candidate; replacing it must preserve complete 2D/pixel-art coverage.

**Plan items (revised 27 November 2026).** Nothing for 2D exists: no sprite, atlas or tilemap code, and the R track has no 2D renderer row; Rapier is built with `dim3` only. This is new work, not covered by R2.

| ID | Action | Measurable criterion | Depends on | Effort / status |
| --- | --- | --- | --- | --- |
| V1.1.1 | Agree one scene and limits: camera, pixel scaling, target resolutions, budgets. | Written agreement before production. | none | 2–3 d |
| V1.1.2 | 2D pipeline: sprite/atlas import, frames, alpha, layers, 2D camera, tile/level data and collisions (decide Rapier 2D or grid rules). | Author builds a section in Editor, changes a tile and sprite and sees it in game; no ARPG or 3D fields required. | R2.4, V0.5.2 | 3–4 w |
| V1.1.3 | Finish game: controls, traversal, checkpoints, ending, replay, clean export. | Scaling captures match the agreement; older projects pass regressions. | V1.1.2 | 2 w |

1. **Agree one scene and limits.** For a platformer, choose a short level with a jump, platform and checkpoint. Before production, fix camera, pixel-scaling rules, target resolutions and budgets on the declared PC.
2. **Derive needed tools from content.** Import sprite/atlas; define frames, transparency, layers, camera and required collisions. Let the author draw/assemble a section in Editor, change a tile/sprite and check it in-game. Do not require ARPG or unnecessary 3D fields from a 2D author.
3. **Finish the game and check return.** Complete controls, traversal, checkpoints, ending and replay; export to a clean machine. Capture selected scales, check edges/jump predictability, compare composition with G2 and recheck older projects affected by changes.

**Owner check.** Start with an empty compatible 2D template, atlas and short G3 level. Place a platform, replace a sprite frame, save, traverse in Player and respawn at the checkpoint. The platform/new frame are visible; scaling follows agreement; the game is completable without 3D/combat settings.

**Team evidence.** Real 2D game/export; excluded capabilities in build and UI; first authoring route; its own measured budgets and regressions.

Basis: [portfolio gates](portfolio-roadmap.md), [current import capabilities](content.md).

<a id="flight"></a>

## 1.2 — I build a game where height matters

3D models in a planar ARPG do not establish spatial movement or configuration without game-specific hardcoding.

**Entry and dependencies.** Next proposed game after G3; reuse import, UI, input and packaging. The course determines the scope of general physics.

**Plan items (revised 27 November 2026).** Rapier 3D with raycasts and shape queries exists (R6.1, done), so the general 3D contact work is no longer part of 1.2; camera and orientation remain.

| ID | Action | Measurable criterion | Depends on | Effort / status |
| --- | --- | --- | --- | --- |
| V1.2.1 | Define one demanding section: two heights, a turn and an obstacle, with acceptable deviations. | Written scene and tolerances. | none | 2–3 d |
| V1.2.2 | Spatial movement, orientation and camera on Rapier queries; gates repositionable in Scene. | Moving a gate upward changes the route; collisions and camera match position. | R6.1, R6.2 | 1–1.5 w |
| V1.2.3 | Close the loop: course, checkpoints, time/score, replay, clean export. | Old route fails, new route passes; own budget measured; G1–G3 regressions pass. | V1.2.2 | 1.5–2 w |

1. **Build one demanding section.** Two spans at different heights, a turn and an obstacle. Define movement in height/depth, orientation and camera behavior; fix the scene and acceptable deviations beforehand.
2. **Generalize only necessary contracts.** Add missing 3D collision queries/camera settings. Flight, gate rules and score belong to the project or reusable gameplay mechanics. Let authors reposition gates and change the route in Scene.
3. **Close the loop.** Course, checkpoints, time/score result and replay. Export the changed route from Editor, complete it on a clean machine, measure its own budget and check shared changes against G1–G3.

**Owner check.** Start with the delivered course and openable G4 project. Move one gate upward; try flying at its old height, then climb. Finish and restart. The old route does not count; the new route does. Camera/collisions match position and Player reproduces the result.

**Team evidence.** Full completion/replay, delivery, measurement and demonstrated spatial mechanics. A general rigid-body engine alone is not the criterion.

Basis: [portfolio gates](portfolio-roadmap.md), [scene authoring](scene-files.md).

<a id="real"></a>

## 1.3 — I obtain the agreed realistic image in a game

A PBR switch or one attractive frame does not establish that the scene can be edited, delivered and played through.

**Entry and dependencies.** G5 is a proposed compact first-person exploration story. Agree visual references, content, hardware, frame/resource budgets in advance.

**Plan items (revised 27 November 2026).** Materials, lights, shadows, IBL, HDR and the editor workflow are R5.1–R5.6; R5.6's owner check is this milestone's owner check, so 1.3 here covers only the game around them. Today only base color is imported and rendered.

| ID | Action | Measurable criterion | Depends on | Effort / status |
| --- | --- | --- | --- | --- |
| V1.3.1 | Fix references, a small section and comparison conditions (exposure, lighting, viewpoint, resolution, memory, frame time). | Reference set and budgets recorded before production. | none | 3–4 d |
| V1.3.2 | Build the G5 section with the R5 material and light path; changes through Editor. | Roughness change visible in Editor and Player under identical lighting. | R5.1–R5.6 | 1–1.5 w |
| V1.3.3 | Moving quality and the finished game. | Reference points, filtering and flicker, frame time and memory within agreed budgets; G3 does not require G5 maps. | V1.3.2 | 2 w |

1. **Fix references and a small section.** One room/street, appropriate licenses, real asset dimensions, target frame and interaction task. Separately define comparison conditions: exposure, lighting, viewpoint, resolution, memory and frame time.
2. **Carry materials from author to screen.** Import required maps, including normal/roughness/metallic according to the reference. Implement actual shader reads, filtering/mipmaps, required lighting/shadows. Let the author change material/light through Editor and verify the visible Player result. Add a subsystem only if the trial scene cannot meet its criterion.
3. **Check moving quality and finish the game.** Traverse goals, interactions and ending. Compare reference points, flicker/filtering in motion, frame time and memory. Separately check previous inexpensive profiles: G5 improvements do not become their mandatory assets/settings.

**Owner check.** Start with previously accepted references under identical lighting/viewpoint, the G5 project and clean game delivery. Change one object's roughness, observe it in light, follow the route and reach the ending. Open the old pixel project. The difference matches the material, the game matches visual agreement/budget, and G3 does not require G5's maps.

**Team evidence.** Finished interaction, comparison against pre-agreed references/budget on target hardware, separate export and regressions.

Basis: [portfolio gates](portfolio-roadmap.md), [current import capabilities](content.md).

<a id="portfolio"></a>

## 1.3.x — five different games demonstrate one foundation

Five games on five incomparable forks prove less than five games on one foundation with different selected configurations.

**Entry and dependencies.** All five small games are functionally complete. Portfolio acceptance is a separate decision after G5, not an automatic consequence of version 1.3.

**Plan items (revised 27 November 2026).**

| ID | Action | Measurable criterion | Depends on | Effort / status |
| --- | --- | --- | --- | --- |
| V1.3.x.1 | Choose a common release; verified migration of copies for each game. | Old pinned instances still work; refusal leaves a cell open and changes nothing. | V0.6.4, V0.8.4 | 1 w |
| V1.3.x.2 | Five deliveries on that release. | Per game: reused modules, alternate composition, edit route, saves, target hardware, budget, clean launch. | V1.1.3, V1.2.3, V1.3.3, V1.3.x.1 | 2–3 w |
| V1.3.x.3 | Coverage check and decision. | Five accepted games across 2D, 2.5D, spatial 3D, pixel art, stylization, realism; owner decides to stop or pick one question. | V1.3.x.2 | 2–3 d |
| V1.3.x.4 | G6–G10 only for a new question; not planned. | Each needs a named question first. | V1.3.x.3 | not scheduled |

1. **Choose a common version and accept transitions only in verification copies.** Old pinned instances/packages stay usable. For each game, explicitly accept a compatible common release or perform a verified migration of a copy. Refusal leaves an unclosed portfolio cell; it does not force an update of the working project.
2. **Build and verify five deliveries.** For each, record reused modules and its own policy, alternate composition/refusal, authored-edit route, gameplay loop/saves, target OS/hardware, budget and launch on a clean declared system. Compare one common version with individual configurations.
3. **Check coverage and decide.** At least five accepted games, not five templates: 2D, 2.5D, spatial 3D, pixel art, stylization, realism and mobile-style on PC. Stop if the goal is met. If a meaningful question remains, choose one next candidate.
4. **Expand only after defining a new test.** G6 tactics: turn order/pathfinding. G7 racing: vehicle contacts/camera. G8 simulation: its own agent/save budget. G9 rhythm: input/audio latency. G10 procedural world: geometry loading/release. For the selected game, repeat: one real section → missing contract → authored edit → finished game → independent delivery.

**Owner check.** Start with five identified Players, five authored projects on one foundation version and a table of differences. Choose any two unlike games, such as 2D and realistic. Check the foundation version, edit one gameplay parameter in each and launch both. Open all five results in the summary. One foundation has separate configurations and different mechanics/graphics; each has independent delivery, completed loop and retained evidence. A game missing mandatory evidence does not count.

**Team evidence.** G1–G5 matrix on one declared release. Do not relabel old measurements or WSLg as a new platform. This completes the declared minimum; it does not promise support for arbitrary games.

Basis: [portfolio gates](portfolio-roadmap.md), [independent review](review.md), [verification boundaries](verification.md).

<a id="rebuild"></a>

## Engine rebuild track — a modern Vulkan renderer and ready-made libraries

**Status — the owner started this track on 21 November 2026. It remains the current priority.** The owner chose the high-end rendering route: stay on Vulkan through `ash`, render the editor through Vulkan as well, build the renderer GPU-driven from the start, and then add hybrid ray tracing, mesh shaders and other modern features on top of a solid rasterizer. Runtime pieces that are not this project's value (ECS storage, physics, audio, math) move to proven libraries; the editor, its authored-data rules and the render-graph/HLSL authoring model stay.

Phase numbers R0–R10 are planning labels, not versions. Effort is a rough range for one full-time developer with Vulkan experience; it is speculation until R0 records real throughput (see the [complete route](#map) rule on estimates). The phases are ordered by dependency; independent phases can overlap when there is more than one developer.

**Task status.** The last column records completion, active work, a pending estimate, or a paused estimate such as `Paused · 2–3 w`. Optional tasks keep their `Optional ·` prefix. Implementation chats hand over accepted results and evidence. The dedicated roadmap chat checks `main` and actual starts with the working chats, then records them together in one EN/RU pull request. The [visual roadmap](roadmap-map.md) uses these tables.

<a id="rebuild-status"></a>

**Accepted by 26 November 2026.** R1 is accepted. RV-D.1–RV-D.8, the R1.9 S3 optimisation, the [R3.1 painter comparison](editor-vulkan-spike.md#r31-results), the R4.1 editor extension contract, the R6.1 Rapier step, R7.1 glTF import and CPU animation playback, and the [R8.1 S5 prototype](r81-s5-meshlets.md#r81-result) are complete at the checked `main` snapshot. The [final Windows and Linux CI](https://github.com/ModuleEngine/ModuleEngine/actions/runs/45771712793) passed at `f8bc9ac`.

**Step 4 accepted by 27 November 2026.** [PR #67](https://github.com/ModuleEngine/ModuleEngine/pull/67) completed R4.2–R4.3. The Editor builds without games or with a selected game. A fourth game integrated without edits to `crates/editor`, and its input and HUD were checked offscreen. [PR #68](https://github.com/ModuleEngine/ModuleEngine/pull/68) completed R1.10. In paired headless Windows Release runs, the median of the mean tick times fell from 980.00 to 559.55 µs. Late rejection and retry were verified. [Measurements and scope](evidence/r110-arpg/README.md).

**Next by dependency.** R6.2, R3.2, R7.2 and R8.2 are available. The map automatically lists R6.2 first, and no next start has been approved. Step 5 (R3.2 and R3.3) has not started. R2.5 has cleared its technical prerequisites and remains paused. V0.5.0 may audit existing behavior before R3 acceptance and the owner's separate go. Implementation of 0.5 on `main` and new UI work remain paused until both. The tested audio scenario and both Practice Arena examples are preserved on the [separate 0.5 audio branch at `c199903`](https://github.com/ModuleEngine/ModuleEngine/commit/c19990396a9c5be43309f9d102a992385555d81e).

**Relation to the version route.** On 21 November 2026 the owner paused the 0.5 brief and new UI work until R1, R3 and R4 are accepted, because they change the world storage, the editor shell and the editor/game boundary that 0.5 would build on. The track does not replace any milestone: R5 is the technical base for [1.3](#real), R6–R7 supply the physics and skeleton that [0.9a](#combat) and the [G2 decisions](#g2-agreement) need, and every phase keeps the G1 and ARPG budgets from [release decisions](#decision) as regression gates. Ray tracing and mesh shaders are optional hardware tiers: G1/G3-style packages must never require them.

### Historical starting point, 0.4.2

Snapshot of `main` at `b6f54c9` before the rebuild. See [current status](current-status.md) for implemented behavior.

| Area | State at the checkpoint | Consequence |
| --- | --- | --- |
| World storage | `EcsWorld` (`crates/engine-core/src/world.rs`) keeps every column in `BTreeMap<u64, _>`; dynamic components are `serde_json::Value`. `GameSession::step_budget` (`crates/engine-core/src/game.rs`) clones the whole `GameState` before every 60 Hz tick for rollback. | Tick cost grows with world size even when nothing changes; components are parsed JSON, not typed data. |
| Earlier storage decision | [ecs-storage-decision.md](ecs-storage-decision.md) chose to extend `EcsWorld` over `hecs` as a migration-cost choice with no performance claim. | R1 must record a superseding decision backed by a benchmark. |
| Renderer binding | `render-vulkan` already uses `ash 0.38`, Vulkan 1.3 with dynamic rendering, an authored render graph (`graph.rs`) and HLSL compiled to SPIR-V through DXC (`compiler.rs`). | Keep the binding, the graph documents and the HLSL authoring contract; rewrite the architecture under them. |
| Geometry | `geometry::vertices` transforms every triangle of every item on the CPU each frame into one 72-byte-per-vertex buffer; the frame issues one `cmd_draw`. | No GPU-resident meshes, no instancing, no culling. |
| Materials and textures | All base-color textures are packed into one atlas of at most 8192 px, sampled with `Load` (nearest, no mipmaps). Only base color is read. | No PBR maps, no filtering, a hard texture limit. |
| Lighting | `forward.hlsl` hardcodes one directional light `(-0.5, 1.0, 0.7)`; there are no shadows. SSAO, FXAA and tonemap passes exist. | Lights are not data; no shadow or IBL path. |
| GPU resources | One `vkAllocateMemory` per buffer/image, one command pool and buffer, CPU waits on a fence each frame; per-pass state goes through 128-byte push constants. | No allocator, no frames in flight, no descriptor model for many resources. |
| Presentation | The editor and Player UI run on `eframe` over WGPU (DX12 on Windows, Vulkan on Linux). Vulkan frames reach them through shared D3D12/OPAQUE_FD images (`presentation.rs`, `player/src/gpu_presentation.rs`). The editor uses detached native windows. | Two GPU devices and an interop bridge; a game swapchain does not exist yet. |
| Animation | `engine-content` rejects glTF files with skins or animations. | No skeletal animation. |
| Physics | No physics engine; games use their own grid/collision code (`game-kit/src/grid.rs`). | No rigid bodies, joints or general queries. |
| Audio | `host-services` mixes WAV only and outputs through WinMM; other platforms return an error. | No compressed formats, no Linux audio. |
| Editor coupling | `crates/editor` depends directly on `gameplay-arpg`, `project-g1`, `project-puzzle` and `example-two-arenas`; `game_ui.rs` alone is about 7,600 lines. | Adding a game means changing the editor. |

### Rules for the whole track

- Authored data rules from [AGENTS.md](../AGENTS.md) stay: the authored document is separate from the Play copy, stable IDs and unknown JSON payloads survive, a failed change keeps the last good version. Scene files stay JSON; only the in-memory runtime representation changes.
- Each risky choice (R1.1, R2.1, R3.1, R8.1, R9.1) starts with a spike and a benchmark; the decision is recorded in [decisions](decisions.md) before migration starts.
- The old path stays behind a switch until the new one passes parity; it is removed in the same phase after acceptance, not left as a second production implementation.
- Every hardware-dependent feature declares its tier and a fallback. The [R0.4 hardware contract](hardware-tiers.md) lists the Vulkan 1.3 base tier A and the independent mesh shader (B) and ray tracing (C) additions, with their individual features and requirements for native Player presentation.
- Measurements follow [release decisions](#decision): sequential GPU runs, recorded driver and load, mean and p95, the real game window rather than offscreen alone.
- "Not worse than R0.6" (or than another baseline) is decided by the [not-worse rule](bench.md#not-worse), approved on 21 November 2026: the baseline commit and the candidate run interleaved in one session, five runs each; medians are compared with a per-metric tolerance of max(5 %, 2 × measured spread); a borderline result is repeated once; peak memory is compared with a 1 % floor; p99 is tracked but does not block. `python scripts/bench.py --compare <baseline>` returns the verdict for the S1–S5 metrics; R3.5 open-to-first-frame has no automated check yet.

### Benchmark scenes used by all phases

| Scene | Content | Main metrics |
| --- | --- | --- |
| S1 | An accepted G1 level at 1920×1080 Playing | Frame mean/p95 ≤16.67 ms, peak working set ≤1 GiB (existing gate) |
| S2 | ARPG at 1080p with 20 active enemies | Frame mean/p95 ≤16.667 ms (existing gate), tick time |
| S3 | Stress: 10,000 static and 1,000 moving objects, 200 unique meshes, 500 textures; 2,000 entities with 8 components each | CPU tick, CPU frame, GPU time per pass, uploads per frame, draw calls, memory |
| S4 | Lighting: a Sponza-class interior with 256 point lights, one sun and Khronos glTF sample models | GPU time per pass, image difference against a reference |
| S5 | Dense geometry: 20–50 million source triangles | GPU time, culled vs drawn meshlets, memory |

<a id="rebuild-r0"></a>

### R0 — Freeze and baseline · 1–2 weeks · depends on nothing

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R0.1 | Done 21 November 2026: the owner paused the 0.5 brief and new UI work; recorded in [current status](current-status.md) and [decisions](decisions.md). Bug fixes to accepted 0.4.2 remain allowed. | Dated decision in both languages. | Done |
| R0.2 | Done 21 November 2026: S3–S5 are reproducible fixtures (S4 on pinned Khronos Sponza) and `scripts/bench.py` writes JSON results to `outputs/bench/<date>/`; see [running benchmarks](bench.md) and the [accepted series](bench-r02-20261121.md) (maximum spread 4.75 %). | Three consecutive runs differ by ≤5 % on mean values. | Done |
| R0.3 | Added GPU timestamp queries per render-graph pass and CPU spans for tick, extraction, recording and submission. | [Frame timings](bench.md#frame-timings) are available in Editor Game and benchmark JSON. | Done |
| R0.4 | [Hardware tiers and device evidence](hardware-tiers.md). RTX 4080 SUPER is the development reference. GTX 1660 SUPER 6 GB is the candidate for minimum Windows tier A hardware. | Tier table and full Vulkan dump recorded in `docs/`. Physical qualification is on the [pre-release checklist](hardware-tiers.md#minimum-gpu). | Done |
| R0.5 | Hygiene prerequisites: GPU and DXC tests skip with a clear message when unavailable; Vulkan validation layers and debug names in debug builds; typed errors with `thiserror` in `render-vulkan` instead of `String`. | `verify` passes on a machine without DXC/GPU with explicit skips; a debug run shows no validation errors on S1. | Done |
| R0.6 | [S1–S5 baseline report](bench-r06-20261122.md) on post-R0.5 `main` (22 November 2026). All 15 runs completed with a maximum declared spread of 2.39 %. | The report is the reference for later "not worse" comparisons of these metrics. | Done |

<a id="rebuild-r1"></a>

### R1 — World storage on `bevy_ecs` · 3–5 weeks · depends on R0

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R1.1 | [Hybrid Bevy selected](ecs-storage-decision.md#r11): reflected native values with lossless codecs and JSON columns for schema data. The standalone spike compares query/mutation, spawn/despawn and snapshot/restore. | Hybrid S3 tick 34.82× faster (gate ≥5×). Both Bevy variants passed the authored-data cases. The dynamic JSON variant averaged 4.59× faster. | Done |
| R1.2 | [Stable IDs in the production ECS](ecs-world.md). An immutable `StableId(u64)` and ordered `u64 → Entity` index preserve IDs through scene/prefab resolution, saves, Player export and Rust IPC. | [Core and native fixture checks](ecs-world.md#r12-verification), including byte-identical saves, high IDs, references and Bevy slot reuse. | Done |
| R1.3 | [Component registry and codecs](component-schemas.md). The native `Transform` exposes `Reflect`. Schema components keep exact JSON keys, `null` and unknown payloads. One registry serves UI, saves, IPC and validation. | Round-trip tests on every scene in `examples/` and the G1/G2 projects produce identical JSON. See [verified corpus and open coverage](ecs-world.md#r13-verification). | Done |
| R1.4 | [Fixed-tick transaction](ecs-world.md#lifetime-and-transactions) stages command buffers and changed ECS rows. A failed tick discards the journal. | Late rejection and recovery through `GameHost`. The empty S3 transaction with the canonical ECS avoids a whole-world traversal. Structural packets, active schema validation on edited worlds, and compatibility provider diffs can still walk the world. See [verification](ecs-world.md#r14-verification). | Done |
| R1.5 | [Bevy schedule](module-execution.md) runs Core and ARPG gameplay at a fixed 60 Hz in the saved stage and node order. | Two Windows replays match the baseline event log for all ten G1 levels and the Small monsters ARPG encounter. `GameHost` late rejection rolls back and retries the tick. See [verification](module-execution.md#r15-verification). | Done |
| R1.6 | Change-driven extraction: render extraction reads `Changed<…>` and produces deltas (spawned, moved, removed, material changed) instead of a full `SceneSnapshot`. The full snapshot remains for the software renderer and diagnostics. | On S3 with 1,000 moving objects the delta contains only those objects. Schema-only edits must not produce a transform delta. | Done |
| R1.7 | Migrate consumers: `game-host`/`game_worker`, `runtime-script` typed commands, `project-g1`, `project-puzzle`, `gameplay-arpg`, `example-two-arenas`. The Object provider stays a compatibility adapter. | All crate test suites pass; no consumer touches the old column maps. | Done |
| R1.8 | Acceptance and removal of the old storage. Measure the [session checkpoint copy cost](#r18-session-checkpoint). | S2 tick p95 [not worse](bench.md#not-worse) than R0.6. S3 ECS tick p99 ≤2 ms in the standalone fixture, taken over all samples of the five paired runs combined (each run's own p99 is reported alongside). The paired comparison with R0.6 is reported in full; its memory and S3 ECS differences are the [accepted cost](#r18-accepted-cost) of R1.1/R1.2 and do not block. Compare S3 with session metadata against the R1.4 baseline and report the copy cost separately. S1/S2 absolute gates pass (60 FPS, S1 peak working set ≤1 GiB). Independent review. | Done |
| R1.9 | S3 ECS tick optimisation ([accepted regression](#r18-accepted-cost) from R1.8). First add a per-operation microbenchmark for the 1,000 motion updates: Bevy lookup, `Arc`/binding bookkeeping, JSON access. Then batch transform updates and move the per-write bookkeeping to once per batch. | Paired Release comparison against the R1.8 candidate on `main` after #38 (`133f98f`) or later: S3 ECS mean and p95 lower, no S3 sample above 2 ms in a strict per-sample run, S2 tick p95 [not worse](bench.md#not-worse) than R0.6. Late rejection and retry through `GameHost` still pass. Independent review. | Optional · Done |
| R1.10 | Native ARPG nodes share a typed `ArpgFrame` for one fixed tick. The uninterrupted native schedule decodes and encodes it once. Portable state is published before foreign and Custom nodes and reloaded after their edits. | Paired Release runs of the headless 20-enemy encounter show no regression. Accepted state and event traces match, and late rejection and retry through `GameHost` pass. [R1.10 evidence](evidence/r110-arpg/README.md). | Done |

<a id="r18-accepted-cost"></a>

**R1.8 accepted cost versus R0.6.** The earlier R1.8 snapshot `8e3b5bd` failed the paired
[not-worse](bench.md#not-worse) comparison with R0.6 on peak working set
(about +28 % on S1, +32 % on S2) and on the standalone S3 ECS tick (mean about
+60 %, p95 about +56 %). Both follow from the storage chosen in R1.1/R1.2:
Bevy rows, JSON columns, the stable-ID index and the rollback copies add
resident data and per-write bookkeeping that the R0.6 plain maps did not have.
The product owner accepted this on 23 November 2026 in return for the
R1.6 change-driven extraction and the transactional tick.

In that snapshot, the combined p99 of 1,200 standalone S3 ECS tick samples
was 1.688 ms. Per-run p99 was 2.071, 1.510, 1.584, 1.515 and 1.635 ms. All four
samples above 2 ms fell in the first run, with a maximum of 2.322 ms.

R1.8 was accepted in [PR #39](https://github.com/ModuleEngine/ModuleEngine/pull/39)
at `6ba445c2ffa9baef46a51131cf0073610fb34067`, which includes the S3 changes
from [PR #38](https://github.com/ModuleEngine/ModuleEngine/pull/38). The standalone
S3 ECS tick p99 was 0.7939 ms across 1,200 samples, with a maximum of 0.9590 ms.
S2 tick p95 improved from 3.7197 to 3.5307 ms. S1 and S2 met the 60 FPS gate in
all five pairs, and S1 peak working set was 330.105 MiB against a 1 GiB limit.
Independent review confirmed that the acceptance criteria were met. The full
paired comparison still failed solely on the accepted S1/S2 memory increases.

R1.9 remains an optional optimisation of the S3 tick. R1 was completed with
its eight required tasks. Peak memory is outside the scope of R1.9.

<a id="r18-session-checkpoint"></a>

**R1.8 debt — session checkpoint copies.** `GameState::checkpoint_tick` copies
`scene`, `module_data`, and `control` so a failed tick can restore what systems
write directly through `&mut GameState`. Use a `GameHost` S3 variant with a
representative, non-empty `module_data` payload. Record its entry count and
serialized size. Time the metadata copies separately from the full fixed tick.
Build the accepted R1.4 revision and the R1.8 candidate with the same payload
and timing probe. Compare fixed-tick mean and p95 using the paired Release
procedure and tolerance in the [not-worse rule](bench.md#not-worse). If the
copy cost materially contributes to a regression or threatens the S3 tick
budget, address it in R1.8 and recheck late rejection and retry through
`GameHost`.

<a id="rebuild-rv"></a>

### Review fixes after R2.4

Two independent reviews of `main` at `c93f919` were made on 23 November 2026, one on Windows with the reference GPU and one on Linux with a software Vulkan device. They were combined on 24 November 2026. Their findings are planned as three required stages and one optional stage:

1. **RV-A** first: the whole workspace in required CI, so that the regression tests written in RV-B and later run on every pull request. RV-A originally reserved the light route for roadmap status cells. See the [current CI routes](ci.md) for documentation-only PRs.
2. **RV-B**: the editor scenarios found by the review. R2.5 starts after RV-A, RV-B.1–RV-B.3 and RV-C.3 are done; RV-B.4 may close separately.
3. **RV-C** alongside the start of R2.5: evidence that a clean clone can read, the R2.5 baseline and, before any R6 task, the measured ARPG tick and the decision on typed components.
4. **RV-D** is optional: maintenance items taken when work touches the subsystem.

<a id="rebuild-rv-a"></a>

### RV-A — Green CI covers the whole workspace · 4–6 days · depends on R0; before R2.5

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| RV-A.1 | Fix the two tests that fail on `main` outside required CI. `migration_copy_accepts_case_distinct_filename_and_preserves_legacy_source` in `crates/editor/src/workspace_files.rs` compares two whole migration results, although the scene asset ID is generated and the path depends on the destination. `failed_partial_native_allocations_release_owned_handles_and_device_recovers` in `crates/render-vulkan/src/vulkan.rs` declares only Vulkan but renders a graph that needs DXC. | On Linux the migration test checks the meaning of the result, its references and the preserved source while allowing different IDs and paths. Without DXC the render test is skipped with a visible reason; with Vulkan and DXC it checks handle release and device recovery. | Done |
| RV-A.2 | Required CI runs `cargo test --workspace --locked` on Windows and Linux after `cargo fetch --locked`. Linux prepares software Vulkan and the pinned DXC for the render tests; a separate step without DXC checks that capability skips stay explicit. | The required gate fails when any non-ignored workspace test fails, and the run report lists counts per package. Both failures fixed in RV-A.1 would have failed it. | Done |
| RV-A.3 | Minimal Player as its own configuration in required CI: build and tests with `--no-default-features`. | Code paths for absent games and renderer are compiled and tested on every pull request that takes the full route (a documentation-only PR runs `Documentation checks`); a game or Vulkan dependency added to the minimal Player fails CI. | Done |
| RV-A.4 | A completed CI run for every merge commit on `main`. Superseded runs are cancelled only for pull requests. | Two merges in quick succession each get a finished `main` run; pull request runs still cancel superseded ones. | Done |
| RV-A.5 | Explicit offline-cache precondition for tests that build the Custom SDK with `--offline`. Today 23 tests in Script, Host and Editor fail without a full `cargo fetch`. | Without the cache these tests stop with one clear reason that names the missing step; in required CI dependencies are fetched before tests. | Done |
| RV-A.6 | Minimum Python version for `scripts/`. `tarfile.extractall(filter=...)` needs Python 3.12 or a backport (3.9.17 and later); on 3.9.13 two CI DXC unpacking tests failed. | The minimum version is documented in both languages, and `bootstrap` and `verify.py` report an unsupported interpreter before running tests. | Done |

<a id="rebuild-rv-b"></a>

### RV-B — Editor scenarios from the review · 1–1.5 weeks · depends on RV-A; RV-B.1–RV-B.3 before R2.5

Each fix starts with a regression test that fails on the current code and runs in the required CI from RV-A.

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| RV-B.1 | Scene and Play select the world that the project actually connects. Today a project with a connected `world.objects` node and a disabled `world.ecs` node still gets ECS, and with a connected Custom the choice differs between code paths (3 of 6 cases diverged). | Scene and Play reach the ready state with the actually connected Objects, ECS or Custom; adding or moving a disabled node does not change the result. | Done |
| RV-B.2 | Undo, Redo and Restore keep the data of a disabled module. Today a logical route such as `{"archive.navigation":{"path":"waypoints/route-A"}}` saved through Inspector is treated as a required file during Restore. | Undo, Redo and Restore preserve unknown fields unchanged; a referenced file that is really missing is still reported. | Done |
| RV-B.3 | An unapplied linked-template edit survives switching objects. Today a colour or field change of template A is lost after selecting an object with template B, and Inspector may show `Inspector drafts · none`. The finding comes from reading the code, so the first step is a test that reproduces it. | After A → B → A the edit is kept, or the user confirms discarding it beforehand; Apply, Discard and project close see the same draft. | Done |
| RV-B.4 | One policy for ID 0 in Core. Today `Spawn` validation accepts ID 0 for both providers, then Objects publishes the object and ECS rejects it; the constructors also differ. | The constructor, validation and apply follow one policy for both providers; after a rejection the previous state is unchanged. | Done |

<a id="rebuild-rv-c"></a>

### RV-C — Readable evidence and measured decisions · 1–1.5 weeks · depends on RV-A; RV-C.3 before R2.5, RV-C.4 before all of R6

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| RV-C.1 | Keep compact acceptance evidence next to the code in `docs/evidence/<task>/`: the comparison result, bench conditions, commit SHA, artifact hashes and the reviewer's conclusion. Large logs and PNG files stay local. Start with R2.4: the Windows review evidence and the combined review, with links that resolve in a clean clone. | A clean clone opens every link of the R2.4 evidence. A link check in CI rejects new links from `docs/` into `outputs/`, `work/` or absolute local paths. | Done |
| RV-C.2 | Record what the R2.4 result shows and what it does not. S3-100k averages about 1.76 ms with culling and 1.733 ms with culling off, so the 60 FPS target holds with a large margin but culling gives no speed-up on that scene. On the occlusion scene the measured result is fewer draw commands (2,052 down to 4) with an image that matches the reference; no paired frame-time run with culling on and off exists for that scene, so a speed-up claim there needs its own paired measurement. R2.4 stays accepted on its original criteria. Each of the three series had one interval above 16.67 ms, the worst 20.854 ms. Name the implemented Hi-Z variant: a current-frame depth seed of large occluders, then the test. | The R2.4 notes and [hardware tiers](hardware-tiers.md#r24-culling) state the long intervals, the measured effect on each scene (frame time on S3-100k, command count on the occlusion scene) and the Hi-Z variant in both languages. Later frame-rate criteria say whether single intervals above 16.67 ms are allowed. | Done |
| RV-C.3 | R2.5 baseline before the render graph work: S4 reference images and transient memory on `main`, published as in RV-C.1. | R2.5 compares against a published baseline (images, transient memory, commit, conditions) that a clean clone can read. | Done |
| RV-C.4 | Measure the ARPG tick with 20–30 enemies and split out the per-tick copies (resources, events, system states, `module_data`, `control`) and the `ArpgFrame` serialization. Evaluate copy-on-write on the same load, keeping rollback and retry. Record in [decisions](decisions.md) which frequently used components get typed access and what role Bevy keeps. | A paired Release measurement reports the share of copies and serialization. A dated decision in both languages exists before any R6 task starts. Late rejection and retry through `GameHost` still pass. | Done |

<a id="rebuild-rv-d"></a>

### RV-D — Maintenance from the review · optional · take each item when work touches its subsystem

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| RV-D.1 | Custom worker protocol on its own channel instead of a marker in the stdout that user code also writes to. | User output, including the marker text, does not break the packet parser; a test covers it. | Optional · Done |
| RV-D.2 | Enforce `clippy::undocumented_unsafe_blocks` in `render-vulkan`, then in presentation, native dialogs and Host waits. | The lint is enforced for `render-vulkan` in the clippy profile; every `unsafe` block there states its preconditions. | Optional · Done |
| RV-D.3 | Editor verification modules behind a feature flag, as `bench` already is. | A release Editor builds without the verification modules; verification routes enable the feature; the EXE size change is recorded. | Optional · Done |
| RV-D.4 | Compare the not-worse tolerance (the larger of 5 % and twice the range of five baseline runs) with the median absolute deviation or a confidence interval of the median on recorded series. | A dated decision in [benchmarks](bench.md#not-worse) to keep or change the rule, with the recorded series behind it. | Optional · Done |
| RV-D.5 | Typed errors at Core boundaries: `WorldProvider` and the renderer adapter keep the error type instead of `String`. | `VulkanError` and world errors reach the caller with their kind; these boundaries gain no new `Result<_, String>`. | Optional · Done |
| RV-D.6 | Report malformed component values. `world_component` in ARPG and similar readers count or log deserialization failures instead of treating the component as absent. | A malformed `health` produces a diagnostic in debug builds and a counter in verification output. | Optional · Done |
| RV-D.7 | Explicit consent before compiling Custom Rust from an unfamiliar Team project. | Opening such a project runs no `build.rs` or procedural macro until the user agrees; the answer is remembered per project. | Optional · Done |
| RV-D.8 | Documentation entry point. README describes the current state and the R track without paths from one PC, the build version is separated from milestone status, and `current-status.md` is split into a short current snapshot and an archive. | README and `current-status.md` in both languages match `main`; the archive keeps old entries with working links. | Optional · Done |

<a id="rebuild-r2"></a>

### R2 — GPU-driven renderer foundation · 8–12 weeks · depends on R0; R2.3 also on R1.6; R2.5 also on RV-A, RV-B.1, RV-B.2, RV-B.3 and RV-C.3 ([review fixes](#rebuild-rv))

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R2.1 | Device layer: tier detection, a memory allocator (`gpu-allocator`) instead of one allocation per resource, 2–3 frames in flight with per-frame command pools and timeline semaphores, a staging ring and a transfer queue, a deletion queue that replaces `retirement.rs` semantics. Spike first: allocator and frame pacing on S3. | Zero validation errors on S1–S3; in steady state the CPU never waits for the frame it just submitted. | Done |
| R2.2 | Bindless resources: one global descriptor set with descriptor indexing (texture array, samplers, storage buffers) or buffer device address. Replace the atlas with per-texture images, mipmaps generated at import or on upload, anisotropic sampling. The push-constant ABI gains bindless indices; custom HLSL passes keep working. | 1,000 unique textures without the 8192 px limit; no shimmering on a moving camera over S4 floors; existing custom-pass tests pass. | Done |
| R2.3 | GPU-resident scene: one mesh pool (shared vertex/index buffers) uploaded once per mesh; instance buffer with transforms and material indices updated from R1.6 deltas; material buffer. Remove the CPU vertex transform in `geometry.rs`. | Upload bytes per frame proportional to changed objects; S3 CPU frame ≤1 ms. | Done |
| R2.4 | GPU culling and indirect drawing: compute frustum culling, `vkCmdDrawIndexedIndirectCount`, then two-phase Hi-Z occlusion culling. | Draw calls per pass do not grow with object count; S3 scaled to 100,000 instances keeps 60 FPS at 1080p on the [R0.4 reference GPU](hardware-tiers.md#reference-gpu). | Done |
| R2.5 | Render graph v2: automatic barriers with synchronization2, transient resource aliasing, async compute queue. Existing graph documents load unchanged; built-in passes (forward, gbuffer, deferred, SSAO, FXAA, tonemap, color encode) are migrated. | Existing graph and custom-pass tests pass; built-ins match the old images within the `image_quality_check` tolerance; transient memory on S4 is at least 30 % lower. | Paused · 2–3 w |
| R2.6 | Game swapchain for Player: present modes, frame pacing, resize. HDR output itself is [R10.2](#rebuild). | Player S1/S2 run directly on the swapchain; frame-time p95 [not worse](bench.md#not-worse) than R0.6. | 1 w |
| R2.7 | Parity and switch: the old renderer behind a feature flag until S1/S2 pass, then removal. | S1/S2 gates pass on the new renderer; old code deleted; independent review. | 0.5–1 w |

<a id="rebuild-r3"></a>

### R3 — Editor on Vulkan, no DX12 bridge · 3–5 weeks · depends on R2.1; can overlap R2.2–R2.7

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R3.1 | Spike: `eframe` has no Vulkan backend. Compare `winit` + `egui-winit` + `egui-ash-renderer` against a thin own egui painter on the R2 device. [Decision](decisions.md#r31-editor-painter): R3.2 starts with `egui-ash-renderer 0.13.0`. ModuleEngine owns the device, swapchain and frame lifetime. | Decision recorded; the spike draws the current editor at ≥60 FPS with crisp text at 100 % and 150 % DPI. | Done |
| R3.2 | Replace the `eframe` shell in Editor and Player UI: window, input, clipboard, IME, DPI, cursor, and the detached native windows from `native_windows.rs`. egui widgets and panes stay unchanged. | Every existing pane opens; detached windows, docking and resize work; native window checks pass. | 1–1.5 w |
| R3.3 | Scene and Game views sample render-graph output images on the same device. Remove the D3D12/OPAQUE_FD interop path after acceptance. | One frame less latency than today; `gpu_presentation.rs` interop removed; Linux uses the same path. | 4–5 d |
| R3.4 | Port verification hooks that depend on `eframe` (`__screenshot`, `ui_capture.rs`, editor benchmark). | UI capture and editor benchmark run as before. | 2–3 d |
| R3.5 | Acceptance. | Open-to-first-frame [not worse](bench.md#not-worse) than R0.6 (the paired open-to-first-frame check is still to be built: neither `--compare` nor the R0.6 report measures it); owner opens the editor, moves panes and plays G1 without visible change; independent review. | 1–2 d |

<a id="rebuild-r4"></a>

### R4 — Decouple the editor from concrete games · 1–2 weeks · depends on R1

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R4.1 | Define an editor extension interface: a game registers its panels, inspectors, verification scenarios and Play hooks. | Interface documented in [architecture](architecture.md). | Done |
| R4.2 | Move game-specific code out of `game_ui.rs` and related editor files into the `ui` features of `gameplay-arpg`, `project-g1` and `project-puzzle`, together with the game presentation registration in `crates/player/src/presentations.rs`; turn `example-two-arenas` into an example that uses the same interface. | The editor crate builds with no game crate in its dependencies, and Player registers game presentations through the same interface. | Done |
| R4.3 | Composition selects which games are linked into an editor build. | Adding a test game requires no change in `crates/editor`; G1 and ARPG behave as before. | Done |

<a id="rebuild-r5"></a>

### R5 — Physically based materials, lights and shadows · 6–9 weeks · depends on R2

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R5.1 | glTF metallic-roughness materials: base color, normal, metallic-roughness, occlusion, emissive, alpha mask/blend; importer and material asset in the editor; BC7/BC5 compression at import. | Khronos sample models (DamagedHelmet, FlightHelmet) import with all maps; editor changes reach Player. | 1.5 w |
| R5.2 | Lights as ECS components (directional, point, spot, area later) with clustered light culling in compute; one shading model for forward and deferred. | S4 with 256 point lights: lighting ≤2 ms at 1080p on the [R0.4 reference GPU](hardware-tiers.md#reference-gpu). | 1.5 w |
| R5.3 | Shadows: cascaded shadow maps for the sun (4 cascades, PCF with optional PCSS), a shadow atlas for point/spot lights, cached static shadows. | No visible cascade seams or acne on S4; shadows ≤2.5 ms. | 1.5–2 w |
| R5.4 | Image-based lighting and sky: environment cubemap, prefiltered specular, irradiance as spherical harmonics, BRDF LUT, a physical sky model. | Sample models match the Khronos glTF Sample Viewer within an agreed image tolerance. | 1 w |
| R5.5 | HDR pipeline: physical camera exposure with auto exposure, bloom, tonemapper choice (ACES/AgX), motion vectors and TAA. TAA is the prerequisite for ray-tracing denoising and upscaling. | Stable image without crawling edges in motion over S4; previous FXAA profile still available. | 1.5 w |
| R5.6 | Editor workflow: edit lights and materials live, a material preview sphere, the R0.3 overlay shows pass costs. | Owner changes one object's roughness and sees it in Editor and Player (the [1.3](#real) owner check). | 0.5–1 w |

<a id="rebuild-r6"></a>

### R6 — Physics with `rapier3d`, audio with `kira`, shared math · 2–3 weeks · depends on R1; all of R6 also on [RV-C.4](#rebuild-rv-c); can overlap R2–R5

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R6.1 | `rapier3d`: rigid body, collider and joint components; a physics step synchronized to the 60 Hz tick with `enhanced-determinism`; raycasts and shape queries for systems and scripts; collision events as `GameEvent`; debug draw in the Scene view. | Recorded-input replay stays identical; S2 with projectiles: physics ≤1 ms per tick. | Done |
| R6.2 | Move game collision code to `rapier3d` where it is a general physics problem; grid rules in `game-kit` stay as game rules. | G1 levels and the ARPG encounter play as before. | 2–3 d |
| R6.3 | `kira` replaces the WinMM mixer: OGG/MP3/FLAC/WAV, buses, volume and fades, spatial audio; Linux output. Existing game-event-to-sound bindings and audio composition from 0.5 planning keep their meaning. | Same bindings play on Windows and Linux; measured output latency ≤50 ms. | 4–5 d |
| R6.4 | `glam` for math instead of hand-written matrix helpers (`transform_point` and similar); `rfd` for native file dialogs where the editor has its own. | No hand-written matrix code left outside codecs; dialogs work on Windows and Linux. | 2–3 d |

<a id="rebuild-r7"></a>

### R7 — Skeletal animation · 3–5 weeks · depends on R2.3 and R1; first consumer is the G2 hero

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R7.1 | Import glTF skins, joints and animation clips (the importer rejects them today). | Khronos skinned samples (CesiumMan, Fox) import and play. | Done |
| R7.2 | GPU skinning in compute, writing into the mesh pool, so culling, shadows and later ray-tracing refits see the skinned pose. | 50 skinned characters on S2 within the frame budget. | 1 w |
| R7.3 | Animation runtime: clip sampling, blending, additive layers, root motion, events on frames. | Attack and dodge timing from the G2 decisions 24–25 can be driven by animation events. | 1 w |
| R7.4 | Animation state graph with both a visual editor and a text file, applied during paused Play (G2 decisions 22 and 26). This is also the animation-graph task of [0.9a](#combat) step 3; it is tracked only here. | Owner edits a transition while paused and sees it after resuming. | 1–1.5 w |
| R7.5 | Equipment swaps that change the hero's appearance (G2 decision 14) through attached meshes on joints. | Changing weapon/armor changes the rendered hero. | 2–3 d |

<a id="rebuild-r8"></a>

### R8 — Mesh shaders and virtualized geometry · 4–6 weeks · depends on R2.4; tier B with tier A fallback

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R8.1 | Spike: meshlet build with `meshopt` and a task/mesh shader path on S5. [Decision](decisions.md#r81-meshlet-prototype): keep R2.4 as the production path and retain the experimental Tier B prototype. | Decision with numbers: mesh path vs R2.4 indirect path. | Done |
| R8.2 | Meshlets at import: clustering, bounds and normal cones stored in the asset. | Meshlet data cached; import time recorded. | 3–4 d |
| R8.3 | Task/mesh shader pipeline with per-meshlet frustum, cone and Hi-Z culling. | S5 at 60 FPS at 1080p on the [R0.4 reference GPU](hardware-tiers.md#reference-gpu) using tier B. | 1.5 w |
| R8.4 | Continuous LOD: simplified LOD chain (`meshopt` simplify) or meshlet hierarchy with screen-space error selection, without visible popping. | S5 memory and GPU time within agreed budget; no popping in a fly-through. | 1.5–2 w |
| R8.5 | Fallback: the same meshlet data drawn through compute culling and indirect draws on tier A. | Identical image on tier A within tolerance. | 3–4 d |

<a id="rebuild-r9"></a>

### R9 — Hybrid ray tracing · 10–16 weeks · depends on R5 (TAA, PBR); skinned refit depends on R7; tier C with raster fallback

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R9.1 | Acceleration structures: BLAS per mesh with compaction, TLAS rebuilt every frame from the GPU instance buffer, BLAS refit for skinned meshes, memory budget and streaming of BLAS builds. Spike first on S4. | TLAS build ≤0.5 ms for S3; acceleration-structure memory reported in the overlay. | 2 w |
| R9.2 | Ray-traced shadows with ray queries in compute, soft shadows from area/sun size, temporal denoiser. | S4 sun + 16 shadowed lights ≤2 ms; stable in motion; falls back to R5.3. | 1.5 w |
| R9.3 | Ray-traced ambient occlusion as an option next to SSAO. | Visible improvement in contact areas; ≤1 ms; SSAO remains the fallback. | 1 w |
| R9.4 | Hybrid reflections: screen-space reflections with ray-traced fallback for off-screen and rough surfaces; spatiotemporal denoising. Evaluate integrating NVIDIA NRD or AMD FidelityFX denoisers through FFI against an own SVGF-class denoiser. | Mirror and glossy reflections stable in motion; ≤3 ms on S4. | 2–3 w |
| R9.5 | Dynamic global illumination with ray-traced irradiance probes (DDGI class), with probe placement editing in the editor. ReSTIR GI is a later option, not part of this phase. | Bounced light reacts to a moved light within 1 s; ≤2 ms; raster fallback is IBL from R5.4. | 2–3 w |
| R9.6 | Path-traced reference mode in the editor: progressive accumulation with the same materials, used to check PBR and the hybrid result. | Reference and hybrid frames comparable side by side in the editor. | 1–1.5 w |
| R9.7 | Quality presets and capability rules: each RT effect has a raster fallback, presets Low/Medium/High/RT, and a package without RT never requires tier C. | A G1 package runs on a tier A GPU; the same G5-class project switches between RT and raster without authored-data changes. | 0.5–1 w |

<a id="rebuild-r10"></a>

### R10 — Other modern features · 3–6 weeks · each item needs a named consumer before it starts

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R10.1 | Temporal upscaling: AMD FSR (open source) through FFI on top of R5.5 TAA inputs; NVIDIA DLSS through Streamline as an optional plugin. | Render at 67 % resolution with quality comparable to native TAA; ≥30 % frame-time gain on S4 with RT. | 1–2 w |
| R10.2 | HDR display output (scRGB/HDR10) with a separate SDR path, including the HDR-ready swapchain moved out of R2.6. | Correct output on an HDR monitor; SDR unchanged. | 3–4 d |
| R10.3 | GPU particles in compute with sorting and lighting. | 100,000 particles ≤1 ms. | 1 w |
| R10.4 | Variable-rate shading where the GPU supports it. | ≥10 % gain on S4 without visible artifacts, or the feature is dropped. | 3–4 d |
| R10.5 | Texture and mesh streaming with a residency budget. | S5 runs within a fixed VRAM budget without stalls. | 1–2 w |

### Cross-cutting hygiene, done inside the phases

- `thiserror` error types in every crate touched by a phase; no new `Result<_, String>` APIs.
- GPU/DXC-dependent tests skip explicitly instead of failing on machines without them (R0.5).
- Backups of authored projects rotate with a fixed count when migrations touch them (R1, R4).
- Each phase updates [architecture](architecture.md), [decisions](decisions.md), [current status](current-status.md) and its contract documents in both languages, and ends with an independent review per [review](review.md).

### Order, dependencies and total effort

- R0 → R1 → R4 and R6.
- R0 → R2.1 → R2.2 → R2.3 (also needs R1.6) → R2.4 → R2.5 → R2.6 → R2.7.
- [Review fixes](#rebuild-rv): R0 → RV-A → RV-B.1–RV-B.3 → R2.5 (RV-B.4 may close separately); RV-A → RV-C (RV-C.3 before R2.5, RV-C.4 before all of R6); RV-D is optional and taken alongside related work.
- R2.1 → R3, in parallel with the rest of R2.
- R1 + R3 + R4 accepted and a separate owner go → resume 0.5 implementation and new UI work.
- R1 + R4 + R6 accepted → [AI-assisted live authoring](#ai-authoring) AI-A → AI-B (outside this track's effort totals).
- R2 → R5 → R9 and R10; R2.4 → R8; R1 + R2.3 → R7 → skinned refit in R9.1.

| Block | Phases | Effort, one developer |
| --- | --- | --- |
| Core and editor foundation | R0, R1, R2, R3, R4 | 16–26 weeks |
| Libraries and animation | R6, R7 | 5–8 weeks |
| Modern image | R5, R8, R9, R10 | 23–37 weeks |
| Whole track | R0–R10 | about 44–71 weeks (11–17 months) |
| Review fixes | RV-A, RV-B, RV-C | about 3–4 weeks, outside the R0–R10 total; optional RV-D about 2–3 weeks more |

The first useful checkpoint is after R2 + R5: a GPU-driven renderer with PBR and shadows, about 4.5–7 months in (R0, R1, R2 and R5). 0.5 implementation and new UI work resume after R1, R3 and R4 are accepted and the owner gives a separate go; R5–R10 can then continue in parallel with the version route, and R6–R7 should land before [0.9a](#combat) needs them. Replace these estimates with measured throughput after R0 and again after R2.

**Version route.** The revised plan items add about 15 weeks of one developer's work for V0.5.0–V0.8.4 (0.5 about 4 w, 0.6 about 3 w, 0.7 about 4 w, 0.8 about 4 w) and about 36 weeks for V0.9a.1–V1.3.x.4 (0.9a 6 w, 0.9b 7 w, 1.0 5.5 w, 1.1 6 w, 1.2 3.5 w, 1.3 4 w, 1.3.x 4 w); these are rough guesses before measured throughput, and the 1.0 and 1.3.x items are mostly human work. Order: V0.5.0 → V0.5.1 → V0.5.2 in parallel with V0.6.1–V0.6.2; V0.5.3 after R6.3; V0.7.1 after R2.5; V0.8 after V0.6; V0.7.4 after R5.2. V0.5.0 may audit existing behavior before R3 acceptance and the owner's separate go. 0.5 implementation and new UI work remain paused until both. V0.6.1–V0.6.2 do not require new UI.

**Stop conditions.** If an R1.1 or R3.1 spike misses its criterion, stop and record the alternative before migrating. If a phase breaks an S1/S2 gate or an authored-data invariant, it returns for correction while the previous path stays active. A modern feature without a consumer or that misses its cost budget is dropped rather than kept as an unused subsystem.

<a id="ai-authoring"></a>

## AI-assisted live authoring — build a game by dialogue while it runs

**Status — idea recorded by the owner on 21 November 2026; no implementation has started.** The author types a request in plain language ("add a WASD character", "add a double jump", "make the platforms move"). An AI prepares a change while the game keeps running, the engine applies the ready result, and the author tries it at once and refines it with the next request. "Live" does not mean instant generation: it means the game is not closed while a change is prepared and applied.

Stages AI-A and AI-B are planning labels, not versions. By the owner's decision of 21 November 2026 this work is **not near-term**: no stage starts before [R1](#rebuild-r1), [R4](#rebuild-r4) and [R6](#rebuild-r6) are accepted, and it does not pause or reorder the [rebuild track](#rebuild). Those phases supply what live authoring needs: the change journal and stable-ID index (R1), the editor extension interface (R4) and physics for jump and moving platforms (R6).

### What already exists and what is missing

Checked against `main` at `bae6047` (R1.2, R1.4 and R1.6 done; R1.8, R4 and R6 not yet); AI-A starts by re-checking this table:

| Area | Current state | Consequence |
| --- | --- | --- |
| New logic during Play | [Custom Rust](custom-rust.md) compiles in the background; a compatible hot apply keeps declared state ([custom-state.md](custom-state.md)); a failed tick restores the last accepted state. If a Custom system panics, times out or returns rejected commands, GameWorker pauses further ticks until Restart or a successful Apply of corrected code ([game-worker.md](game-worker.md)); the previous implementation is not restored automatically. | Generated mechanics can already be applied without stopping the game, but a faulty one needs Restart or a corrected Apply. |
| Scene and system graph during Play | Authored changes and a new graph require **Stop** ([editor-game.md](editor-game.md)); **Restart** is cheap (no recompilation, no GPU recreation). | Applying a scene change still means Stop → Apply → Play. R1.2 (`StableId` index) and R1.4 (change journal) are done and are the base for live patching in AI-B. |
| Ready-made modules | ARPG systems give X/Z movement, collision, combat and health ([gameplay.md](gameplay.md)); there is no gravity, jump or rigid-body physics. | A platformer needs [R6](#rebuild-r6) physics; without it jump would be hand-written Rust. |
| Vocabulary for the AI | Component schemas and the [capability inventory](engine-capabilities.md) describe installed modules and fields; `--check` validates a saved project. | The AI can be given a typed catalogue and a validator instead of guessing. |
| Delivery | Save, `--check` and `--export-game` produce a Player with prebuilt workers. | Accepted changes become a standalone game through the existing route. |
| Safety | The Custom worker has ordinary OS permissions; it is not a sandbox ([questions](questions.md)). | Generated Rust runs only locally and is shown to the author. |

### Rules for every stage

- **Order of means.** The AI first changes data (scene JSON, prefabs, overrides, settings), then assembles installed systems in `game.execution`, and only then writes new logic as Custom Rust. It does not write native modules: they require rebuilding the editor. Rust that proves useful repeatedly is moved into a native module by a person.
- **One request is one changeset:** an authored JSON patch, Custom sources and the request text. It passes the same validation as Apply and `--check` before it is applied; a rejected changeset changes nothing.
- **The AI edits only the authored document, never the Play copy.** Saving is the ordinary Save; no change may exist only at runtime. Authored-data rules from [AGENTS.md](../AGENTS.md) stay in force.
- **Undo** reverts one changeset as one history entry. Reverting the edit (authored data) is separate from reverting the game moment (a Play-state snapshot).
- **Runtime failure** (panic, the 150 ms timeout, rejected commands) rolls back the whole changeset, and its diagnostics go back to the AI for the next attempt.
- **Team** keeps its whitelist: the AI may change scenes and permitted gameplay Rust only.
- Sending project content to an external AI service is the owner's explicit choice of provider and data, made before AI-A starts.

<a id="ai-a"></a>

### AI-A — External-agent prototype · 1 week · depends on R1, R4 and R6 accepted

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| AI-A.0 | Re-check the table above on the then-current `main`; confirm that WASD reaches game input in an empty project without the ARPG package; measure Custom Rust compile time per request. | Results recorded with numbers; a missing input source is added as the smallest generic module or the prototype uses an existing game package. | 0.5–1 d |
| AI-A.1 | A command-line agent works on a project folder with tools: read the module/schema inventory (including the `rapier3d` bodies, colliders, joints, queries and collision events that R6.1 delivers), read and write scene JSON and `custom_sources`, run `--check`. R6.1 promises no ready-made character controller, so the prototype gets WASD movement and jump from a small generated Custom Rust system built on those physics primitives; a reusable controller is not a prerequisite. No engine or editor code changes. | The agent writes a changeset into the project folder, and it passes `--check` for each request. | 2 d |
| AI-A.2 | Run the five reference requests on an empty scene (character with WASD; platforms and jump; double jump; moving platforms; slower movement and a new environment). Route for each request: (1) start from a saved project with no unsaved editor changes; (2) the agent writes and validates the candidate in the project folder; (3) the author reopens the project through **File → Open Project…** or **Recent Projects** ([editor workflow](editor-workflow.md)), because Stop → Play uses the editor's in-memory authored document and would otherwise run the old scene even after `--check` passes; (4) press Play and verify the requested change. | Recorded: share of requests playable within ≤2 iterations, time from request to playable result, number of manual edits, share of requests met by modules without new Rust. | 1–2 d |
| AI-A.3 | Decision. | Owner decides go / change / drop on the recorded metrics; the result is recorded in [decisions](decisions.md). | 0.5 d |

<a id="ai-b"></a>

### AI-B — Editor integration and live scene patching · 2–3 weeks · depends on an AI-A go decision

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| AI-B.1 | A request panel as an editor extension through the [R4.1](#rebuild-r4) interface; status of the pending changeset; the game keeps running while it is prepared. | The editor crate gains no dependency on an AI client outside the extension; the panel can be absent. | 3–4 d |
| AI-B.2 | Live scene patching: the same authored diff is applied to the Play copy through the R1.4 change journal and the R1.2 `StableId` index, without Stop. | Adding/moving/removing objects and changing component fields appears in the running game; a rejected patch leaves both documents unchanged (test). | 4–5 d |
| AI-B.3 | Changeset history: one Undo entry per request, automatic rollback on runtime failure, diagnostics returned to the agent. | Undo of a request restores authored data byte-identically; a panicking, timing-out or rejected generated system leaves the last accepted state, and the previous implementation is restored automatically and keeps running without a manual Restart (the current engine only pauses ticks until Restart or a corrected Apply). | 2–3 d |
| AI-B.4 | Acceptance. | The owner repeats the AI-A five-request route inside the editor without Stop for scene changes; independent review. | 1 d |

**Order:** R1 + R4 + R6 accepted → AI-A → (go) → AI-B. **Stop conditions:** if AI-A shows that most requests need manual repair, drop or reshape the idea before AI-B; AI-B never weakens authored-data invariants to make a patch apply live.

[↑ Back to the complete route](#map)

<a id="usability"></a>

## How to establish use without the engine's author

Test three roles: a game author configures existing capabilities; an extension developer writes new code; a player launches the delivered game. A finite set of attempts cannot establish “any developer.” The operational promise is that someone in the declared role needs only the delivery and its normal help, without internal knowledge or oral instructions from the engine's creator. Configuration requires no Rust; writing a new implementation requires the appropriate language and SDK.

| Participant card, without menu names | Success |
| --- | --- |
| U1 — Orientation | From ordinary launch, identify game, rights and what renders the frame; distinguish installed, used and ready. Team has a different route; unavailable architecture is not promised. |
| U2 — Visible change | Change permitted anti-aliasing, explain the difference, save, reopen and observe the selected result in Player. |
| U3 — Error | Try an incompatible variant in a tutorial copy; explain cause and next step in their own words; resume without data loss. |
| U4 — Two games | While changing B, keep A available and rebuildable. Reverse configuration first; separately reverse a native-set update at the appropriate milestone. |
| U5 — New developer | Restore Full from its description, add their extension using documentation, deliver a limited setting and game, and show composition with/without the capability. |
| U6 — Recipient | Receive only the game, launch in the declared clean environment, play, exit and continue. A clean author workstation is a separate procedure. |

**Initial threshold.** For the first target, the owner plus one independent author in the appropriate role. Initial guidance for an already prepared example: at most five minutes to find the setting and ten minutes of active work to change and restore it; record installation/build waiting separately. These are proposed UX targets. Refine them publicly after the first observation and before the next acceptance, never retrospectively to fit results. No check requires prior knowledge of the technical term FXAA.

**1.0 threshold.** The owner plus four new developers. On shared U1–U3 tasks in applicable deliveries, the owner and at least three of four developers succeed without observer help. For role tasks, two of two unfamiliar extension authors and two of two game authors pass their mandatory operations. U4 runs in Full, U5 with the technical role and U6 with a separate recipient. Data safety, A/B isolation and Team restrictions must pass in **every** attempt. Investigate a repeated identical obstacle even when the numeric threshold passes. Record active time, build time separately, wrong turns, help use, hints and expected outcomes. Delivered help is allowed; an oral hint makes an attempt “with help.”

<a id="decision"></a>

## Release decisions — a short answer for the owner, evidence for the team

Each card step ends in a visible result or explicit refusal. The developer owns implementation/data. The independent verifier owns observation, preservation checks and binding results to that same artifact. The owner evaluates actions and interface meaning instead of reciting a technical report.

**Delivery card:** version/task → folder/project to open → two to four actions → expected change → what must remain unchanged → how to return. Record “independently / with a hint / unsuccessful / not checked” and the stopping point. Delivered links to the build and working example are mandatory. See [owner cards](product-roadmap-checks.md).

**Evidence behind the card.** Bind HEAD, a digest including uncommitted files, delivery hashes, selected features and module versions. Preserve original/accepted data references and isolated test saves. For affected contracts, check incompatibility, Cancel, Undo/Redo, no-op, external edits, hidden drafts, Save/reopen, Play/Stop and export. No-op creates neither override nor migration. Choose checks according to the change; a label edit does not require the heavy matrix. Substantial changes require independent review. Technical verification does not replace a person's attempt.

**Budgets and platforms.** Preserve ARPG's 1080p/20-active-enemy criteria: mean and p95 ≤16.667 ms, plus cold Custom preparation in Editor ≤45 seconds for the accepted scenario. For the accepted G1 scenario: 1920×1080 Playing, mean and p95 ≤16.67 ms and peak working set ≤1 GiB. A light menu or offscreen check does not replace those limits. Run GPU/timing checks sequentially and record conditions, driver version and load. Give G3–G5 their own budgets before concrete content work. Declare Linux features separately, including audio, and test on actual target hardware before claiming support. Players and authors require different environments.

**Stop conditions and limits.** Data destruction, an unrequested change to A, a rights bypass or a failed mandatory scenario returns the candidate for correction while preserving the previous set. If the problem is comprehension, establish what the person expected before expanding functionality. Add a subsystem only with a named consumer (such as G2's skeleton or G3's sprite), a usefulness criterion and a disabling rule. A public marketplace, stable binary ABI, networking, another graphics API, arbitrary interchangeability of every part and a catalogue of independent Engines are not hidden requirements of 1.0.

Basis: [verification](verification.md), [independent review](review.md), [UI rules](editor-ui-spec.md), [G1 evidence boundaries](g1-verification-20261101.md).

<a id="g2-agreement"></a>

### Preserved G2 decisions of 6 November

The following 27 recorded choices are part of this planning baseline, not requirements obtainable only from a historical audit attachment. The first G2 goal establishes and verifies reusable foundations; the second builds the complete game on that accepted foundation. This does not turn the first vertical slice into acceptance of all foundations still needed by the campaign.

| No. | Agreed decision |
| --- | --- |
| 1 | A small campaign across several locations. |
| 2 | Three locations. |
| 3 | Hero actions: attack, dodge and one ability. |
| 4 | The active ability is a directed projectile. |
| 5 | Dodge is a dash with brief invulnerability. |
| 6 | Ordinary enemies: melee fighter, shooter and heavy. |
| 7 | Dash and projectile have separate cooldowns; no additional shared resource is required by this choice. |
| 8 | Progression uses experience, levels and equipment with random loot. |
| 9 | Three equipment slots: weapon, armor and amulet. |
| 10 | Base items have one or two random bonuses. |
| 11 | Death preserves experience/items and respawns at a checkpoint. |
| 12 | Only the current unfinished encounter resets after death. |
| 13 | Target ordinary playthrough: 20–30 minutes. |
| 14 | Changing weapon/armor changes the hero's appearance. |
| 15 | Melee weapons: one-handed sword and two-handed hammer. |
| 16 | Final boss: three attacks and two phases. |
| 17 | Each level-up offers a choice of one upgrade. |
| 18 | Both inventory and level-up choice pause the game. |
| 19 | After exiting, resume at the checkpoint with progress preserved. |
| 20 | Visual references: Diablo III. |
| 21 | Use freely available ready-made assets with adaptation and appropriate redistribution rights. |
| 22 | Animation states/transitions have both a visual graph and a text file already in G2. |
| 23 | Enemy behavior/boss phases have both a behavior graph and a text file already in G2. |
| 24 | Attacks briefly stop the hero's movement. |
| 25 | Dodge can interrupt an attack or projectile preparation at any moment. |
| 26 | Apply animation/AI graph changes during paused Play. |
| 27 | Items appear on the ground and are picked up by clicking. |
