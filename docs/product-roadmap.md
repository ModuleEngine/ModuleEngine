# ModuleEngine Product Roadmap

[English](product-roadmap.md) | [Русский](ru/product-roadmap.md) | [Documentation](README.md)

Recorded project roadmap and planning baseline, updated 17 November 2026.

The final demonstration is concrete: you assemble the capabilities you need. A colleague reproduces that configuration from its description and changes a second game. The first game still builds and plays as before. A player receives only what their game needs.

This document records the reviewed sequence and its full acceptance expectations. **The owner closed 0.4.2 on 17 November 2026; 0.5 is the next planned milestone. Its implementation has not started.** The 0.4.1 implementation and historical 0.4.2 engineering evidence remain recorded below. Future version numbers and milestone scopes are planning labels: they do not mean those versions have shipped, authorize every implementation, or promise dates. Existing specifications, ownership boundaries and budgets remain in force. Use the agreed [0.5 composition and dependency scope](#compose) for the next brief and the [owner check cards](product-roadmap-checks.md) for short personal checks.

[Version map](#map) · [Baseline](#baseline) · [Starting handoff](#start) · [Independent use](#usability) · [Release decisions](#decision)

<a id="map"></a>

## The complete route

| Version / planning label | New evidence | Direction |
| --- | --- | --- |
| [0.4](#baseline) | Starting point: one engine, working G1 on the current Windows PC and a separate Player. | Existing baseline |
| [0.4.1](#render) | First complete route from ordinary launch to the image in Player, with compact settings and permitted choices in the game menu. | Editor |
| [0.4.2](#image-and-docking) | Image, panes, stabilization and compact chrome; closed by the owner’s decision. | Accepted 17 Nov 2026 |
| [0.5](#compose) | Add, remove or replace a capability without unrelated settings getting in the way. | Next · planned |
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

**Entry.** The owner closed 0.4.2 on 17 November 2026. Before implementation, define a separate brief from the [agreed 0.5 scope](#compose): current capabilities, examples with/without audio, rights and actual Player-composition evidence. Implementation of 0.5 has not started.

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

**Status — next planned milestone; implementation has not started and follows a separate brief.**

Disabling a button while retaining its service, resource requirement and Player dependency does not remove unnecessary engine capabilities.

**Entry and dependencies.** The owner has closed 0.4.2; preserve the 0.4.1 route. Define a separate 0.5 brief from the agreed scope below; this does not gate parallel G1/G2 checks. Reconcile stage 4 with 2A, then cover existing capabilities in stage 5 and the dependency map in stage 6.

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

### Ordered work

1. **Take one real episode and assets.** A small arena, hero, one opponent, obstacle and checkpoint. Select a model/clips distributable with the source project; define skeleton, import, pose transition, hit event and weapon attachment. Agree readability from the gameplay camera, not merely the importer preview.
2. **Build execution foundations on that episode.** Movement/collision, melee hit, interruptible action, dash and projectile with independent cooldowns. The technical module plays the clip and reports an event; combat mechanics decide who takes damage; values and appearance belong to the game. Check miss, hit, wall and stopping.
3. **Let the author control the actual encounter.** Animation and behavior graphs each have a text source and visible active state. During paused Play, change a transition/condition. Preserve compatible state, offer explicit reset for incompatibility, retain the accepted version on error. A static mockup does not count.
4. **Close the loop through Player.** Audio/visual telegraph, combat outcome, death and checkpoint return. Build Player and let someone play without explaining its schemas. Compare the trial encounter with a 20-active-enemy load early, before multiplying content.
5. **Obtain a second independent composition.** The same shared combat works in a second project with fixed stats and no XP/loot systems, mandatory fields or dependencies. Finish and verify required reusable first-goal G2 contracts before full campaign production. Run affected G1 regressions.

**Owner check.** Start with two test projects and the arena Player; one project has no progression. Fight, dodge and reach a checkpoint. As author, change one animation transition and one enemy condition, observe effects and restore them. Run the same encounter in the second composition. Behavior visibly changes and returns after reversal. The second game needs neither XP/loot nor changes to Core/Host game rules.

**Team evidence.** Real skeleton import/frame, executable graphs/text, package/save, second consumer, original G2 load and G1 regressions. A new shared module has a consumer, not merely a description.

Basis: [portfolio/game gates](portfolio-roadmap.md), [G1 assessment](g1-engine-assessment.md), the preserved G2 decisions above.

<a id="campaign"></a>

## 0.9b — the author builds complete G2 on the verified foundation

One encounter shows how to extend the engine. Only a full game demonstrates that editor, content and saves withstand production.

**Entry and dependencies.** G2's second goal starts after its accepted first-goal foundation. 0.9a is the first complete example, not a substitute for the remaining required foundation.

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

1. **Choose a common version and accept transitions only in verification copies.** Old pinned instances/packages stay usable. For each game, explicitly accept a compatible common release or perform a verified migration of a copy. Refusal leaves an unclosed portfolio cell; it does not force an update of the working project.
2. **Build and verify five deliveries.** For each, record reused modules and its own policy, alternate composition/refusal, authored-edit route, gameplay loop/saves, target OS/hardware, budget and launch on a clean declared system. Compare one common version with individual configurations.
3. **Check coverage and decide.** At least five accepted games, not five templates: 2D, 2.5D, spatial 3D, pixel art, stylization, realism and mobile-style on PC. Stop if the goal is met. If a meaningful question remains, choose one next candidate.
4. **Expand only after defining a new test.** G6 tactics: turn order/pathfinding. G7 racing: vehicle contacts/camera. G8 simulation: its own agent/save budget. G9 rhythm: input/audio latency. G10 procedural world: geometry loading/release. For the selected game, repeat: one real section → missing contract → authored edit → finished game → independent delivery.

**Owner check.** Start with five identified Players, five authored projects on one foundation version and a table of differences. Choose any two unlike games, such as 2D and realistic. Check the foundation version, edit one gameplay parameter in each and launch both. Open all five results in the summary. One foundation has separate configurations and different mechanics/graphics; each has independent delivery, completed loop and retained evidence. A game missing mandatory evidence does not count.

**Team evidence.** G1–G5 matrix on one declared release. Do not relabel old measurements or WSLg as a new platform. This completes the declared minimum; it does not promise support for arbitrary games.

Basis: [portfolio gates](portfolio-roadmap.md), [independent review](review.md), [verification boundaries](verification.md).

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
