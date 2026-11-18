# Dialogs and native picker purposes

[English](editor-ui-dialogs.md) | [Русский](ru/editor-ui-dialogs.md) | [Catalogue](editor-ui-catalog.md)

Complete EN/RU meaning comes from one bilingual catalogue. All pictures are design proposals, not running-editor captures. Mockup UI is English under the language contract. Eight state categories are specified per surface, including explained N/A. An existing surface does not mean its proposed layout is implemented.

<a id="n01"></a>

## N01 — Open project file

**Current surface; proposed layout.**

**Purpose:** Select an existing project JSON without replacing the current document until loading and validation succeed.

**Entry:** File/Tools/Assets command → native picker worker; contextual title/filter/directory.

**Access and scope:** Current command guards plus OS filesystem permissions; native implementation currently Windows only.

**Sources:** [crates/editor/src/native_dialogs.rs::open_project](../crates/editor/src/native_dialogs.rs), [crates/editor/src/editor_workflow.rs::start_dialog](../crates/editor/src/editor_workflow.rs), [docs/editor-workflow.md](editor-workflow.md)

### Requirements

- Open/recent transition resolves authored and tool drafts before publication.

- A missing/bad/incompatible file preserves current project, resources, layout and bindings; Recent changes only on success.

- Cancel, Unicode/long paths and failed publication preserve the previous project and do not change process working directory.

**Actions / controls:** `Confirm OS selection`, `Cancel`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | The OS owns picker layout/language; the application supplies the title, filter and current directory hint. |
| Empty / no selection | An empty directory is valid unless this purpose requires a selected file; selection rules belong to the native provider. |
| Busy / preparing | One background dialog worker owns the request; keep the editor responsive and prevent a second conflicting picker. |
| Draft / modified | Typed path/name is tentative; Cancel returns no selected path and performs no application file operation. |
| Error / rejected | Native creation/path/permission errors become the operation report; previous project and Recent state remain intact. |
| Restricted / read-only | On unsupported platforms report unavailable native dialogs honestly; no mockup establishes a Linux implementation. |
| Missing / stale dependency | A path disappearing after selection is revalidated by the actual command; no stale selection is trusted. |
| Narrow / short area | Native placement/DPI/accessibility belong to the provider; our schematic does not prescribe OS pixel measurements. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/n01.svg) · [State sheet](media/editor-ui-catalog/n01-states.svg).

<a id="n02"></a>

## N02 — Save Project As

**Current surface; proposed layout.**

**Purpose:** Choose a standalone authored project copy with its resources, then continue with the copy only after success.

**Entry:** File/Tools/Assets command → native picker worker; contextual title/filter/directory.

**Access and scope:** Current command guards plus OS filesystem permissions; native implementation currently Windows only.

**Sources:** [crates/editor/src/native_dialogs.rs::save_project](../crates/editor/src/native_dialogs.rs), [crates/editor/src/editor_workflow.rs::start_dialog](../crates/editor/src/editor_workflow.rs), [docs/editor-workflow.md](editor-workflow.md)

### Requirements

- Preserve exact IDs/unknown JSON and the installed Engine binding rules; this is not Full-source export.

- OS overwrite confirmation N08 does not waive later destination/race/resource checks.

- Cancel, Unicode/long paths and failed publication preserve the previous project and do not change process working directory.

**Actions / controls:** `Confirm OS selection`, `Cancel`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | The OS owns picker layout/language; the application supplies the title, filter and current directory hint. |
| Empty / no selection | An empty directory is valid unless this purpose requires a selected file; selection rules belong to the native provider. |
| Busy / preparing | One background dialog worker owns the request; keep the editor responsive and prevent a second conflicting picker. |
| Draft / modified | Typed path/name is tentative; Cancel returns no selected path and performs no application file operation. |
| Error / rejected | Native creation/path/permission errors become the operation report; previous project and Recent state remain intact. |
| Restricted / read-only | On unsupported platforms report unavailable native dialogs honestly; no mockup establishes a Linux implementation. |
| Missing / stale dependency | A path disappearing after selection is revalidated by the actual command; no stale selection is trusted. |
| Narrow / short area | Native placement/DPI/accessibility belong to the provider; our schematic does not prescribe OS pixel measurements. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/n02.svg) · [State sheet](media/editor-ui-catalog/n02-states.svg).

<a id="n03"></a>

## N03 — New project destination

**Current surface; proposed layout.**

**Purpose:** Choose a new/empty destination for the selected supported template and composition preset.

**Entry:** File/Tools/Assets command → native picker worker; contextual title/filter/directory.

**Access and scope:** Current command guards plus OS filesystem permissions; native implementation currently Windows only.

**Sources:** [crates/editor/src/native_dialogs.rs::pick_folder](../crates/editor/src/native_dialogs.rs), [crates/editor/src/editor_workflow.rs::start_dialog](../crates/editor/src/editor_workflow.rs), [docs/editor-workflow.md](editor-workflow.md)

### Requirements

- Current New menu choices are inventoried; describe alternative Engine labels as composition presets of one ModuleEngine.

- Creating a project never edits the previous project or installs an omitted optional package.

- Cancel, Unicode/long paths and failed publication preserve the previous project and do not change process working directory.

**Actions / controls:** `Confirm OS selection`, `Cancel`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | The OS owns picker layout/language; the application supplies the title, filter and current directory hint. |
| Empty / no selection | An empty directory is valid unless this purpose requires a selected file; selection rules belong to the native provider. |
| Busy / preparing | One background dialog worker owns the request; keep the editor responsive and prevent a second conflicting picker. |
| Draft / modified | Typed path/name is tentative; Cancel returns no selected path and performs no application file operation. |
| Error / rejected | Native creation/path/permission errors become the operation report; previous project and Recent state remain intact. |
| Restricted / read-only | On unsupported platforms report unavailable native dialogs honestly; no mockup establishes a Linux implementation. |
| Missing / stale dependency | A path disappearing after selection is revalidated by the actual command; no stale selection is trusted. |
| Narrow / short area | Native placement/DPI/accessibility belong to the provider; our schematic does not prescribe OS pixel measurements. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/n03.svg) · [State sheet](media/editor-ui-catalog/n03-states.svg).

<a id="n04"></a>

## N04 — Windows Player destination

**Current surface; proposed layout.**

**Purpose:** Choose an export folder for the selected standalone Windows game.

**Entry:** File/Tools/Assets command → native picker worker; contextual title/filter/directory.

**Access and scope:** Current command guards plus OS filesystem permissions; native implementation currently Windows only.

**Sources:** [crates/editor/src/native_dialogs.rs::pick_folder](../crates/editor/src/native_dialogs.rs), [crates/editor/src/editor_workflow.rs::start_dialog](../crates/editor/src/editor_workflow.rs), [docs/editor-workflow.md](editor-workflow.md)

### Requirements

- Player has no editor/compiler requirement; host build prerequisites and selected native features still must be validated.

- Use only this project’s working graph/package selection; do not add every gameplay package for convenience.

- Cancel, Unicode/long paths and failed publication preserve the previous project and do not change process working directory.

**Actions / controls:** `Confirm OS selection`, `Cancel`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | The OS owns picker layout/language; the application supplies the title, filter and current directory hint. |
| Empty / no selection | An empty directory is valid unless this purpose requires a selected file; selection rules belong to the native provider. |
| Busy / preparing | One background dialog worker owns the request; keep the editor responsive and prevent a second conflicting picker. |
| Draft / modified | Typed path/name is tentative; Cancel returns no selected path and performs no application file operation. |
| Error / rejected | Native creation/path/permission errors become the operation report; previous project and Recent state remain intact. |
| Restricted / read-only | On unsupported platforms report unavailable native dialogs honestly; no mockup establishes a Linux implementation. |
| Missing / stale dependency | A path disappearing after selection is revalidated by the actual command; no stale selection is trusted. |
| Narrow / short area | Native placement/DPI/accessibility belong to the provider; our schematic does not prescribe OS pixel measurements. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/n04.svg) · [State sheet](media/editor-ui-catalog/n04-states.svg).

<a id="n05"></a>

## N05 — Team editor destination

**Current surface; proposed layout.**

**Purpose:** Choose a reduced editor package preserving frozen Engine and allowed Project editing.

**Entry:** File/Tools/Assets command → native picker worker; contextual title/filter/directory.

**Access and scope:** Current command guards plus OS filesystem permissions; native implementation currently Windows only.

**Sources:** [crates/editor/src/native_dialogs.rs::pick_folder](../crates/editor/src/native_dialogs.rs), [crates/editor/src/editor_workflow.rs::start_dialog](../crates/editor/src/editor_workflow.rs), [docs/editor-workflow.md](editor-workflow.md)

### Requirements

- Team is not a source-authoring Full build; visible layouts do not unlock architecture or frozen schedules.

- Do not claim all Team packages are compiler-free; game Custom capabilities follow the actual package/SDK contract.

- Cancel, Unicode/long paths and failed publication preserve the previous project and do not change process working directory.

**Actions / controls:** `Confirm OS selection`, `Cancel`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | The OS owns picker layout/language; the application supplies the title, filter and current directory hint. |
| Empty / no selection | An empty directory is valid unless this purpose requires a selected file; selection rules belong to the native provider. |
| Busy / preparing | One background dialog worker owns the request; keep the editor responsive and prevent a second conflicting picker. |
| Draft / modified | Typed path/name is tentative; Cancel returns no selected path and performs no application file operation. |
| Error / rejected | Native creation/path/permission errors become the operation report; previous project and Recent state remain intact. |
| Restricted / read-only | On unsupported platforms report unavailable native dialogs honestly; no mockup establishes a Linux implementation. |
| Missing / stale dependency | A path disappearing after selection is revalidated by the actual command; no stale selection is trusted. |
| Narrow / short area | Native placement/DPI/accessibility belong to the provider; our schematic does not prescribe OS pixel measurements. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/n05.svg) · [State sheet](media/editor-ui-catalog/n05-states.svg).

<a id="n06"></a>

## N06 — Full source destination

**Current surface; proposed layout.**

**Purpose:** Choose a complete matching source-project delivery with documentation and required source inventory.

**Entry:** File/Tools/Assets command → native picker worker; contextual title/filter/directory.

**Access and scope:** Current command guards plus OS filesystem permissions; native implementation currently Windows only.

**Sources:** [crates/editor/src/native_dialogs.rs::pick_folder](../crates/editor/src/native_dialogs.rs), [crates/editor/src/editor_workflow.rs::start_dialog](../crates/editor/src/editor_workflow.rs), [docs/editor-workflow.md](editor-workflow.md)

### Requirements

- Source completeness and SDK pins must really validate; export cannot grant rights by rewriting a profile label.

- Tool downloads may be required; no promise of an offline toolchain bundle or changed GPU drivers.

- Cancel, Unicode/long paths and failed publication preserve the previous project and do not change process working directory.

**Actions / controls:** `Confirm OS selection`, `Cancel`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | The OS owns picker layout/language; the application supplies the title, filter and current directory hint. |
| Empty / no selection | An empty directory is valid unless this purpose requires a selected file; selection rules belong to the native provider. |
| Busy / preparing | One background dialog worker owns the request; keep the editor responsive and prevent a second conflicting picker. |
| Draft / modified | Typed path/name is tentative; Cancel returns no selected path and performs no application file operation. |
| Error / rejected | Native creation/path/permission errors become the operation report; previous project and Recent state remain intact. |
| Restricted / read-only | On unsupported platforms report unavailable native dialogs honestly; no mockup establishes a Linux implementation. |
| Missing / stale dependency | A path disappearing after selection is revalidated by the actual command; no stale selection is trusted. |
| Narrow / short area | Native placement/DPI/accessibility belong to the provider; our schematic does not prescribe OS pixel measurements. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/n06.svg) · [State sheet](media/editor-ui-catalog/n06-states.svg).

<a id="n07"></a>

## N07 — Import asset file

**Current surface; proposed layout.**

**Purpose:** Choose one file supported by the current model/texture/audio importer for the target project folder.

**Entry:** File/Tools/Assets command → native picker worker; contextual title/filter/directory.

**Access and scope:** Current command guards plus OS filesystem permissions; native implementation currently Windows only.

**Sources:** [crates/editor/src/native_dialogs.rs::import_asset](../crates/editor/src/native_dialogs.rs), [crates/editor/src/editor_workflow.rs::perform_asset_action](../crates/editor/src/editor_workflow.rs), [docs/editor-workflow.md](editor-workflow.md)

### Requirements

- Picker filters reflect actual import support, not every possible shader/model extension.

- Import failure preserves existing files/references and reports unsupported content honestly.

- Cancel, Unicode/long paths and failed publication preserve the previous project and do not change process working directory.

**Actions / controls:** `Confirm OS selection`, `Cancel`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | The OS owns picker layout/language; the application supplies the title, filter and current directory hint. |
| Empty / no selection | An empty directory is valid unless this purpose requires a selected file; selection rules belong to the native provider. |
| Busy / preparing | One background dialog worker owns the request; keep the editor responsive and prevent a second conflicting picker. |
| Draft / modified | Typed path/name is tentative; Cancel returns no selected path and performs no application file operation. |
| Error / rejected | Native creation/path/permission errors become the operation report; previous project and Recent state remain intact. |
| Restricted / read-only | On unsupported platforms report unavailable native dialogs honestly; no mockup establishes a Linux implementation. |
| Missing / stale dependency | A path disappearing after selection is revalidated by the actual command; no stale selection is trusted. |
| Narrow / short area | Native placement/DPI/accessibility belong to the provider; our schematic does not prescribe OS pixel measurements. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/n07.svg) · [State sheet](media/editor-ui-catalog/n07-states.svg).

<a id="n08"></a>

## N08 — OS overwrite confirmation

**Current surface; proposed layout.**

**Purpose:** Confirm an existing Save As destination through the native provider, followed by the normal guarded transaction.

**Entry:** Nested inside native Save As when its provider detects an existing file.

**Access and scope:** OS-owned confirmation and language; app rechecks destination/asset/binding safety after selection.

**Sources:** [crates/editor/src/native_dialogs.rs::save_project](../crates/editor/src/native_dialogs.rs), [crates/editor/src/workspace_files.rs::save_as_confirmed](../crates/editor/src/workspace_files.rs)

### Requirements

- Show the exact destination; only explicit provider confirmation enables the confirmed-overwrite code path.

- Cancel leaves the original file and active project unchanged.

- A changed destination after confirmation rejects publication rather than overwriting unexpected data.

**Actions / controls:** `Confirm overwrite in OS dialog`, `Cancel`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact existing destination and the OS provider’s overwrite confirmation. The outer Save As flow owns destination selection and later validation. |
| Empty / no selection | Not applicable: overwrite confirmation has no empty-folder selection. A tentative or missing destination belongs to the outer Save As flow. |
| Busy / preparing | One background dialog worker owns the request; keep the editor responsive and prevent a second conflicting picker. |
| Draft / modified | Not applicable: the confirmation edits no authored/form draft; the outer native Save As retains its tentative path. |
| Error / rejected | Native creation/path/permission errors become the operation report; previous project and Recent state remain intact. |
| Restricted / read-only | On unsupported platforms report unavailable native dialogs honestly; no mockup establishes a Linux implementation. |
| Missing / stale dependency | Destination changed/disappeared after consent: the guarded publication revalidates it; consent is not a blanket overwrite permission. |
| Narrow / short area | Native placement/DPI/accessibility belong to the provider; our schematic does not prescribe OS pixel measurements. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/n08.svg) · [State sheet](media/editor-ui-catalog/n08-states.svg).

<a id="d01"></a>

## D01 — Unsaved changes / draft resolution

**Current surface; proposed layout.**

**Purpose:** Resolve the current project and local tool drafts before a guarded transition.

**Entry:** Open/New/Quit/Check/export/file commands when the existing save guard requires it.

**Access and scope:** Existing action-specific guard; asset operations cannot use a discard/unsaved-version bypass.

**Sources:** [crates/editor/src/editor_workflow.rs::draw_workflow_modals](../crates/editor/src/editor_workflow.rs), [crates/editor/src/panes.rs::module_tool_draft_resolutions_ui](../crates/editor/src/panes.rs)

### Requirements

- Name the pending action and current project, and list unresolved JSON/schedule/module-tool drafts.

- Save and continue is unavailable until tool drafts are applied/discarded. Hidden/inactive tool drafts support explicit Discard.

- Use saved version is for Check/export, Don’t save for eligible transitions; neither appears as an asset-transaction escape hatch.

**Actions / controls:** `Save and continue`, `Use saved version / Don’t save when permitted`, `Cancel`, `Discard module draft`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No unresolved drafts: show the ordinary guarded save choice; no dummy draft rows. |
| Busy / preparing | Save is pending: prevent repeated transition; on failure remain on the same project/action and show the error. |
| Draft / modified | Explicitly discarding one hidden module draft resolves only that tool, not all other project changes. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Escape/close means Cancel. Enter may activate only the focused enabled action, never an unsafe implicit discard.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d01.svg) · [State sheet](media/editor-ui-catalog/d01-states.svg).

<a id="d02"></a>

## D02 — Operation result / report

**Current surface; proposed layout.**

**Purpose:** Explain the actual command outcome and preserve selectable details and safe recovery.

**Entry:** Tools → Last operation report or operation feedback; not every informational event needs a modal.

**Access and scope:** Read-only result; any linked retry/open action retains its own real rights/validation.

**Sources:** [crates/editor/src/editor_workflow.rs::draw_workflow_modals](../crates/editor/src/editor_workflow.rs), [crates/editor/src/activity.rs::draw_activity](../crates/editor/src/activity.rs)

### Requirements

- Show command, success/warning/failure, affected output path and actual last-good state.

- Expandable selectable/copyable full diagnostics; preserve compiler/OS text verbatim inside labelled details.

- Closing a report is not Cancel/Undo of the completed operation; unresolved module drafts remain explicit.

**Actions / controls:** `Copy details`, `Close`, `Open relevant tool when supported`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Selected data and actual command availability are visible. |
| Empty / no selection | No previous report: disable Last operation report with a reason, not an empty success dialog. |
| Busy / preparing | An operation still running is shown by activity/status; do not call a partial report a completed success. |
| Draft / modified | Not applicable: result text is not an editable project draft; unresolved module drafts use the explicit resolver rows. |
| Error / rejected | Show the rejected candidate and actionable error; retain authored data, draft and last-good runtime as the applicable contract requires. |
| Restricted / read-only | Show the exact Full/Team/source/Play restriction; no workspace tab grants permission. |
| Missing / stale dependency | Keep stable IDs and missing references inspectable; never silently replace or activate a dependency. |
| Narrow / short area | Stack secondary sections or use explicit overflow; preserve scope, primary action, error and navigation without shrinking text. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d02.svg) · [State sheet](media/editor-ui-catalog/d02-states.svg).

<a id="d03"></a>

## D03 — New folder

**Current surface; proposed layout.**

**Purpose:** Create one validated leaf folder inside the writable asset root.

**Entry:** Assets toolbar/context/detail action for the selected stable asset ID.

**Access and scope:** Actual asset protection, owner root, busy lock and saved reference-transaction rules.

**Sources:** [crates/editor/src/assets_ui.rs::BrowserDialog](../crates/editor/src/assets_ui.rs), [crates/editor/src/assets_ui.rs::dialog_ui](../crates/editor/src/assets_ui.rs), [docs/asset-workspace-api.md](asset-workspace-api.md)

### Requirements

- Reject empty/invalid leaf names and collisions; do not interpret a folder name as an escape path.

- Showing the form creates no directory until confirmed.

**Actions / controls:** `Create`, `Cancel`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Folder name | `text` | Props | One validated leaf name. |
| Parent folder | `readonly` | assets/models | Existing writable folder context, not an editable escape path. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | Required input missing: primary action disabled with an inline reason; Cancel remains available. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | Keep entered form values local until confirmation; closing cancels the uncommitted request. |
| Error / rejected | Preserve input and previous data on failure; show the exact validation/operation error and safe retry. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | Selected asset/destination became unavailable: disable confirmation, keep its ID and entered values for correction/Cancel. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d03.svg) · [State sheet](media/editor-ui-catalog/d03-states.svg).

<a id="d04"></a>

## D04 — Rename asset

**Current surface; proposed layout.**

**Purpose:** Rename a known file while preserving stable ID and validated references.

**Entry:** Assets toolbar/context/detail action for the selected stable asset ID.

**Access and scope:** Actual asset protection, owner root, busy lock and saved reference-transaction rules.

**Sources:** [crates/editor/src/assets_ui.rs::BrowserDialog](../crates/editor/src/assets_ui.rs), [crates/editor/src/assets_ui.rs::dialog_ui](../crates/editor/src/assets_ui.rs), [docs/asset-workspace-api.md](asset-workspace-api.md)

### Requirements

- Keep the extension and show the full old/new names; protected/unchanged names cannot be confirmed.

- Save first; cancelling save cancels rename, and publication errors preserve file/reference consistency.

**Actions / controls:** `Rename`, `Cancel`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Current asset | `readonly` | assets/models/crate.glb | Exact target retained until successful rename. |
| New name | `text` | crate-large.glb | Keep extension and validate collision/protection. |
| Stable ID | `readonly` | asset://stable-id | Rename never edits the asset ID. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | Required input missing: primary action disabled with an inline reason; Cancel remains available. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | Keep entered form values local until confirmation; closing cancels the uncommitted request. |
| Error / rejected | Preserve input and previous data on failure; show the exact validation/operation error and safe retry. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | Selected asset/destination became unavailable: disable confirmation, keep its ID and entered values for correction/Cancel. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d04.svg) · [State sheet](media/editor-ui-catalog/d04-states.svg).

<a id="d05"></a>

## D05 — Move asset

**Current surface; proposed layout.**

**Purpose:** Choose an allowed destination folder for a known asset file.

**Entry:** Assets toolbar/context/detail action for the selected stable asset ID.

**Access and scope:** Actual asset protection, owner root, busy lock and saved reference-transaction rules.

**Sources:** [crates/editor/src/assets_ui.rs::BrowserDialog](../crates/editor/src/assets_ui.rs), [crates/editor/src/assets_ui.rs::dialog_ui](../crates/editor/src/assets_ui.rs), [docs/asset-workspace-api.md](asset-workspace-api.md)

### Requirements

- Show source/destination; same folder, protected file or invalid target disables Move.

- Retain asset identity and all supported links through the saved transaction; moving folders is not implied by this file-only command.

**Actions / controls:** `Move`, `Cancel`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Asset | `readonly` | assets/models/crate.glb | Exact source target. |
| Destination folder | `enum` | assets/models/props | Choose a supported destination inside the writable project root. |
| Stable ID | `readonly` | asset://stable-id | Move preserves identity. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | Required input missing: primary action disabled with an inline reason; Cancel remains available. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | Keep entered form values local until confirmation; closing cancels the uncommitted request. |
| Error / rejected | Preserve input and previous data on failure; show the exact validation/operation error and safe retry. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | Selected asset/destination became unavailable: disable confirmation, keep its ID and entered values for correction/Cancel. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d05.svg) · [State sheet](media/editor-ui-catalog/d05-states.svg).

<a id="d06"></a>

## D06 — Delete asset / resolve usages

**Current surface; proposed layout.**

**Purpose:** Resolve live references before a protected move-to-trash transaction.

**Entry:** Assets toolbar/context/detail action for the selected stable asset ID.

**Access and scope:** Actual asset protection, owner root, busy lock and saved reference-transaction rules.

**Sources:** [crates/editor/src/assets_ui.rs::BrowserDialog](../crates/editor/src/assets_ui.rs), [crates/editor/src/assets_ui.rs::dialog_ui](../crates/editor/src/assets_ui.rs), [docs/asset-workspace-api.md](asset-workspace-api.md)

### Requirements

- Show usages. An unused asset needs no replacement; a used asset requires explicit detach or a valid same-kind alternative.

- Dependency semantics still validate: same file kind alone does not prove model/shader replacement is compatible.

- Save first, keep objects, write files/references together and retain the file in project trash.

**Actions / controls:** `Move to trash`, `Cancel`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Asset | `readonly` | crate.glb · asset://stable-id | Target and stable ID remain visible. |
| Known usages | `readonly` | 10 authored objects / templates | Use known usages, not unavailable evidence treated as empty. |
| Reference action | `enum` | Replace references | Example after explicit user choice; initial unresolved used-asset state disables confirmation. |
| Replacement | `enum` | crate-alt.glb | Same kind is necessary; dependency validation still runs before writing. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | Required input missing: primary action disabled with an inline reason; Cancel remains available. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | Keep entered form values local until confirmation; closing cancels the uncommitted request. |
| Error / rejected | Preserve input and previous data on failure; show the exact validation/operation error and safe retry. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | Selected asset/destination became unavailable: disable confirmation, keep its ID and entered values for correction/Cancel. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d06.svg) · [State sheet](media/editor-ui-catalog/d06-states.svg).

<a id="d07"></a>

## D07 — Asset usages

**Current surface; proposed layout.**

**Purpose:** Inspect the actual usage list without changing a resource or project.

**Entry:** Assets toolbar/context/detail action for the selected stable asset ID.

**Access and scope:** Actual asset protection, owner root, busy lock and saved reference-transaction rules.

**Sources:** [crates/editor/src/assets_ui.rs::BrowserDialog](../crates/editor/src/assets_ui.rs), [crates/editor/src/assets_ui.rs::dialog_ui](../crates/editor/src/assets_ui.rs), [docs/asset-workspace-api.md](asset-workspace-api.md)

### Requirements

- Scrollable usage paths retain full selectable identity and actual count.

- No usages and unavailable usage evidence are distinct; opening does not trigger Delete.

**Actions / controls:** `Close`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | Zero known usages is explicitly shown; it does not silently remove the asset. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | Not applicable: usage inspection has no form draft; Close leaves all data untouched. |
| Error / rejected | Preserve input and previous data on failure; show the exact validation/operation error and safe retry. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | A stale/deleted target cannot be confirmed; retain its identity and provide Close/refresh. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d07.svg) · [State sheet](media/editor-ui-catalog/d07-states.svg).

<a id="d08"></a>

## D08 — Create workspace

**Proposed surface; not implemented.**

**Purpose:** Create a named personal layout without duplicating the game or granting Engine rights.

**Entry:** Proposed top workspace +.

**Access and scope:** Presets and editor types obey existing source/Team availability.

**Sources:** [docs/editor-clarity-plan.md](editor-clarity-plan.md), [crates/editor/src/panes.rs::PersonalLayout](../crates/editor/src/panes.rs)

### Requirements

- Default to Duplicate current; alternatively choose an available Project/Engine layout preset.

- Trim display name, require 1–64 Unicode scalar values and uniqueness under Unicode lowercase comparison; never treat the name as a path.

- Allocate new stable workspace/area identities; copy view arrangement, not Game/Custom runtime state or native windows.

**Actions / controls:** `Create workspace`, `Cancel`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Workspace name | `text` | Lighting study | Trimmed unique 1–64 Unicode scalar display name, never a path. |
| Start from | `enum` | Duplicate current | Available preset alternatives obey rights. |
| Storage | `readonly` | Personal editor settings | No authored project clone or dirty flag. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | Required input missing: primary action disabled with an inline reason; Cancel remains available. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | Keep entered form values local until confirmation; closing cancels the uncommitted request. |
| Error / rejected | Name collision or personal-settings write failed: retain the form and old active layout; publish only after success. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | A stale/deleted target cannot be confirmed; retain its identity and provide Close/refresh. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d08.svg) · [State sheet](media/editor-ui-catalog/d08-states.svg).

<a id="d09"></a>

## D09 — Rename workspace

**Proposed surface; not implemented.**

**Purpose:** Rename a custom layout label while retaining stable IDs and all view state.

**Entry:** Proposed custom workspace RMB → Rename.

**Access and scope:** Custom layouts only in this baseline; Engine/Project preset identity is preserved.

**Sources:** [docs/editor-clarity-plan.md](editor-clarity-plan.md), [crates/editor/src/panes.rs::PersonalLayout](../crates/editor/src/panes.rs)

### Requirements

- Use the same display-name validation as D08.

- No authored dirty flag, project rename or Engine rights change.

- Cancel/write failure leaves the original label and active arrangement.

**Actions / controls:** `Rename workspace`, `Cancel`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Workspace identity | `readonly` | Stable workspace ID | Renaming preserves the ID. |
| Current name | `readonly` | My layout | Existing label stays on Cancel/error. |
| New name | `text` | Rendering study | Same name rules as D08; custom layouts only. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | Required input missing: primary action disabled with an inline reason; Cancel remains available. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | Keep entered form values local until confirmation; closing cancels the uncommitted request. |
| Error / rejected | Preserve input and previous data on failure; show the exact validation/operation error and safe retry. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | A stale/deleted target cannot be confirmed; retain its identity and provide Close/refresh. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d09.svg) · [State sheet](media/editor-ui-catalog/d09-states.svg).

<a id="d10"></a>

## D10 — Delete custom workspace

**Proposed surface; not implemented.**

**Purpose:** Remove only a custom layout after showing the personal-state consequence.

**Entry:** Proposed custom workspace RMB → Delete; preset reset uses the same guard family with its own scope label.

**Access and scope:** Never remove the last usable workspace or delete authored project data; rights still apply to fallback panes.

**Sources:** [docs/editor-clarity-plan.md](editor-clarity-plan.md), [crates/editor/src/panes.rs::PersonalLayout](../crates/editor/src/panes.rs)

### Requirements

- Name the layout and personal view state removed; document/resources remain unchanged.

- Activate a safe remaining layout before removal; preset Reset requires an explicit target and preserves other layouts.

- Persist atomically; Cancel/error leaves the old layouts intact.

**Actions / controls:** `Delete custom workspace`, `Cancel`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | Required input missing: primary action disabled with an inline reason; Cancel remains available. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | Not applicable: confirmation owns no authored draft; existing project drafts are retained and not discarded. |
| Error / rejected | Preserve input and previous data on failure; show the exact validation/operation error and safe retry. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | A stale/deleted target cannot be confirmed; retain its identity and provide Close/refresh. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d10.svg) · [State sheet](media/editor-ui-catalog/d10-states.svg).

<a id="d11"></a>

## D11 — Review render-preset replacement

**Proposed surface; not implemented.**

**Purpose:** Make replacement of the renderer’s internal graph an explicit authored operation.

**Entry:** Proposed Rendering/graph → Review preset; legacy preview navigates here instead of view-like architecture toggles.

**Access and scope:** Full/source and actual candidate validation; this does not widen Project-exposed choices.

**Sources:** [crates/editor/src/app.rs::switch_gpu_path](../crates/editor/src/app.rs), [docs/editor-ui-design.md](editor-ui-design.md)

### Requirements

- Show selected renderer and old/new preset, replaced passes/custom wiring and reconciled exposure/settings.

- Explicit Apply replacement is one Undo; Cancel changes no graph, source, setting, world or game state.

- Prepare rejected candidate separately; preserve last-good renderer and distinguish saved/authored from running state.

**Actions / controls:** `Apply replacement`, `Cancel`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | No selected compatible renderer: no preset Apply; offer navigation to the actual renderer. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | The replacement preview is tentative until Apply; unresolved conflicting drafts are named and must be resolved. |
| Error / rejected | Preserve input and previous data on failure; show the exact validation/operation error and safe retry. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | A stale/deleted target cannot be confirmed; retain its identity and provide Close/refresh. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d11.svg) · [State sheet](media/editor-ui-catalog/d11-states.svg).

<a id="d12"></a>

## D12 — Reset selected layout

**Proposed surface; not implemented.**

**Purpose:** Review replacing the selected personal arrangement with its known default.

**Entry:** Windows / workspace menu → Reset Layout; confirmation is proposed, the reset command already exists.

**Access and scope:** Reset only the selected layout to available editor types; no source/Team rights change.

**Sources:** [crates/editor/src/editor_workflow.rs::Command](../crates/editor/src/editor_workflow.rs), [docs/editor-clarity-plan.md](editor-clarity-plan.md)

### Requirements

- Name the exact workspace and preview the replacement arrangement; other layouts stay intact.

- Preserve document, sources, assets and unresolved tool drafts; reset is not project reset.

- Cancel or personal-layout write error preserves the original arrangement and active area.

**Actions / controls:** `Reset this layout`, `Cancel`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | Required input missing: primary action disabled with an inline reason; Cancel remains available. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | Not applicable: no authored draft is edited; hidden module drafts remain reachable/resolvable after a confirmed reset. |
| Error / rejected | Preserve input and previous data on failure; show the exact validation/operation error and safe retry. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | A stale/deleted target cannot be confirmed; retain its identity and provide Close/refresh. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Keyboard focus stays inside the active context; Escape cancels only its uncommitted gesture or popup.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d12.svg) · [State sheet](media/editor-ui-catalog/d12-states.svg).

<a id="d13"></a>

## D13 — Scene source operation

**Current surface; proposed layout.**

**Purpose:** Create, duplicate, rename, move, trash or explicitly externalize an authored scene.

**Entry:** File → Scene, Hierarchy scene overflow or registered scene actions in Assets.

**Access and scope:** Stop Play; validated authored document and unresolved-draft guards. Workspace names grant no rights.

**Sources:** [crates/editor/src/scene_sources.rs::draw_scene_source_dialog](../crates/editor/src/scene_sources.rs), [docs/scene-files.md](scene-files.md)

### Requirements

- Bind to stable scene/source IDs and initial scene. Paths stay inside scenes/. Entry deletion, collisions and broken references reject.

- Use the save-first/background/file-history route. Cancel before submission writes nothing; publication failure preserves the last good document/files. No fake cancellation during publication.

**Actions / controls:** `Move`, `Cancel`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Scene ID | `readonly` | arena | Retain exact identity; validate the candidate before applying. |
| Source | `readonly` | scenes/example/arena.scene.json | Retain exact identity; validate the candidate before applying. |
| Destination folder | `text` | scenes/rooms | Retain exact identity; validate the candidate before applying. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | Missing required name yields an inline error. Move uses a normalized scenes/ folder; Convert has explanatory text instead of fields. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | Keep entered form values local until confirmation; closing cancels the uncommitted request. |
| Error / rejected | Pre-submit validation keeps input; background errors appear in the operation report and retain previous disk/document state. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | Missing source, external byte conflict or changed base rejects; reload explicitly or preserve work with Save As. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Name fields take initial focus for New/Duplicate/Rename; Enter submits after the opening frame and Escape cancels. Move/Convert/Delete keep the same root-modal ownership.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d13.svg) · [State sheet](media/editor-ui-catalog/d13-states.svg).

<a id="d14"></a>

## D14 — Rename object

**Current surface; proposed layout.**

**Purpose:** Edit an authored display name without changing object identity or gameplay.

**Entry:** Hierarchy F2 or object context Rename.

**Access and scope:** Stop Play; validated authored document and unresolved-draft guards. Workspace names grant no rights.

**Sources:** [crates/editor/src/scene_sources.rs::draw_scene_source_dialog](../crates/editor/src/scene_sources.rs), [docs/scene-files.md](scene-files.md)

### Requirements

- Bind to the exact scene/object and initial scene. Stale Apply rejects without changing data; never infer game-specific names.

- Apply is one in-memory Undo; Save persists it. Cancel/no-op preserves exact null/absence and all gameplay/prefab data.

**Actions / controls:** `Rename`, `Cancel`

### Fields and bindings

Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.

| UI control | Kind | Example | Contract / binding |
| --- | --- | --- | --- |
| Object ID | `readonly` | 1 | Retain exact identity; validate the candidate before applying. |
| Name | `text` | Hero | Retain exact identity; validate the candidate before applying. |

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Show the exact target, consequence and one clear primary action plus Cancel/Close. |
| Empty / no selection | An empty name explicitly clears a previously set name. Unchanged empty input preserves null/absence. |
| Busy / preparing | Prevent duplicate submission; offer Cancel only while it can really cancel, otherwise show the noncancellable phase. |
| Draft / modified | Keep entered form values local until confirmation; closing cancels the uncommitted request. |
| Error / rejected | Keep local input and the previous authored object on validation failure. |
| Restricted / read-only | Explain the protected target or operation/rights lock; do not bypass it with another entry point. |
| Missing / stale dependency | Missing scene/object or changed base rejects Apply; Cancel and reopen against current data. |
| Narrow / short area | Keep header and action footer reachable; scroll only the body and clamp to available work area. |

**Keyboard / focus:** Focus the name field on opening; Enter applies after the first frame, Escape cancels. Child OS areas defer editing while the shared modal is active.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/d14.svg) · [State sheet](media/editor-ui-catalog/d14-states.svg).
