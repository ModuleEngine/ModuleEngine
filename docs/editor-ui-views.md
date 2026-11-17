# Embedded views and shared controls

[English](editor-ui-views.md) | [Русский](ru/editor-ui-views.md) | [Catalogue](editor-ui-catalog.md)

Complete EN/RU meaning comes from one bilingual catalogue. All pictures are design proposals, not running-editor captures. Mockup UI is English under the language contract. Eight state categories are specified per surface, including explained N/A. An existing surface does not mean its proposed layout is implemented.

<a id="v01"></a>

## V01 — GPU rendering graph

**Current surface; proposed layout.**

**Purpose:** Compose actual rendering passes and typed resources inside the selected renderer.

**Entry:** Open the renderer subgraph in Module Blueprint or Overview → rendering graph; it is not another EditorType.

**Access and scope:** Full/source, installed GPU descriptors and real PassManifest format/semantic/extent/sample compatibility.

**Sources:** [crates/render-vulkan/src/graph.rs::default_graph](../crates/render-vulkan/src/graph.rs), [crates/editor/src/app.rs::is_gpu_graph_scope](../crates/editor/src/app.rs), [docs/gpu-hlsl.md](gpu-hlsl.md)

### Requirements

- Forward example is Scene → Geometry → SSAO → Tone map → FXAA → Color encode → Frame; Deferred adds G-buffer/lighting and real resource edges.

- Expose meaning plus exact type/format/size/sample constraints at pins; raster/compute Custom HLSL keeps its declared inputs/outputs.

- Delete/restore AO or AA reconciles connected settings, validates surviving edges and preserves the last-good renderer on rejection.

**Actions / controls:** `Add GPU pass`, `Inspect resource`, `Open HLSL`, `Delete selection`, `Review preset`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No working frame output: show the missing route and compatible add list, without silently constructing a pipeline. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Connection mismatch names both ports and actual constraints; no wrong-type wire or implicit conversion is accepted. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | A removed pass is Absent, not merely Disabled; its exposure/variants must not remain live. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/v01.svg) · [State sheet](media/editor-ui-catalog/v01-states.svg).

<a id="v02"></a>

## V02 — Rendering configuration route

**Proposed surface; not implemented.**

**Purpose:** Explain rendering architecture versus allowed game settings in one coherent route.

**Entry:** Proposed Overview → Rendering details; delegates changes to the same graph/settings commands.

**Access and scope:** Architecture/exposure requires Full; Project chooses only current permitted values and variants.

**Sources:** [docs/editor-clarity-plan.md](editor-clarity-plan.md), [crates/editor/src/properties.rs::project_settings_panel](../crates/editor/src/properties.rs), [crates/editor/src/properties.rs::engine_settings_panel](../crates/editor/src/properties.rs)

### Requirements

- Identify current renderer/path, AO/AA meaning and pass presence/enabled state.

- Never show SMAA/DLAA or another API as working options without installed compatible implementations.

- Values and graph are two presentations of one document; show saved/applied/working revisions and D11 for preset replacement.

**Actions / controls:** `Open rendering graph`, `Edit exposure`, `Choose permitted AA`, `Review preset`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Current architecture | `readonly` | Vulkan module / Forward | Read the actual selected renderer; replacement is an Engine graph/preset action. |
| Ambient occlusion | `readonly` | Present · enabled · SSAO | Presence is not the same as an enabled/disabled parameter. |
| Anti-aliasing | `enum` | FXAA | Only the Engine-exposed variants may be chosen from Project. |
| Working revision | `readonly` | Separate from authored / saved revision | Pending/rejected changes must identify retained pixels. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Explain the missing selection or empty collection; offer only a supported creation or navigation route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Project/Team sees only permitted settings or an explanation; it cannot widen variants or replace the renderer. |
| Missing / stale dependency | AA pass removed: show Absent and an Engine restore route when allowed; no dead Project dropdown. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/v02.svg) · [State sheet](media/editor-ui-catalog/v02-states.svg).

<a id="v03"></a>

## V03 — Assets / Trash

**Current surface; proposed layout.**

**Purpose:** Inspect project-trash entries and restore through validated file/reference operations.

**Entry:** Assets → Trash; return preserves the asset browsing location.

**Access and scope:** Only the project-owned trash service and supported Restore command; no invented permanent-delete/Empty Trash feature.

**Sources:** [crates/editor/src/assets_ui.rs::TrashTile](../crates/editor/src/assets_ui.rs), [crates/editor/src/assets_workspace_ui.rs::assets_panel](../crates/editor/src/assets_workspace_ui.rs), [docs/asset-workspace-api.md](asset-workspace-api.md)

### Requirements

- Show original path, type, deletion time and stable trash identity.

- Restore conflicts must explain the destination/references and preserve the current files; no silent overwrite.

- Undo/Redo and restore follow the existing asset transaction contract.

**Actions / controls:** `Restore selected`, `Back to Assets`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Project trash is empty. This does not mean authored references were cleared. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Not applicable: viewing Trash has no local document draft; the Restore command resolves required save guards. |
| Error / rejected | Restore destination collision or publication failure leaves trash entry, files and references intact. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/v03.svg) · [State sheet](media/editor-ui-catalog/v03-states.svg).

<a id="v04"></a>

## V04 — Merge conflict workspace

**Current surface; proposed layout.**

**Purpose:** Resolve each actual three-way JSON conflict before applying a merged project.

**Entry:** History / Merge → Prepare merge → selected conflict.

**Access and scope:** Same source/asset validation as document restore; unresolved conflict output is not a runnable accepted project.

**Sources:** [crates/editor/src/panels.rs::history_panel](../crates/editor/src/panels.rs), [crates/project-tools/src/merge.rs](../crates/project-tools/src/merge.rs), [docs/history-assets.md](history-assets.md)

### Requirements

- Display exact JSON path and Base / Current / Other values with selected resolution.

- Keep current, Take other and Use base are explicit; array conflicts may cover a whole array under the current merger.

- Apply only after all resolutions and validation; Save result with conflicts is separately labelled and preserves the authored project.

**Actions / controls:** `Keep current`, `Take other`, `Use base`, `Apply merged project`, `Save result with conflicts`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No conflicts remains a successful merge result; show validation status before Apply. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Resolution choices modify only the merge candidate until Apply; Cancel/return preserves current authored data. |
| Error / rejected | A resolved candidate can still fail graph/schema/asset validation; preserve resolution choices and current project. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/v04.svg) · [State sheet](media/editor-ui-catalog/v04-states.svg).

<a id="v05"></a>

## V05 — Module node inspector

**Current surface; proposed layout.**

**Purpose:** Edit only the selected module/pass parameters and inspect its actual contracts.

**Entry:** Select a node in Module Blueprint; embedded side inspector, not the authored object Inspector.

**Access and scope:** Selected graph/node identity, Full/source and parameter/manifest validator.

**Sources:** [crates/editor/src/app.rs::node_inspector](../crates/editor/src/app.rs), [crates/editor/src/panels.rs::gpu_pass_controls](../crates/editor/src/panels.rs)

### Requirements

- Show node/module IDs, owner/purpose, inputs/outputs and supported structured parameters.

- Advanced raw params and GPU manifest remain exact JSON; unknown payload survives unrelated edits.

- Edit HLSL/template replacement appears only for compatible real pass types; replacing a built-in is an authored change.

**Actions / controls:** `Edit parameter`, `Inspect port`, `Open HLSL`, `Apply params`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Selected node | `readonly` | ao · gpu.ssao | Stable node/module identity in the selected graph. |
| Strength | `number` | 0.8 | Actual selected pass parameter; limits/validation come from source. |
| Inputs | `readonly` | color / normals / depth | Read actual resource declarations; do not edit types by relabelling. |
| Advanced params | `code` | { "strength": 0.8 } | Validated node params JSON; preserve opaque sibling keys. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Select a node in this graph; do not keep editing a node selected in another scope. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Selected node was deleted: clear this view selection, retain the graph and do not reuse another node’s index. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/v05.svg) · [State sheet](media/editor-ui-catalog/v05-states.svg).

<a id="v06"></a>

## V06 — Rust visual-node inspector

**Current surface; proposed layout.**

**Purpose:** Explain and edit supported scalar/command nodes while preserving unsupported text fragments.

**Entry:** Select a node in the Rust visual graph; the source/slot stays visible.

**Access and scope:** Actual supported parser node and current source policy, never arbitrary full-language AST editing.

**Sources:** [crates/editor/src/app.rs::code_inspector](../crates/editor/src/app.rs), [crates/editor/src/panels.rs::command_inspector](../crates/editor/src/panels.rs)

### Requirements

- Show selected operation and editable typed values only where supported.

- Opaque Rust fragments open as text; visual edits update the same source through its existing candidate path.

- Different Custom instances never share mutable runtime state merely because compiled code is reused.

**Actions / controls:** `Edit supported value`, `Open fragment as text`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Source / slot | `readonly` | game-controller | Selected source/slot cannot be changed by editing a node label. |
| Selected operation | `readonly` | Supported number parameter | Only an actually supported scalar/command node has a visual editor. |
| Number value | `number` | 1.0 | Typed supported parameter updates the same Rust source. |
| Text fragment | `readonly` | Open preserved Rust as text | Unsupported syntax remains text rather than fake editable fields. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Explain the missing selection or empty collection; offer only a supported creation or navigation route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Parser/compilation failure keeps editable source and previous runtime; unsupported structure is not discarded. |
| Restricted / read-only | A node not visually supported is text-only with an explanation, not a dead editable numeric field. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/v06.svg) · [State sheet](media/editor-ui-catalog/v06-states.svg).

<a id="v07"></a>

## V07 — Rust / Text mode

**Current surface; proposed layout.**

**Purpose:** Give ordinary Rust text a full-width editing view without losing its visual-mode context.

**Entry:** Custom / Rust → Text; P12 and V08 are other views of the same source.

**Access and scope:** The selected source’s actual editing and state-application policy.

**Sources:** [crates/editor/src/app.rs::code_text](../crates/editor/src/app.rs), [docs/custom-rust.md](custom-rust.md)

### Requirements

- Keep source selector, Text/Graph/Side by side and compile/working status visible.

- Use line-based diagnostics and text-owned Undo/Redo; file Save remains global and guarded.

- Switching mode never discards text or regenerates unsupported code.

**Actions / controls:** `Graph`, `Side by side`, `Apply Rust`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Empty Rust text is a real source draft to validate, not a request to remove its Custom module. |
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

**Mockups:** [Layout](media/editor-ui-catalog/v07.svg) · [State sheet](media/editor-ui-catalog/v07-states.svg).

<a id="v08"></a>

## V08 — Rust / Graph mode

**Current surface; proposed layout.**

**Purpose:** Use the supported visual Rust subset with explicit preserved text fragments.

**Entry:** Custom / Rust → Graph; selecting a visual node opens V06.

**Access and scope:** Real parser coverage and source/slot availability; graphs are not a replacement game scheduler.

**Sources:** [crates/editor/src/app.rs::code_graph](../crates/editor/src/app.rs), [docs/custom-rust.md](custom-rust.md)

### Requirements

- Label scalar/command context and incomplete visual coverage.

- Use the proposed pan/zoom/context gestures from the graph-input baseline only where this Rust graph supports the command; preserve unsupported source fragments and do not invent editable connections.

- Graph layout/selection is personal view state, not a new source candidate.

**Actions / controls:** `Text`, `Side by side`, `Inspect node`, `Open text fragment`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No supported visual nodes: show the preserved source fragment and Text route, not a blank lost program. |
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

**Mockups:** [Layout](media/editor-ui-catalog/v08.svg) · [State sheet](media/editor-ui-catalog/v08-states.svg).

<a id="v09"></a>

## V09 — Custom state recovery

**Current surface; proposed layout.**

**Purpose:** Explicitly resolve incompatible hot-reload state without replacing the working instance accidentally.

**Entry:** Rejected Custom candidate → Apply with state reset or Explicit state migration (JSON); embedded in Rust. Game instances use the separate Explicit Game state migration (JSON) worker route.

**Access and scope:** Candidate must match current source/slot; actual game/non-game reset and migration support are separate.

**Sources:** [crates/editor/src/app.rs::custom_panel](../crates/editor/src/app.rs), [crates/editor/src/game_ui.rs::custom_editor_ui](../crates/editor/src/game_ui.rs), [docs/custom-state.md](custom-state.md)

### Requirements

- Show working/candidate revision and the incompatibility before reset or migration.

- Both current non-game Scripts slots and Game instances expose reset and explicit JSON migration. Bind the actions/text to the selected host/slot and candidate; do not share Game worker state with preview Scripts.

- Invalid migration or stale candidate preserves old state/code and editable migration text.

**Actions / controls:** `Keep working version`, `Apply with state reset`, `Apply with migrated state`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Selected slot | `readonly` | preview-update · non-game Scripts | Host and instance identity determine the target; Game worker instances are a separate route with independent state. |
| Working | `readonly` | revision 12 · previous state retained | Read the active slot snapshot; keep it running until the new candidate is explicitly accepted. |
| Rejected candidate | `readonly` | revision 13 · state shape incompatible | Keep the rejected source and incompatibility visible; a stale candidate cannot replace the current open source. |
| Explicit migration JSON | `code` | { "timer": 0.0 } | Edit migration JSON for this slot; validate syntax and candidate state shape before replacement. Failure preserves the working code/state and this draft. |
| Game route | `readonly` | Separate Game instance · reset or explicit JSON migration | Game requests use custom_editor_ui and the Game worker’s request/session checks; non-game requests use Scripts. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No rejected state candidate: hide recovery actions; ordinary compatible Apply remains the route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Migration JSON/ABI/state validation failed: keep the previous running slot and entered migration. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Candidate belongs to an older source: require applying current source first; disable reset/migrate for the stale candidate. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/v09.svg) · [State sheet](media/editor-ui-catalog/v09-states.svg).

<a id="v10"></a>

## V10 — Architecture map

**Proposed surface; not implemented.**

**Purpose:** Inspect source-backed capability dependencies as an optional supplement to Overview.

**Entry:** Proposed later stage 6 → Architecture map; no current EditorType registration is claimed.

**Access and scope:** Read-only baseline; actual navigation respects tool/source rights and owner evidence.

**Sources:** [docs/editor-clarity-plan.md](editor-clarity-plan.md), [docs/engine-composition-direction.md](engine-composition-direction.md), [docs/engine-capabilities.md](engine-capabilities.md)

### Requirements

- Derive nodes/relationships from current metadata; distinguish Engine, Gameplay, Project and unknown.

- Explain dependency/removal consequences without pretending every library/service is an executable graph node.

- Open the same Overview details/tools; no independent configuration storage or multi-engine management.

**Actions / controls:** `Inspect relationship`, `Open capability details`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Display source-backed facts and supported navigation; viewing has no authoring or initialization side effects. |
| Empty / no selection | No source-backed relationships: explain incomplete metadata, do not infer that the engine has no dependencies. |
| Busy / preparing | Use available snapshots and identify pending evidence; reading must not launch a factory, service or compiler. |
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

**Mockups:** [Layout](media/editor-ui-catalog/v10.svg) · [State sheet](media/editor-ui-catalog/v10-states.svg).

<a id="c01"></a>

## C01 — Schema-driven property fields

**Shared control family; proposed standard.**

**Purpose:** Provide consistent editable values without conflating PropertySchema with ComponentSchema or losing authored semantics.

**Entry:** Project/Engine settings, applicable object fields and component inspectors.

**Access and scope:** Exact owner/schema/SDK pins, scope, type/bounds and exposed capability; field visibility follows actual composition.

**Sources:** [crates/engine-core/src/settings.rs::PropertyType](../crates/engine-core/src/settings.rs), [crates/editor/src/properties.rs::value_editor](../crates/editor/src/properties.rs), [docs/component-schemas.md](component-schemas.md)

### Requirements

- Cover all current PropertyType variants: Number, Boolean, Enum, Text, ObjectReference and AssetReference.

- Number is finite within inclusive bounds; Text max_len is UTF-8 bytes, not a character-count promise.

- Optional references distinguish null from missing key/default. Component fields use their source schema and preserve unknown keys/payload.

- No-op Apply does not write overrides; inherited/effective/authored states have explicit labels and reset semantics.

**Actions / controls:** `Edit value`, `Choose enum`, `Clear optional reference`, `Use inherited/default value`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Exposure | `number` | 1.0 | Number: finite inclusive min/max from the source schema. |
| Enabled | `boolean` | True | Boolean is a JSON boolean, not text or a tri-state null. |
| Anti-aliasing | `enum` | FXAA | Enum values come from the current allowed choices. |
| Display label | `text` | Example label | Text max_len is UTF-8 bytes. |
| Object reference | `enum` | Hero · ID 1 | ObjectReference stores stable numeric ID; optional clear stores null only when allowed. |
| Asset reference | `enum` | hero.glb · asset://stable-id | AssetReference preserves stable IDs and supported legacy paths. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Missing key/default and explicit null remain different visible data states; never coerce unknown input to zero. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Show the exact bound/type/path failure at the field and keep the authored value and opaque sibling keys. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/c01.svg) · [State sheet](media/editor-ui-catalog/c01-states.svg).

<a id="c02"></a>

## C02 — Object/asset reference pickers

**Shared control family; proposed standard.**

**Purpose:** Choose a valid stable reference while preserving missing/unknown IDs and legacy paths.

**Entry:** Source-owned reference field, asset details, replacement or drag/drop target.

**Access and scope:** Object candidates satisfy required components in the resolved scene; asset candidates satisfy kind/extension and dependency checks.

**Sources:** [crates/editor/src/game_ui.rs::reference_editor](../crates/editor/src/game_ui.rs), [crates/editor/src/properties.rs::value_editor](../crates/editor/src/properties.rs), [docs/asset-workspace-api.md](asset-workspace-api.md)

### Requirements

- Label numeric legacy-reference input versus richer schema-aware picker; they are distinct current implementations.

- Show names plus exact IDs/path; optional None means authored null only when permitted.

- Known empty candidate set differs from unavailable enumeration; no dangling ID is erased merely because it is not listed.

**Actions / controls:** `Choose compatible object`, `Choose compatible asset`, `Clear optional value`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Legacy object ID | `number` | 1 | Legacy exposed property editor uses validated numeric identity. |
| Scene object | `enum` | Hero · ID 1 | Schema-aware choices filter required components in this scene. |
| Model asset | `enum` | hero.glb · asset://stable-id | Filter by allowed kind and preserve missing current references. |
| Optional value | `enum` | None (authored null) | None is available only for optional references. |
| Legacy asset path | `text` | assets/models/hero.glb | Preserve supported legacy path representation. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No compatible candidates: explain the filter and offer existing navigation/import only; required reference cannot be cleared. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Missing current reference remains shown as unresolved; if enumeration itself failed show Unknown, not an empty picker. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/c02.svg) · [State sheet](media/editor-ui-catalog/c02-states.svg).

<a id="c03"></a>

## C03 — Icon / pin help and feedback

**Shared control family; proposed standard.**

**Purpose:** Explain standard controls, actual connection constraints and operation state without permanent jargon clutter.

**Entry:** Hover or keyboard focus on icons/pins; selected mismatch or operation status exposes persistent critical details.

**Access and scope:** Help reads actual command/port state and cannot itself invoke, activate or widen a capability.

**Sources:** [docs/editor-ui-spec.md](editor-ui-spec.md), [crates/editor/src/app.rs::graph_connection_error](../crates/editor/src/app.rs), [crates/editor/src/activity.rs::draw_activity](../crates/editor/src/activity.rs)

### Requirements

- Action + real shortcut + effect + disabled reason; semantic accessible name/state exists beyond tooltip text.

- Pin details include direction/type/meaning and actual format/extent/sample constraints; no generic texture compatibility shortcut.

- Delay 500 ms for hover, immediate focus help, maximum 320 points; hoverable, Escape-dismissable, no critical error only on hover.

- This shared-component sheet shows labelled non-executable control specimens; omit the shell Game/Save controller so samples cannot read as a second live toolbar. Different visual states are examples, not duplicated commands.

**Actions / controls:** `Show control help`, `Inspect connection mismatch`, `Open diagnostics`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Explain the missing selection or empty collection; offer only a supported creation or navigation route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Not applicable: help owns no authored draft; descriptions reflect the current candidate/command without changing it. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Flip/clamp help inside work area and wrap to 320 points; do not cover a captured gizmo or hide the focused control. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/c03.svg) · [State sheet](media/editor-ui-catalog/c03-states.svg).

<a id="c04"></a>

## C04 — Transforms, colour and standard controls

**Shared control family; proposed standard.**

**Purpose:** Specify common interactive fields, tree/list controls and colour popups shared by editor surfaces.

**Entry:** Inspector and compatible legacy properties; standard controls throughout the editor.

**Access and scope:** Actual field type/ranges and renderer/material support; a colour popup is not a new PropertyType variant or transparent-material feature.

**Sources:** [crates/editor/src/game_ui.rs::inspector_ui_with_composition](../crates/editor/src/game_ui.rs), [crates/editor/src/scene_tools.rs](../crates/editor/src/scene_tools.rs), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- Vector axes use readable X/Y/Z labels and finite values; displayed rotation units are explicit, current stored rotations remain radians.

- Define normal/hover/pressed/focus/selected/disabled states for buttons, toggles, radios, dropdowns, trees, tiles and splitters.

- Colour popup shows stored colour values and explicit apply/gesture history; do not imply unsupported alpha blending or HDR pipelines.

- Selection, scrolling and expanding sections are view state; they do not create local component overrides.

- This shared-component sheet shows labelled non-executable control specimens; omit the shell Game/Save controller so samples cannot read as a second live toolbar. Different visual states are examples, not duplicated commands.

**Actions / controls:** `Edit transform`, `Choose stored colour`, `Expand section`, `Select list item`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Position | `vector3` | 0.00 / 0.00 / 5.00 | Separate finite X/Y/Z controls. |
| Rotation (radians) | `vector3` | 0.00 / 0.00 / 0.00 | Rotation units stay explicit. |
| Scale | `vector3` | 1.00 / 1.00 / 1.00 | Separate source-valid axis values. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Explain the missing selection or empty collection; offer only a supported creation or navigation route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | One drag/colour gesture uses the owning field’s transaction; text editing and Escape rollback preserve the exact previous value. |
| Error / rejected | Invalid/nonfinite transform or unsupported colour field leaves authored values unchanged and shows a local reason. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/c04.svg) · [State sheet](media/editor-ui-catalog/c04-states.svg).

<a id="v11"></a>

## V11 — GPU graph / Deferred example

**Current surface; proposed layout.**

**Purpose:** Show the Deferred data path without treating every renderer as a fixed G-buffer pipeline.

**Entry:** Same GPU subgraph editor as V01, with the actual selected Deferred configuration.

**Access and scope:** The same Full/source/resource-contract rules; a preset is not a harmless view toggle.

**Sources:** [crates/render-vulkan/src/graph.rs::default_graph](../crates/render-vulkan/src/graph.rs), [docs/gpu-hlsl.md](gpu-hlsl.md)

### Requirements

- G-buffer produces albedo/normals/depth consumed by Deferred lighting; SSAO also uses real normals/depth edges.

- Remaining Tone map → FXAA → Color encode → Frame matches the default graph, with real manifest checks.

- Custom renderer graphs may have other shapes/resources within the currently supported contracts; do not imply arbitrary buffer/Nanite support.

**Actions / controls:** `Inspect pass`, `Inspect resource`, `Open HLSL`, `Review preset`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Explain the missing selection or empty collection; offer only a supported creation or navigation route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | A layout may look connectable but still fail semantic/format/extent/sample checks; previous renderer remains labelled and working. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Missing G-buffer input/lighting output is a validation failure with port identity, not an automatic switch to Forward. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/v11.svg) · [State sheet](media/editor-ui-catalog/v11-states.svg).

<a id="v12"></a>

## V12 — Rust / scalar graph example

**Current surface; proposed layout.**

**Purpose:** Distinguish supported scalar expressions from game-command graphs and execution schedules.

**Entry:** Custom / Rust → compatible scalar source → Graph or Side by side.

**Access and scope:** Real parser subset and slot contract only; this is not arbitrary Rust structural editing.

**Sources:** [crates/editor/src/app.rs::code_graph](../crates/editor/src/app.rs), [crates/editor/src/scalar.rs](../crates/editor/src/scalar.rs), [docs/custom-rust.md](custom-rust.md)

### Requirements

- Show typed numeric/time/operator flow and its corresponding preserved source.

- Unrecognized constructs remain text fragments; graph navigation does not create a code candidate.

- Compile/apply follows the selected scalar/update source lifecycle, not Game command/state assumptions.

**Actions / controls:** `Open text`, `Inspect expression`, `Apply Rust`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Source is not visually representable: retain text and explain coverage instead of rendering game-command nodes. |
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

**Mockups:** [Layout](media/editor-ui-catalog/v12.svg) · [State sheet](media/editor-ui-catalog/v12-states.svg).

<a id="v13"></a>

## V13 — Gameplay execution JSON

**Current surface; proposed layout.**

**Purpose:** Edit the execution resource/stage JSON draft as another view of the Gameplay Systems schedule.

**Entry:** Gameplay Systems → advanced execution JSON; return to the same visual draft.

**Access and scope:** Frozen Team schedule, Full/source and Stop-Play requirements remain; raw JSON is not a bypass.

**Sources:** [crates/editor/src/game_ui.rs::systems_ui](../crates/editor/src/game_ui.rs), [docs/module-execution.md](module-execution.md)

### Requirements

- Scope is GameProject.execution, not the whole GraphDocument or arbitrary Rust/HLSL.

- Show syntax and semantic binding/dependency errors; Apply validates the shared draft, Discard restores authored schedule.

- Text Undo stays local; document Save cannot accept an unresolved schedule draft implicitly.

**Actions / controls:** `Apply schedule`, `Discard draft`, `Return to graph`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | Explain the missing selection or empty collection; offer only a supported creation or navigation route. |
| Busy / preparing | Show the real pending operation and last-good revision; prevent conflicting edits without blocking navigation. |
| Draft / modified | Mark local edits separately from saved/applied data; commit only through the existing validated transaction. |
| Error / rejected | Invalid resource ID/type or after dependency keeps the current executor and exact editable execution text. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/v13.svg) · [State sheet](media/editor-ui-catalog/v13-states.svg).
