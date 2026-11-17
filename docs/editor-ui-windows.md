# Editor windows and workspaces

[English](editor-ui-windows.md) | [Русский](ru/editor-ui-windows.md) | [Catalogue](editor-ui-catalog.md)

Complete EN/RU meaning comes from one bilingual catalogue. All pictures are design proposals, not running-editor captures. Mockup UI is English under the language contract. Eight state categories are specified per surface, including explained N/A. An existing surface does not mean its proposed layout is implemented.

<a id="p01"></a>

## P01 — Engine Overview

**Current surface; proposed layout.**

**Purpose:** Understand this project’s technical composition and reach its real configuration.

**Entry:** Engine preset; Windows → Engine Overview; opening reuses a matching area.

**Access and scope:** Current tool requires Engine authoring/source availability; a retained unavailable area shows a reason, not unlocked Team access.

**Sources:** [crates/editor/src/engine_overview.rs::engine_overview_panel](../crates/editor/src/engine_overview.rs), [crates/editor/src/dock.rs::EditorType](../crates/editor/src/dock.rs)

### Requirements

- Separate Technical Engine, reusable Gameplay, Project and Unknown ownership.

- Show project/configuration basis/rights and installed, offered, participating evidence independently.

- Use only real navigation targets; inspection never executes factories or compiles.

- Use the focused hierarchy prototype: role-based row spacing, selected item marker, restrained boundaries and at most one contextual primary action. Readonly facts remain visually distinct from editable fields.

- Keep composition groups, subsystem selection and details in separate visual levels. Place real render-graph navigation next to the selected renderer; preserve installed/offered/participating facts and the runtime-readiness limitation.

**Actions / controls:** `Open render graph`, `Show renderer node`, `Source details`, `Refresh evidence`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected group and subsystem have persistent markers; the renderer detail has one primary Open render graph action. Show Installed, Offered and Participating as three separate facts, not a Ready badge. |
| Empty / no selection | No matching capabilities. Clear the search; this is not proof that the engine is empty. |
| Busy / preparing | Use available snapshots and identify pending evidence; reading must not launch a factory, service or compiler. |
| Draft / modified | Not applicable: this view does not own an authored draft; show project dirty state only in the shell. |
| Error / rejected | Explain the failed evidence/read operation and retain the previous readable record with its provenance. |
| Restricted / read-only | Omit or disable unsupported navigation under actual rights; retaining an area never unlocks it. |
| Missing / stale dependency | Evidence unavailable for a capability: show Unknown and its source/error, never an inactive checkbox. |
| Narrow / short area | At a 480-point dock area, drill into the selected subsystem with Back to composition. Keep rights/context, supported actions and evidence visible; selection and search survive the return. This is an area variant, not an entire OS window. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

- Inspect the full-size and 480-point compact-area layouts: identify context, selection, primary action and draft/inheritance state without relying on colour alone. These static proposals do not establish native interaction acceptance.

**Visual hierarchy:** [rationale and geometry](editor-ui-hierarchy.md).

**Mockups:** [Layout](media/editor-ui-catalog/p01.svg) · [State sheet](media/editor-ui-catalog/p01-states.svg) · [Compact area](media/editor-ui-catalog/p01-compact.svg).

<a id="p02"></a>

## P02 — Module Blueprint / root graph

**Current surface; proposed layout.**

**Purpose:** Inspect and edit the current root module process without confusing it with GPU passes or the gameplay schedule.

**Entry:** Windows → Module Blueprint; Overview opens the actual selected root scope.

**Access and scope:** Full with matching complete sources; existing source and candidate validation remain authoritative.

**Sources:** [crates/editor/src/panes.rs::module_graph_area](../crates/editor/src/panes.rs), [crates/editor/src/app.rs::graph_panel](../crates/editor/src/app.rs)

### Requirements

- Show the actual scope breadcrumb and document/apply status.

- Use MMB/RMB pan, pointer wheel zoom, background/node RMB actions and mixed Delete/Undo as the agreed target.

- Keep one authority: node inspector, raw JSON and graph edit the same data; opening a subgraph does not imply a universal scheduler.

- Do not render a persistent Add/Open/Frame/Delete toolbar. Background RMB opens Add, a double LMB click opens an existing permitted subgraph, F frames the current graph and Delete removes the selection. Context menus remain discoverable equivalents of these same commands.

**Actions / controls:** `Add node`, `Open subgraph`, `Frame graph`, `Delete selection`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | The canvas starts directly below the pane header, without repeated Add/Open/Frame/Delete buttons. Context actions and gestures target the same current graph and existing validated commands. |
| Empty / no selection | Show No nodes with a contextual Right-click to add hint, available scope and actual compatible palette. Do not create default nodes merely to fill the canvas. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Invalid wire/node candidate: show endpoints and exact contract mismatch; keep the last-good executable graph. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** With graph-canvas focus: F frames the current graph and Delete removes the mixed selection through validation/one Undo. Double-click an expandable node outside pins/embedded controls to enter its subgraph; text, popups and modals own their input first. RMB click adds/opens actions; MMB/RMB drag pans; plain wheel zooms. Escape cancels the active gesture. Menu key/Shift+F10 opens the focused graph target’s context menu as a proposed keyboard equivalent.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

- Confirm no permanent command row and no reserved empty toolbar strip; RMB creation, eligible-node double-click, focused-canvas F/Delete and menu equivalents preserve scope, focus priority and Undo. This is a proposed input contract, not native implementation evidence.

**Mockups:** [Layout](media/editor-ui-catalog/p02.svg) · [State sheet](media/editor-ui-catalog/p02-states.svg).

<a id="p03"></a>

## P03 — Module Library

**Current surface; proposed layout.**

**Purpose:** Find a compatible node for a clearly identified graph destination.

**Entry:** Windows → Module Library; Add Node and RMB use the same scope-aware catalogue.

**Access and scope:** Real graph scope, installed descriptors and Full/Team/source rules; no global bypass list.

**Sources:** [crates/editor/src/app.rs::library](../crates/editor/src/app.rs), [crates/editor/src/app.rs::library_graph_context](../crates/editor/src/app.rs)

### Requirements

- Searchable category rows replace the wall of full-width add buttons.

- Name the destination graph, not only a pane number; ambiguous destinations require explicit choice.

- Pin contracts live in contextual help/optional Connection types, not an unexplained permanent footer; no source export command here.

**Actions / controls:** `Search nodes`, `Choose target`, `Add selected node`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No nodes match this search/scope. Clear search or choose an actual compatible graph. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | The catalogue owns no node draft; an unresolved Project File/graph draft explains why Add is disabled. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Target area was closed or changed scope: clear the stale target and offer a valid target; never add elsewhere silently. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p03.svg) · [State sheet](media/editor-ui-catalog/p03-states.svg).

<a id="p04"></a>

## P04 — Engine Exposure

**Current surface; proposed layout.**

**Purpose:** Define exactly which parameters and ready variants a game project may change.

**Entry:** Overview configuration route or Windows → Engine Exposure.

**Access and scope:** Full and complete matching sources only; Project cannot add architecture or widen its own exposure.

**Sources:** [crates/editor/src/properties.rs::engine_settings_panel](../crates/editor/src/properties.rs), [docs/settings.md](settings.md)

### Requirements

- Group exposed parameters, ready variant sets/defaults and participating feature definitions.

- Expose only existing eligible pass parameters/implementations; removing exposure reconciles orphaned Project values.

- PropertySchema exposure and ComponentSchema ownership remain distinct; show source pins and bounds.

**Actions / controls:** `Expose parameter`, `Stop exposing`, `Configure variant set`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Exposed parameter ID | `readonly` | tone-map.exposure | Stable exposed identity and target, not a freely editable value. |
| Default / minimum / maximum | `vector3` | 1.0 / 0.0 / 8.0 | Engine-owned numeric schema/default; labels here are Default / Min / Max, not spatial axes. |
| Allowed AA variants | `readonly` | Off / FXAA | Only eligible real implementations may enter the exposure set. |
| Default AA | `enum` | FXAA | engine.exposure.variants default from allowed options. |
| Feature definitions | `readonly` | Participating source owners only | engine.enabled_modules and source ownership remain explicit. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Nothing is exposed to Project. Offer eligible source-backed controls, not fabricated settings. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Exposure/default/target validation failed: retain old definitions and Project values, show the rejected field. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p04.svg) · [State sheet](media/editor-ui-catalog/p04-states.svg).

<a id="p05"></a>

## P05 — Project Settings

**Current surface; proposed layout.**

**Purpose:** Set permitted game-level values and rules without replacing the engine architecture.

**Entry:** Tools → Project Settings or Windows → Project Settings.

**Access and scope:** Only Engine-exposed controls and participating owner schemas; same rules apply to scripts.

**Sources:** [crates/editor/src/properties.rs::project_settings_panel](../crates/editor/src/properties.rs), [docs/settings.md](settings.md)

### Requirements

- Show authored value, effective default and active variant separately.

- Rules select only existing variant options; first matching rule per group wins.

- Absent optional modules have no live settings; changing AA value never changes the allowed implementation set.

**Actions / controls:** `Edit value`, `Choose allowed variant`, `Add rule`, `Remove rule`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Exposure | `number` | 1.0 | project.parameters for an Engine-exposed control ID; finite source-defined bounds. |
| Anti-aliasing | `enum` | FXAA | project.variants chooses only allowed Off / FXAA in this example. |
| Rule 1 | `readonly` | Time >= 1.0 → AA = FXAA | Ordered rule condition/variant; edit through the typed rule controls, not a free text expression. |
| Architecture | `readonly` | Defined by Engine | Project cannot widen the variant set or replace a renderer. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No applicable exposed controls: explain that Engine defines them; do not show irrelevant network/combat settings. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | An exposed target was removed: reconcile through the shared rules and report it, rather than targeting another pass. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p05.svg) · [State sheet](media/editor-ui-catalog/p05-states.svg).

<a id="p06"></a>

## P06 — Scene

**Current surface; proposed layout.**

**Purpose:** Select and place authored objects with a free editor camera.

**Entry:** Windows → Scene; initial Project layout.

**Access and scope:** Current scene authoring guards, one-object selection and Play lock; viewing does not grant editing.

**Sources:** [crates/editor/src/scene_workspace.rs::scene_panel](../crates/editor/src/scene_workspace.rs), [crates/editor/src/scene_view_ui.rs::SceneViewportState](../crates/editor/src/scene_view_ui.rs)

### Requirements

- Icon Move/Rotate/Scale rail and World/Local, snap/step, Frame overlays consume their own events.

- Object picking, gizmo capture and camera gestures have distinct owners; preserve G/R/S/F and Escape.

- One completed gesture creates one Undo; model drops create a new object, texture drops target the selected material.

**Actions / controls:** `Move`, `Rotate`, `Scale`, `Frame Selection`, `Snap`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Empty authored scene remains navigable; Hierarchy/Add or a model drop creates content, no hidden demo replacement. |
| Busy / preparing | Preparing Scene resources: show progress state and retain labelled last-good pixels; overlay selection may stay responsive. |
| Draft / modified | An active drag shows tentative transform; Escape restores the exact pre-gesture value and Stop never saves Play motion. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | During Play authoring is locked; Game controls are separate, camera/viewing policies remain explicit. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p06.svg) · [State sheet](media/editor-ui-catalog/p06-states.svg).

<a id="p07"></a>

## P07 — Hierarchy

**Current surface; proposed layout.**

**Purpose:** Choose scenes and authored objects and manage their hierarchy.

**Entry:** Windows → Hierarchy; shares the selected object with Scene and Inspector.

**Access and scope:** Authoring/Play validation, stable scene/object IDs and parent-cycle checks.

**Sources:** [crates/editor/src/scene_workspace.rs::scene_hierarchy_panel](../crates/editor/src/scene_workspace.rs), [crates/editor/src/hierarchy_ui.rs::hierarchy_ui](../crates/editor/src/hierarchy_ui.rs), [crates/editor/src/scene_sources.rs::draw_scene_source_dialog](../crates/editor/src/scene_sources.rs)

### Requirements

- Current-scene selector and overflow, search and one Add-object icon precede full-width 28-point collapsible rows. Scene file commands live in the overflow/File/Assets, not a permanent button stack.

- Object row selects without changing data; parent changes are explicit validated edits.

- Add/delete preserve reference validation and one Undo; do not invent group editing in this slice.

- Show authored names, then prefab identity or Object <id>; F2/context Rename is an explicit metadata edit with Cancel. Filtering retains ancestors; file moves preserve stable IDs.

**Actions / controls:** `Scene file menu`, `Search objects`, `Add object`, `Rename object (F2)`, `Delete object (Delete)`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Scene has no objects: show Add Object; no scene means show a supported scene creation route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Rename stays local until Apply; Cancel/no-op retain exact names, null/absence and history. A stale scene rejects Apply. |
| Error / rejected | A delete/reparent would break the scene or references: explain the offending identity and leave the authored hierarchy unchanged. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Below 180 logical points in height, search/Add move into scene overflow and count metadata is hidden; keep a complete object row and a filtered caption. Wider/taller areas retain full search. |

**Keyboard / focus:** F2/Delete require focus on the selected object row; text fields own their keys. Escape cancels the rename dialog.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p07.svg) · [State sheet](media/editor-ui-catalog/p07-states.svg).

<a id="p08"></a>

## P08 — Inspector

**Current surface; proposed layout.**

**Purpose:** Edit the selected authored object and source-owned component fields safely.

**Entry:** Windows → Inspector; selection comes from Scene/Hierarchy or a deliberate pinned view.

**Access and scope:** Component owner/schema/SDK pins, active composition and Play restrictions; no-op Apply creates no override.

**Sources:** [crates/editor/src/game_ui.rs::inspector_ui_with_composition](../crates/editor/src/game_ui.rs), [docs/component-schemas.md](component-schemas.md)

### Requirements

- Group Transform, parent/prefab/asset bindings and components; no duplicate generic heading.

- Show inherited, locally overridden and unknown/unavailable fields distinctly with explicit reset/inherit actions.

- Preserve exact keys, null, unknown payload and namespaces; advanced JSON is labelled and validated.

- Use the focused hierarchy prototype: role-based row spacing, selected item marker, restrained boundaries and at most one contextual primary action. Readonly facts remain visually distinct from editable fields.

- Group Transform and each source-owned component separately. Transform gestures keep their existing Undo route; Apply Actor fields submits only authored Actor fields. Current draft-cache rebuilds after Apply/selection do not establish preservation of other unsaved component drafts; protecting those transitions is a future implementation requirement.

**Actions / controls:** `Apply Actor fields`, `Component actions`, `Add component`, `Inherit defaults`, `Remove component`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Selected object | `readonly` | Hero · ID 1 | Authored selection, never the transient Play object store. |
| Position | `vector3` | 0.00 / 0.00 / 5.00 | Authored transform position; one completed gesture is one Undo. |
| Rotation (radians) | `vector3` | 0.00 / 0.00 / 0.00 | Preserve stored rotation units and finite values. |
| Scale | `vector3` | 1.00 / 1.00 / 1.00 | Authored transform scale, not a module schema field. |
| Prefab | `enum` | Hero template | Linked prefab identity with explicit inherit/override behavior. |
| Actor / speed | `number` | 6.5 | Example local Actor draft 6.5, authored local override 6.0. Apply Actor fields validates and commits this component only; it does not batch Transform or Health edits. |
| Health / current | `number` | 100 | Example gameplay component, present only when its package participates. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Object identity is readonly; Transform and component sections are distinct. Labels and origin text distinguish inherited values, authored overrides and local drafts. Each component owns its Apply. |
| Empty / no selection | Select an object in Scene or Hierarchy; do not display values from a previously deleted object. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Show Actor speed draft 6.5 separately from authored override 6.0; Apply submits only Actor authored fields. Current Apply/selection rebuilds may reset other unsaved component drafts. Preserving or explicitly resolving them before that rebuild is a future implementation criterion, not an existing guard. A no-op must not create an override. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Missing prefab/model/schema retains the reference and opaque payload; explain availability, do not erase it. |
| Narrow / short area | At a 480-point dock area use one column, bounded label width and three vector fields per row. Keep per-component origin, pending draft and Apply visible; auxiliary context moves out of the field column. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

- Inspect the full-size and 480-point compact-area layouts: identify context, selection, primary action and draft/inheritance state without relying on colour alone. These static proposals do not establish native interaction acceptance.

**Visual hierarchy:** [rationale and geometry](editor-ui-hierarchy.md).

**Mockups:** [Layout](media/editor-ui-catalog/p08.svg) · [State sheet](media/editor-ui-catalog/p08-states.svg) · [Compact area](media/editor-ui-catalog/p08-compact.svg).

<a id="p09"></a>

## P09 — Assets

**Current surface; proposed layout.**

**Purpose:** Browse, import, inspect and safely organize this project’s assets.

**Entry:** Windows → Assets; initial Project layout; Trash is subview V03.

**Access and scope:** Project asset roots, stable IDs, importer support, protected files and save/reference transaction guards.

**Sources:** [crates/editor/src/assets_workspace_ui.rs::assets_panel](../crates/editor/src/assets_workspace_ui.rs), [crates/editor/src/assets_ui.rs::BrowserState](../crates/editor/src/assets_ui.rs)

### Requirements

- Folder pictograms/tree disclosure, search/type filter, Tiles/List and selected details have one shared identity.

- Show address toggles breadcrumbs to selectable project-relative read-only text; Copy path copies it, Copy absolute path is explicit. Escape/blur returns breadcrumbs; manual path navigation is not added.

- Rename/move/delete/usage dialogs use D03–D07; file operations save first and preserve references atomically.

- Use the focused hierarchy prototype: role-based row spacing, selected item marker, restrained boundaries and at most one contextual primary action. Readonly facts remain visually distinct from editable fields.

- Use a folder rail, scoped search and tile/list region, with a separate selected-asset detail. Persistent selection includes a marker/check. Import is the primary folder action; Rename/Move/Trash remain explicit selected-asset commands.

**Actions / controls:** `Import Asset`, `New folder`, `Refresh`, `Tiles`, `List`, `Show address`, `Copy path`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Folder and asset selection are independently visible. Breadcrumbs and optional selectable address identify the same folder; type icons distinguish folders, models, textures, audio and other files. Details refer only to the selected stable asset ID. |
| Empty / no selection | No files in this folder or no search matches: distinguish those cases; retain folder navigation and Import. |
| Busy / preparing | Scanning/importing/preview work is identified per operation; thumbnails do not block browsing or initialize a renderer on drag. |
| Draft / modified | Not applicable: browsing has no authored draft; file-command forms are local and the save guard resolves existing project drafts. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Retain the selected missing asset ID/path and its error; missing preview is not proof the file has no references. |
| Narrow / short area | At a 480-point dock area show the selected asset detail with Back to models. Preserve folder/search/selection when returning; the illustrated alternate address is readonly, not a second navigation authority. File operations still use save-first guards. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

- Inspect the full-size and 480-point compact-area layouts: identify context, selection, primary action and draft/inheritance state without relying on colour alone. These static proposals do not establish native interaction acceptance.

**Visual hierarchy:** [rationale and geometry](editor-ui-hierarchy.md).

**Mockups:** [Layout](media/editor-ui-catalog/p09.svg) · [State sheet](media/editor-ui-catalog/p09-states.svg) · [Compact area](media/editor-ui-catalog/p09-compact.svg).

<a id="p10"></a>

## P10 — Game

**Current surface; proposed layout.**

**Purpose:** Run and observe the Play copy with the game’s camera/input, separate from authoring.

**Entry:** Windows → Game; global Play and local controls operate the same session.

**Access and scope:** Existing GameHost preparation, source/compiled logic capabilities and bounded worker/input contracts.

**Sources:** [crates/editor/src/app.rs::game_scene_panel](../crates/editor/src/app.rs), [crates/editor/src/game_ui.rs::scene_ui](../crates/editor/src/game_ui.rs)

### Requirements

- Keep Play/Pause/Stop/Restart icons and clear keyboard focus/capture guidance.

- Game HUD is project/module content, not a fixed engine combat or puzzle UI.

- Stop restores the authored scene; paused/stale/error frames identify their provenance, not fake live output.

- Use the shared shell toolbar as the only always-visible Game-session controller: state-dependent Play/Pause, Stop and supported Restart; Save stays a separate project command. The Game-area toolbar contains only viewport-specific actions such as Focus game.

**Actions / controls:** `Focus game`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | The illustrated Playing session shows Pause, Stop and Restart once in the shell; the Game area shows Focus game. Session status is visible; local controls do not start a second session. |
| Empty / no selection | No authored game scene: explain the missing game route; do not silently substitute the legacy 3D demo. |
| Busy / preparing | Preparing logic/resources or renderer candidate: nonblocking status, real Stop capability and labelled previous pixels. |
| Draft / modified | Play state is transient, not a dirty authored document. Unresolved authoring drafts block launch under existing guards. |
| Error / rejected | Worker/input/Custom error pauses or rejects according to its contract; show state tick/pixel revision and recovery route. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Keep session controls in the shared shell and viewport-specific actions in labelled overflow. Do not add another local Play/Stop row when the pane narrows; the Game viewport retains focus/state indication. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p10.svg) · [State sheet](media/editor-ui-catalog/p10-states.svg).

<a id="p11"></a>

## P11 — Gameplay Systems

**Current surface; proposed layout.**

**Purpose:** Author the game’s staged execution graph, resources and dependencies.

**Entry:** Windows → Gameplay Systems; Overview opens a real system node when available.

**Access and scope:** Current Engine-authoring gate and frozen Team schedule apply even to gameplay-owned systems; Stop Play before schedule Apply.

**Sources:** [crates/editor/src/app.rs::game_systems_panel](../crates/editor/src/app.rs), [crates/editor/src/game_ui.rs::systems_ui](../crates/editor/src/game_ui.rs), [docs/module-execution.md](module-execution.md)

### Requirements

- Show stage order, resources/bindings, installed-system palette and node parameters as connected parts of one schedule.

- Visual canvas and execution JSON share a draft; Apply validates dependencies and Discard returns to authored data.

- Combat is gameplay, not a compulsory technical engine service; omitted packages have no offered systems.

**Actions / controls:** `Add stage`, `Add system`, `Bind resource`, `Apply schedule`, `Discard draft`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Stage | `enum` | Update | Selected system stage. |
| World binding | `enum` | world | The value is a resource name, not the GameWorld type string. |
| After | `enum` | Movement | Example Collision depends on Movement; no self dependency. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Explain the missing selection or empty collection; offer only a supported creation or navigation route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Stage/resource/node edits remain a local graph/JSON draft; ordinary Save cannot bypass unresolved schedule work. |
| Error / rejected | Cycle, missing binding or incompatible resource: highlight the exact node/port and retain the previous executor. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stage/resource panes become selectable subviews; keep scope, validation and Apply/Discard visible. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p11.svg) · [State sheet](media/editor-ui-catalog/p11-states.svg).

<a id="p12"></a>

## P12 — Custom / Rust — side by side

**Current surface; proposed layout.**

**Purpose:** Edit a named Rust source and its supported visual representation while observing candidate status.

**Entry:** Windows → Custom / Rust; source picker and Text/Graph/Side by side select local views (V07/V08).

**Access and scope:** Use can_edit_source and the real game/non-game slot policies; not all Rust editing is inferred to require Full.

**Sources:** [crates/editor/src/panes.rs::rust_area](../crates/editor/src/panes.rs), [crates/editor/src/app.rs::custom_panel](../crates/editor/src/app.rs), [crates/editor/src/game_ui.rs::custom_editor_ui](../crates/editor/src/game_ui.rs), [docs/custom-rust.md](custom-rust.md)

### Requirements

- Text, supported graph and side inspector share the same source; show the exact slot and game/root context.

- Unsupported Rust stays editable as preserved text fragments; no claim of full Rust visual coverage.

- Compiler error retains working code/state. Compatible hot reload keeps state; reset/migration requires explicit V09 action.

**Actions / controls:** `Choose source`, `Text`, `Graph`, `Side by side`, `Apply Rust`, `Open state recovery`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No editable source for this context: explain the slot/permission and show only an already supported navigation route. |
| Busy / preparing | Candidate compiling off the UI thread; show slot/revision and retain previous working code/state. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Compiler/ABI/state rejection names the candidate and keeps its text; the current source must match before reset/migrate. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p12.svg) · [State sheet](media/editor-ui-catalog/p12-states.svg).

<a id="p13"></a>

## P13 — Custom / HLSL

**Current surface; proposed layout.**

**Purpose:** Edit an actual selected raster/compute pass source and declared resources.

**Entry:** Windows → Custom / HLSL or GPU node → Edit HLSL; Open Vulkan graph returns to that pass.

**Access and scope:** Full/source and selected pass/manifest checks; no arbitrary new API/backend is implied by the text editor.

**Sources:** [crates/editor/src/panels.rs::shader_panel](../crates/editor/src/panels.rs), [crates/editor/src/panels.rs::gpu_pass_controls](../crates/editor/src/panels.rs), [docs/gpu-hlsl.md](gpu-hlsl.md)

### Requirements

- Show pass ID, source ID, raster/compute stages, entry points and declared resource bindings.

- Text remains HLSL; the graph wires passes/resources, not a promise of bidirectional HLSL algorithm nodes.

- Compile/apply preserves last-good renderer and maps diagnostics to source lines/manifest fields.

**Actions / controls:** `Choose shader`, `Apply HLSL`, `Open Vulkan graph`, `New Compute template`, `New Scene template`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Select an actual Custom HLSL pass or use a supported template/replacement; no fake source is auto-created. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Compile/resource/entry mismatch leaves working renderer visible with a retained-frame label and editable rejected text. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p13.svg) · [State sheet](media/editor-ui-catalog/p13-states.svg).

<a id="p14"></a>

## P14 — Project File

**Current surface; proposed layout.**

**Purpose:** Inspect and edit the exact authored project JSON as another representation of the same document.

**Entry:** Windows → Project File.

**Access and scope:** Existing Engine/source gate; raw text is not a bypass for Team or pinned schema checks.

**Sources:** [crates/editor/src/app.rs::file_panel](../crates/editor/src/app.rs), [docs/graph-format.md](graph-format.md)

### Requirements

- Use a code editor with line numbers, clear JSON draft state, Apply file and Discard draft.

- Syntax/semantic validation preserves exact unknown keys, null and other namespaces.

- Dirty text, authored document and accepted runtime may differ; Save obeys existing draft guards.

- Save Project is invoked from the shared shell or File menu, not another local Save button. Apply file and Discard draft remain distinct draft operations and do not replace saving.

**Actions / controls:** `Apply file`, `Discard draft`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | The local command row contains Apply file and Discard draft; Save Project remains in the shell/File menu. Clearly distinguish authored document, text draft and saved bytes. |
| Empty / no selection | An empty text draft is invalid, not a new empty project; retain the previous authored document. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Show syntax line/column or schema/graph path; Discard returns exact authored text and failed Apply keeps the draft. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p14.svg) · [State sheet](media/editor-ui-catalog/p14-states.svg).

<a id="p15"></a>

## P15 — History / Merge

**Current surface; proposed layout.**

**Purpose:** Undo/redo authored changes, save checkpoints and compare/merge project files.

**Entry:** Edit → History / Merge or Windows → History / Merge; V04 details conflicts.

**Access and scope:** Existing restore/merge validation, asset restoration and source/Team rules; simulation state is not authored history.

**Sources:** [crates/editor/src/panels.rs::history_panel](../crates/editor/src/panels.rs), [docs/history-assets.md](history-assets.md)

### Requirements

- Separate document Undo/Redo, saved checkpoints and external base/theirs comparison.

- Show stable version identity/label and exact source paths; do not advertise automatic semantic merge of every object-array conflict.

- Restore/Apply merged validates candidate/resources before publication; unresolved conflicts cannot be applied as a clean project.

**Actions / controls:** `Undo`, `Redo`, `Save checkpoint`, `Restore`, `Compare`, `Prepare merge`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Checkpoint label | `text` | Before render edit | Label for an authored checkpoint, not a project rename. |
| Common base file | `text` | base-project.json | Existing comparison path; reading does not change the current project. |
| Other contributor file | `text` | other-project.json | Existing other-version path, not a native picker invented by this mockup. |
| Merge result | `readonly` | 2 conflicts · open V04 | Resolve actual conflicts and validate before publication. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No checkpoints or no differences are different successful empty states; Undo/Redo reflect their actual stacks. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | A merge candidate is separate from the authored document; JSON/schedule/tool drafts must be resolved before Apply. |
| Error / rejected | Unreadable base/theirs or rejected restore leaves the current project, references and checkpoint files intact. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p15.svg) · [State sheet](media/editor-ui-catalog/p15-states.svg).

<a id="p16"></a>

## P16 — Diagnostics

**Current surface; proposed layout.**

**Purpose:** Understand current failures, operation state and measured counters without interpreting every counter as FPS.

**Entry:** Tools → Diagnostics, Windows → Diagnostics or an operation-status link.

**Access and scope:** Read actual available diagnostics; retain rights for any navigation and do not compile merely to inspect.

**Sources:** [crates/editor/src/panes.rs::diagnostics_panel](../crates/editor/src/panes.rs), [crates/editor/src/activity.rs::draw_activity](../crates/editor/src/activity.rs)

### Requirements

- Group operation errors, graph validation, source/profile availability, Scene/Game worker and render counters.

- Label counter scope/reset lifetime and CPU preparation time; distinguish state tick, pixel revision and GPU/scanout timing.

- Selectable/copyable details and source links supplement concise summaries; no fabricated progress.

**Actions / controls:** `Filter diagnostics`, `Copy details`, `Open relevant tool`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Display source-backed facts and supported navigation; viewing has no authoring or initialization side effects. |
| Empty / no selection | No graph errors is a factual validation result, not certification of every subsystem or performance. |
| Busy / preparing | Render/Custom preparation is indeterminate unless real progress exists; keep operation ownership and last-good state visible. |
| Draft / modified | Not applicable: this view does not own an authored draft; show project dirty state only in the shell. |
| Error / rejected | Explain the failed evidence/read operation and retain the previous readable record with its provenance. |
| Restricted / read-only | Omit or disable unsupported navigation under actual rights; retaining an area never unlocks it. |
| Missing / stale dependency | Distinguish unknown evidence from known absence and preserve the unresolved identity. |
| Narrow / short area | Stack list and detail views with a visible Back route; keep identity and evidence state. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p16.svg) · [State sheet](media/editor-ui-catalog/p16-states.svg).

<a id="p17"></a>

## P17 — 3D Viewport — legacy preview

**Current surface; proposed layout.**

**Purpose:** Retain the legacy root-world demonstration while making its separation from authored Scene explicit.

**Entry:** Current Engine windows/type selector; proposed grouping under legacy/diagnostics, without changing serialized IDs.

**Access and scope:** Current Engine/source restrictions; starting this preview is not starting the authored Game.

**Sources:** [crates/editor/src/app.rs::viewport_panel](../crates/editor/src/app.rs), [crates/editor/src/app.rs::switch_world](../crates/editor/src/app.rs), [crates/editor/src/app.rs::switch_gpu_path](../crates/editor/src/app.rs), [crates/editor/src/editor_workflow.rs::draw_workflow_toolbar](../crates/editor/src/editor_workflow.rs)

### Requirements

- Label Legacy root preview and show the actual world/renderer configuration.

- Objects/ECS and Forward/Deferred currently mutate the document; proposed explicit configuration/preset review replaces view-like toggles.

- Preserve camera/start/reset and existing layouts; do not silently remove this editor type.

- Remove local Start/Pause: current viewport_panel and shared PlayPause both change Editor.playing. Keep the labelled Reset preview command, which clears this legacy runtime/time, and Open configuration; do not imply an independent preview clock.

**Actions / controls:** `Reset preview`, `Open configuration`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Use the shared shell Play/Pause instead of local Start/Pause. The labelled Preview group contains only Reset preview and Open configuration; Reset targets the legacy runtime/time, not an independent session. |
| Empty / no selection | No valid root-world output: show the missing route; do not load the game scene or a default world implicitly. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Camera navigation is personal state; a world/preset change is authored and must be labelled as such. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p17.svg) · [State sheet](media/editor-ui-catalog/p17-states.svg).

<a id="p18"></a>

## P18 — Object Tree — legacy runtime

**Current surface; proposed layout.**

**Purpose:** Inspect objects of the legacy root runtime, not the authored game hierarchy.

**Entry:** Current Engine window list; legacy grouping is a design proposal, IDs remain.

**Access and scope:** Current Engine/source checks and resolved runtime object identity.

**Sources:** [crates/editor/src/properties.rs::object_tree_panel](../crates/editor/src/properties.rs)

### Requirements

- Show runtime/root context beside the object identity and a selection pin if supported.

- Never present this selection as the authored Hierarchy selection automatically.

- Known empty IDs differ from unavailable runtime evidence; preserve the distinction.

**Actions / controls:** `Select runtime object`, `Open legacy properties`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Display source-backed facts and supported navigation; viewing has no authoring or initialization side effects. |
| Empty / no selection | Use a factual empty/no-match message, not an error or a suggestion that all capabilities are absent. |
| Busy / preparing | Use available snapshots and identify pending evidence; reading must not launch a factory, service or compiler. |
| Draft / modified | Not applicable: this view does not own an authored draft; show project dirty state only in the shell. |
| Error / rejected | Explain the failed evidence/read operation and retain the previous readable record with its provenance. |
| Restricted / read-only | Omit or disable unsupported navigation under actual rights; retaining an area never unlocks it. |
| Missing / stale dependency | Runtime unavailable or replacing: label identity evidence unknown; do not clear authored references as if the set were empty. |
| Narrow / short area | Stack list and detail views with a visible Back route; keep identity and evidence state. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p18.svg) · [State sheet](media/editor-ui-catalog/p18-states.svg).

<a id="p19"></a>

## P19 — Object / Components — legacy properties

**Current surface; proposed layout.**

**Purpose:** Edit permitted legacy object/component properties against known runtime/startup IDs.

**Entry:** Current Engine window list; selected object from legacy Object Tree or explicit pin.

**Access and scope:** Current Full/source gate plus exposed PropertySchema applicability and startup persistence constraints.

**Sources:** [crates/editor/src/properties.rs::object_inspector_panel](../crates/editor/src/properties.rs), [docs/settings.md](settings.md)

### Requirements

- Label this legacy property store separately from authored ComponentSchema fields in Inspector.

- Show selected object ID, exposed fields and authored versus effective values; preserve unknown payload.

- Runtime-spawned IDs cannot acquire persistent startup fields without the supported persistence route.

**Actions / controls:** `Select object`, `Pin selection`, `Edit exposed field`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Legacy runtime object | `readonly` | ID 1 | Selection belongs to Object Tree, not authored Hierarchy. |
| Exposed numeric value | `number` | 1.0 | Example eligible PropertySchema value, with real scope/range validation. |
| Persistence | `readonly` | Startup identity required | Runtime-spawned IDs cannot gain persistent startup fields by guessing. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Explain the missing selection or empty collection; offer only a supported creation or navigation route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | An unknown/runtime-only ID cannot be persisted by guessing; show the ScenePersistence requirement and retain old fields. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p19.svg) · [State sheet](media/editor-ui-catalog/p19-states.svg).

<a id="p20"></a>

## P20 — Participating Modules — panel 100

**Current surface; proposed layout.**

**Purpose:** Inspect actual participating module contracts without activating them.

**Entry:** Windows / area selector when panel 100 and its scene.extract owner are available.

**Access and scope:** Registration, installed owner, canonical participation and engine_only=false checks; availability can be unknown.

**Sources:** [crates/editor/src/app.rs::register_module_panel](../crates/editor/src/app.rs), [crates/editor/src/panes.rs::participating_modules_panel](../crates/editor/src/panes.rs), [crates/engine-core/src/settings.rs::motion_property_module](../crates/engine-core/src/settings.rs)

### Requirements

- List effective participating modules from the authored document, expand their descriptor/property contracts and show unknown modules explicitly. Keep this separate from the panel’s own owner/availability metadata.

- Do not invoke the callback for inactive/unavailable owners merely to populate an empty preview.

- Keep this real extension separate from Overview capability inventory and future arbitrary module panels.

**Actions / controls:** read-only

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Display source-backed facts and supported navigation; viewing has no authoring or initialization side effects. |
| Empty / no selection | Use a factual empty/no-match message, not an error or a suggestion that all capabilities are absent. |
| Busy / preparing | Use available snapshots and identify pending evidence; reading must not launch a factory, service or compiler. |
| Draft / modified | Not applicable: this view does not own an authored draft; show project dirty state only in the shell. |
| Error / rejected | Explain the failed evidence/read operation and retain the previous readable record with its provenance. |
| Restricted / read-only | Omit or disable unsupported navigation under actual rights; retaining an area never unlocks it. |
| Missing / stale dependency | Owner missing/inactive/unknown: show a disabled panel shell and reason; do not fabricate a live contract table. |
| Narrow / short area | Stack list and detail views with a visible Back route; keep identity and evidence state. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p20.svg) · [State sheet](media/editor-ui-catalog/p20-states.svg).

<a id="p21"></a>

## P21 — Crate Garden Level Palette — tool 110

**Current surface; proposed layout.**

**Purpose:** Paint a G1-specific board draft through the reusable editor-tool lifecycle.

**Entry:** Available module tool 110 for participating g1.rules; it is Project-owned, not core engine mechanics.

**Access and scope:** Owner participation, no conflicting JSON/schedule/scene gesture and Stop Play before Apply; independent per-tool state.

**Sources:** [crates/editor/src/app.rs::register_stateful_module_tool](../crates/editor/src/app.rs), [crates/project-g1/src/grid_ui.rs::draw_tool](../crates/project-g1/src/grid_ui.rs), [docs/game-ui-tools.md](game-ui-tools.md)

### Requirements

- Garden/width/height/cell size, brush rail and grid form one local draft.

- Brushes are Floor, Wall, Flower/goal, Crate, Gardener, Erase; goals may coexist with crates.

- Apply level validates and produces one Undo; stale source requires Discard, hidden drafts remain resolvable in D01.

**Actions / controls:** `Choose garden`, `Paint cells`, `Apply level`, `Discard draft`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Garden | `enum` | First steps | Select an existing G1 scene ID; switching is disabled while this tool has a modified draft. |
| Width | `number` | 8 | Grid draft width, integer 3–32; edits remain local until validated Apply level. |
| Height | `number` | 6 | Grid draft height, integer 3–32; shrinking must be validated before publication. |
| Cell size | `number` | 1.0 | Grid draft cell size, range 0.1–4.0, drag speed 0.05; one successful Apply is one Undo. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No applicable G1 game/scene: show the owner/game mismatch; never add G1 rules to an unrelated project. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Invalid board has a local validation error; stale source prevents Apply and keeps the draft until explicit Discard. |
| Restricted / read-only | Play or conflicting authoring draft blocks Apply; draft can still be explicitly discarded through its safe route. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p21.svg) · [State sheet](media/editor-ui-catalog/p21-states.svg).

<a id="p22"></a>

## P22 — Extension panel contract

**Future extension contract, not a real registration.**

**Purpose:** Define the required editor frame and lifecycle for future module-supplied panels without inventing their domain UI.

**Entry:** A real source registration contributes Windows/type-selector metadata; unavailable registrations retain only an explanatory fallback.

**Access and scope:** Installed owner, actual participation, engine_only and current authoring guards; no callback invocation for missing owners.

**Sources:** [crates/editor/src/panes.rs::ModulePanelRegistration](../crates/editor/src/panes.rs), [crates/editor/src/panes.rs::module_extension_panel](../crates/editor/src/panes.rs), [docs/game-ui-tools.md](game-ui-tools.md)

### Requirements

- Require purpose/owner/context, one supported action route and declared dirty/apply/discard behavior.

- Read-only panels cannot claim editable tools. Distinct registered stateful tools get host storage identities/revisions; multiple views of one registration must declare draft sharing rather than promise per-area state copies.

- A new first-party registration must add its own catalogue/mocks; this generic frame is not proof all future plugin layouts are designed.

**Actions / controls:** `Supported module action`, `Apply draft`, `Discard draft`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No module content supplied: explain the contract/availability, never fill the area with decorative working-looking controls. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/p22.svg) · [State sheet](media/editor-ui-catalog/p22-states.svg).

<a id="s01"></a>

## S01 — Main editor shell and workspaces

**Current surface; proposed layout.**

**Purpose:** Keep project identity, layout navigation, common commands and operation feedback unambiguous.

**Entry:** Main native editor window.

**Access and scope:** Full/Team/source rights are independent of Engine/Project/custom layout names.

**Sources:** [crates/editor/src/panes.rs::draw_workspaces](../crates/editor/src/panes.rs), [crates/editor/src/editor_workflow.rs::draw_workflow_toolbar](../crates/editor/src/editor_workflow.rs), [docs/editor-clarity-plan.md](editor-clarity-plan.md)

### Requirements

- Use named top tabs with +; proposed creation defaults to cloning the active layout after naming, presets are an alternative.

- One project identity/path/dirty badge, stable grouped Play/Pause/Stop and Save icons, real operation status.

- Menus, toolbar and shortcuts invoke the same commands; layout changes are personal state, not document authoring.

**Actions / controls:** `File`, `Edit`, `Tools`, `Windows`, `Add workspace`, `Play`, `Stop`, `Save Project`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Explain the missing selection or empty collection; offer only a supported creation or navigation route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Show authored unsaved state and pending drafts separately from layout changes and transient Play state. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Overflow tabs/secondary commands; retain project identity and an active-area switcher below 480 logical points wide or 240 high, clamped to work area. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/s01.svg) · [State sheet](media/editor-ui-catalog/s01-states.svg).

<a id="s02"></a>

## S02 — Secondary native editor window

**Implemented surface; acceptance evidence is scoped in [area-host verification](release-042-area-hosts-verification.md).**

**Purpose:** Move an editor area to another OS window while sharing one document/history.

**Entry:** Area → Move to new window or free-header tear-off. Drag to a main/floating leaf edge for docking; there is no Return command.

**Access and scope:** Same project and rights as the main window; a real OS host owns a split tree, not an embedded egui floating panel.

**Sources:** [docs/editor-clarity-plan.md](editor-clarity-plan.md), [docs/editor-ui-spec.md](editor-ui-spec.md), [crates/editor/src/panes.rs::PaneSnapshot](../crates/editor/src/panes.rs)

### Requirements

- Move one leaf rather than duplicate it; preserve stable identity and camera/navigation. Main and floating hosts use one Area command list and tree renderer; split/type choice stay inside the host.

- Area Close removes only its addressed leaf. OS × closes all current leaves of that host; other hosts/project remain open. Host ID survives closing its original first leaf. Explicit drag docking preserves leaf IDs/views. Closed leaves stay absent after save/reopen. The final-main Close explanation is disabled hover help, not a permanent menu paragraph.

- Shared changes/Undo are immediate; repaint/resource/focus lifetime is per native window, including monitor/DPI recovery.

**Actions / controls:** `Split horizontally`, `Split vertically`, `Close area · keep neighboring area`, `Move to new window`; native OS × closes the host.

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Explain the missing selection or empty collection; offer only a supported creation or navigation route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Dock/monitor restoration failure retains the area and unsaved document; show recovery in an available window. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Original split slot no longer exists or monitor is disconnected: rehome inside the visible work area without resetting content. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/s02.svg) · [State sheet](media/editor-ui-catalog/s02-states.svg).
