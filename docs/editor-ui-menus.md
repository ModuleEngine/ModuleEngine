# Menus and contextual commands

[English](editor-ui-menus.md) | [Русский](ru/editor-ui-menus.md) | [Catalogue](editor-ui-catalog.md)

Complete EN/RU meaning comes from one bilingual catalogue. All pictures are design proposals, not running-editor captures. Mockup UI is English under the language contract. Eight state categories are specified per surface, including explained N/A. An existing surface does not mean its proposed layout is implemented.

<a id="m01"></a>

## M01 — File menu and submenus

**Current surface; proposed layout.**

**Purpose:** Expose context-appropriate commands without changing the document merely by opening the menu.

**Entry:** File

**Access and scope:** Actual source/Team, selected scope and command guards remain authoritative.

**Sources:** [crates/editor/src/editor_workflow.rs::draw_workflow_toolbar](../crates/editor/src/editor_workflow.rs), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- Inventory actual New/Recent/Export submenus and conditional composition choices; one configurable engine does not erase current menu entries.

- Call existing guarded commands; opening a menu does not create files or select another Engine/project.

- Recent labels identify the file, expose full path and preserve the current project on failed open.

**Actions / controls:** `New Project >`, `Templates >`, `Open Project…`, `Recent Projects >`, `Save Project`, `Save As…`, `Import Asset…`, `Export >`, `Exit`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Readable command names, checked state, actual shortcut and availability are visible. |
| Empty / no selection | Show a no-match message only for a dynamic/search list; a static menu keeps its disabled entries where useful. |
| Busy / preparing | Disable conflicting commands with the operation reason; opening/closing a menu never starts that operation. |
| Draft / modified | Not applicable: the menu owns no data draft; the destination command resolves applicable drafts. |
| Error / rejected | A failed command reports through its owning tool or operation result; dismissing the menu never loses the draft. |
| Restricted / read-only | Availability follows the actual command guard; disabled help explains rights or unavailable context. |
| Missing / stale dependency | Remove stale dynamic targets or retain a disabled identity with a reason; never invoke a recycled ID. |
| Narrow / short area | Clamp to the active OS work area, scroll long lists, preserve keyboard focus and submenu access. |

**Keyboard / focus:** Arrow keys move focus, Enter invokes only an enabled command, Escape closes the innermost popup. Text fields keep their own context menu.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/m01.svg) · [State sheet](media/editor-ui-catalog/m01-states.svg).

<a id="m02"></a>

## M02 — Edit menu

**Current surface; proposed layout.**

**Purpose:** Expose context-appropriate commands without changing the document merely by opening the menu.

**Entry:** Edit

**Access and scope:** Actual source/Team, selected scope and command guards remain authoritative.

**Sources:** [crates/editor/src/editor_workflow.rs::draw_workflow_toolbar](../crates/editor/src/editor_workflow.rs), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- Make the document-history target explicit; text-field Ctrl+Z/Redo stays in its buffer.

- Show actual Undo/Redo availability and preserve existing one-gesture/one-operation boundaries.

- Menu invocation must not claim text Undo while dispatching a document restore.

**Actions / controls:** `Undo project`, `Redo project`, `History / Merge`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Readable command names, checked state, actual shortcut and availability are visible. |
| Empty / no selection | Show a no-match message only for a dynamic/search list; a static menu keeps its disabled entries where useful. |
| Busy / preparing | Disable conflicting commands with the operation reason; opening/closing a menu never starts that operation. |
| Draft / modified | Not applicable: the menu owns no data draft; the destination command resolves applicable drafts. |
| Error / rejected | A failed command reports through its owning tool or operation result; dismissing the menu never loses the draft. |
| Restricted / read-only | Availability follows the actual command guard; disabled help explains rights or unavailable context. |
| Missing / stale dependency | Remove stale dynamic targets or retain a disabled identity with a reason; never invoke a recycled ID. |
| Narrow / short area | Clamp to the active OS work area, scroll long lists, preserve keyboard focus and submenu access. |

**Keyboard / focus:** Arrow keys move focus, Enter invokes only an enabled command, Escape closes the innermost popup. Text fields keep their own context menu.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/m02.svg) · [State sheet](media/editor-ui-catalog/m02-states.svg).

<a id="m03"></a>

## M03 — Tools menu

**Current surface; proposed layout.**

**Purpose:** Expose context-appropriate commands without changing the document merely by opening the menu.

**Entry:** Tools

**Access and scope:** Actual source/Team, selected scope and command guards remain authoritative.

**Sources:** [crates/editor/src/editor_workflow.rs::draw_workflow_toolbar](../crates/editor/src/editor_workflow.rs), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- Keep current publish/update-contracts discoverable as an advanced source operation, not multi-engine management.

- Publishing validates source/profile pins and reports affected result; it cannot relabel an invalid source package as Full.

- Build and File export share the same operation and progress/report path; no duplicate build worker.

**Actions / controls:** `Check Project`, `Publish Engine profile / update contracts`, `Build Windows Player…`, `Diagnostics`, `Project Settings`, `Last operation report`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Readable command names, checked state, actual shortcut and availability are visible. |
| Empty / no selection | Show a no-match message only for a dynamic/search list; a static menu keeps its disabled entries where useful. |
| Busy / preparing | Disable conflicting commands with the operation reason; opening/closing a menu never starts that operation. |
| Draft / modified | Not applicable: the menu owns no data draft; the destination command resolves applicable drafts. |
| Error / rejected | A failed command reports through its owning tool or operation result; dismissing the menu never loses the draft. |
| Restricted / read-only | Availability follows the actual command guard; disabled help explains rights or unavailable context. |
| Missing / stale dependency | Remove stale dynamic targets or retain a disabled identity with a reason; never invoke a recycled ID. |
| Narrow / short area | Clamp to the active OS work area, scroll long lists, preserve keyboard focus and submenu access. |

**Keyboard / focus:** Arrow keys move focus, Enter invokes only an enabled command, Escape closes the innermost popup. Text fields keep their own context menu.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/m03.svg) · [State sheet](media/editor-ui-catalog/m03-states.svg).

<a id="m04"></a>

## M04 — Windows menu

**Current surface; proposed layout.**

**Purpose:** Expose context-appropriate commands without changing the document merely by opening the menu.

**Entry:** Windows

**Access and scope:** Actual source/Team, selected scope and command guards remain authoritative.

**Sources:** [crates/editor/src/editor_workflow.rs::draw_workflow_toolbar](../crates/editor/src/editor_workflow.rs), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- Group proposed navigation while preserving existing labels/serialized identities and rights.

- Activate an existing matching area in this workspace, otherwise create one beside the active area.

- Saved-layout actions target personal data; Reset uses D12, and loading preserves authored project/drafts.

**Actions / controls:** `Scene and objects >`, `Resources >`, `Gameplay >`, `Engine >`, `Code and files >`, `Diagnostics / legacy >`, `Module tools >`, `Layout >`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Readable command names, checked state, actual shortcut and availability are visible. |
| Empty / no selection | Show a no-match message only for a dynamic/search list; a static menu keeps its disabled entries where useful. |
| Busy / preparing | Disable conflicting commands with the operation reason; opening/closing a menu never starts that operation. |
| Draft / modified | Not applicable: the menu owns no data draft; the destination command resolves applicable drafts. |
| Error / rejected | A failed command reports through its owning tool or operation result; dismissing the menu never loses the draft. |
| Restricted / read-only | Availability follows the actual command guard; disabled help explains rights or unavailable context. |
| Missing / stale dependency | Remove stale dynamic targets or retain a disabled identity with a reason; never invoke a recycled ID. |
| Narrow / short area | Clamp to the active OS work area, scroll long lists, preserve keyboard focus and submenu access. |

**Keyboard / focus:** Arrow keys move focus, Enter invokes only an enabled command, Escape closes the innermost popup. Text fields keep their own context menu.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/m04.svg) · [State sheet](media/editor-ui-catalog/m04-states.svg).

<a id="m05"></a>

## M05 — Workspace context menu

**Proposed surface; not implemented.**

**Purpose:** Expose context-appropriate commands without changing the document merely by opening the menu.

**Entry:** Workspace tab RMB

**Access and scope:** Actual source/Team, selected scope and command guards remain authoritative.

**Sources:** [docs/editor-clarity-plan.md](editor-clarity-plan.md), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- Workspace actions never open another game; clone layout/view state with new stable area IDs.

- Engine/Project preset identities are retained; unavailable Engine tools cannot be smuggled through a custom layout.

- Top +, this menu and the matching dialog share validation and persistence.

**Actions / controls:** `Duplicate current…`, `Rename…`, `Delete…`, `Reset Layout…`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Readable command names, checked state, actual shortcut and availability are visible. |
| Empty / no selection | Show a no-match message only for a dynamic/search list; a static menu keeps its disabled entries where useful. |
| Busy / preparing | Disable conflicting commands with the operation reason; opening/closing a menu never starts that operation. |
| Draft / modified | Not applicable: the menu owns no data draft; the destination command resolves applicable drafts. |
| Error / rejected | A failed command reports through its owning tool or operation result; dismissing the menu never loses the draft. |
| Restricted / read-only | Availability follows the actual command guard; disabled help explains rights or unavailable context. |
| Missing / stale dependency | Remove stale dynamic targets or retain a disabled identity with a reason; never invoke a recycled ID. |
| Narrow / short area | Clamp to the active OS work area, scroll long lists, preserve keyboard focus and submenu access. |

**Keyboard / focus:** Arrow keys move focus, Enter invokes only an enabled command, Escape closes the innermost popup. Text fields keep their own context menu.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/m05.svg) · [State sheet](media/editor-ui-catalog/m05-states.svg).

<a id="m06"></a>

## M06 — Area-border menu

**Proposed surface; not implemented.**

**Purpose:** Expose context-appropriate commands without changing the document merely by opening the menu.

**Entry:** Area border RMB / header menu

**Access and scope:** Actual source/Team, selected scope and command guards remain authoritative.

**Sources:** [docs/editor-clarity-plan.md](editor-clarity-plan.md), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- Define split by visible divider and resulting placement, then map it to the current Axis implementation; no ambiguous H/V letters.

- Join previews surviving content and discards no document/tool draft; last main area leaves a safe fallback.

- Divider hit region wins over graph RMB; Escape cancels preview/capture, menus also have a keyboard/header route.

**Actions / controls:** `Split Vertically — Left/Right`, `Split Horizontally — Top/Bottom`, `Join — keep selected neighbor`, `Move to New Window`, `Return to Main Window`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Readable command names, checked state, actual shortcut and availability are visible. |
| Empty / no selection | Show a no-match message only for a dynamic/search list; a static menu keeps its disabled entries where useful. |
| Busy / preparing | Disable conflicting commands with the operation reason; opening/closing a menu never starts that operation. |
| Draft / modified | Not applicable: the menu owns no data draft; the destination command resolves applicable drafts. |
| Error / rejected | A failed command reports through its owning tool or operation result; dismissing the menu never loses the draft. |
| Restricted / read-only | Availability follows the actual command guard; disabled help explains rights or unavailable context. |
| Missing / stale dependency | Remove stale dynamic targets or retain a disabled identity with a reason; never invoke a recycled ID. |
| Narrow / short area | Clamp to the active OS work area, scroll long lists, preserve keyboard focus and submenu access. |

**Keyboard / focus:** Arrow keys move focus, Enter invokes only an enabled command, Escape closes the innermost popup. Text fields keep their own context menu.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/m06.svg) · [State sheet](media/editor-ui-catalog/m06-states.svg).

<a id="m07"></a>

## M07 — Graph Add Node search

**Current surface; proposed layout.**

**Purpose:** Expose context-appropriate commands without changing the document merely by opening the menu.

**Entry:** Graph background RMB click, or Menu key/Shift+F10 with canvas focus and no node target (proposed keyboard route). Root Blueprint has no persistent + Node button.

**Access and scope:** Actual source/Team, selected scope and command guards remain authoritative.

**Sources:** [crates/editor/src/canvas.rs::draw](../crates/editor/src/canvas.rs), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- RMB release after a pan never opens the search; insertion uses the pointer’s original scope/position.

- Root/GPU descriptors differ; Gameplay Systems uses its system palette and Rust only supported visual operations.

- Enter confirms an enabled result, Escape closes; no decorative unavailable nodes and no arbitrary Rust AST creation.

**Actions / controls:** `Search nodes`, `Scope`, `Compatible categories`, `Create selected`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Readable command names, checked state, actual shortcut and availability are visible. |
| Empty / no selection | Show a no-match message only for a dynamic/search list; a static menu keeps its disabled entries where useful. |
| Busy / preparing | Disable conflicting commands with the operation reason; opening/closing a menu never starts that operation. |
| Draft / modified | Not applicable: the menu owns no data draft; the destination command resolves applicable drafts. |
| Error / rejected | A failed command reports through its owning tool or operation result; dismissing the menu never loses the draft. |
| Restricted / read-only | Availability follows the actual command guard; disabled help explains rights or unavailable context. |
| Missing / stale dependency | Remove stale dynamic targets or retain a disabled identity with a reason; never invoke a recycled ID. |
| Narrow / short area | Clamp to the active OS work area, scroll long lists, preserve keyboard focus and submenu access. |

**Keyboard / focus:** Arrow keys move focus, Enter invokes only an enabled command, Escape closes the innermost popup. Text fields keep their own context menu.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/m07.svg) · [State sheet](media/editor-ui-catalog/m07-states.svg).

<a id="m08"></a>

## M08 — Node actions menu

**Proposed surface; not implemented.**

**Purpose:** Expose context-appropriate commands without changing the document merely by opening the menu.

**Entry:** Node RMB click without drag, or Menu key/Shift+F10 on a focused node target (proposed keyboard route).

**Access and scope:** Actual source/Team, selected scope and command guards remain authoritative.

**Sources:** [docs/editor-clarity-plan.md](editor-clarity-plan.md), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- Actions depend on node kind, supported handler, scope and rights; do not offer unimplemented copy/duplicate actions.

- An RMB click on a selected node retains the mixed selection; a click on an unselected node selects that node. Defer selection until click/drag classification, so RMB pan never changes selection.

- Keep selection/target clear; Delete cleans incident wires and associated settings and restores them on Undo.

- Graph background opens Add; node click opens actions; RMB dragging pans in either target context.

- Open subgraph is the discoverable equivalent of eligible-node double-click. Show Frame graph with F and Delete selected with Delete; each menu entry uses the same current-scope command and enabled/disabled reason.

**Actions / controls:** `Open subgraph`, `Edit parameters`, `Edit HLSL`, `Delete selected`, `Frame graph`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Readable command names, checked state, actual shortcut and availability are visible. |
| Empty / no selection | Show a no-match message only for a dynamic/search list; a static menu keeps its disabled entries where useful. |
| Busy / preparing | Disable conflicting commands with the operation reason; opening/closing a menu never starts that operation. |
| Draft / modified | Not applicable: the menu owns no data draft; the destination command resolves applicable drafts. |
| Error / rejected | A failed command reports through its owning tool or operation result; dismissing the menu never loses the draft. |
| Restricted / read-only | Availability follows the actual command guard; disabled help explains rights or unavailable context. |
| Missing / stale dependency | Remove stale dynamic targets or retain a disabled identity with a reason; never invoke a recycled ID. |
| Narrow / short area | Clamp to the active OS work area, scroll long lists, preserve keyboard focus and submenu access. |

**Keyboard / focus:** Arrow keys move focus, Enter invokes only an enabled command, Escape closes the innermost popup. Text fields keep their own context menu.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/m08.svg) · [State sheet](media/editor-ui-catalog/m08-states.svg).

<a id="m09"></a>

## M09 — Asset context menu

**Current surface; proposed layout.**

**Purpose:** Expose context-appropriate commands without changing the document merely by opening the menu.

**Entry:** Asset tile/list RMB

**Access and scope:** Actual source/Team, selected scope and command guards remain authoritative.

**Sources:** [crates/editor/src/assets_ui.rs::file_commands](../crates/editor/src/assets_ui.rs), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- Target one stable asset ID; command availability reflects protected/busy/missing state.

- Folder context contains only supported navigation/creation, not unsupported directory rename/move/delete.

- Audio/shader entries cannot claim a working thumbnail/audio-preview service that does not exist.

**Actions / controls:** `Preview`, `Rename…`, `Move…`, `Move to trash…`, `Usages…`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Readable command names, checked state, actual shortcut and availability are visible. |
| Empty / no selection | Show a no-match message only for a dynamic/search list; a static menu keeps its disabled entries where useful. |
| Busy / preparing | Disable conflicting commands with the operation reason; opening/closing a menu never starts that operation. |
| Draft / modified | Not applicable: the menu owns no data draft; the destination command resolves applicable drafts. |
| Error / rejected | A failed command reports through its owning tool or operation result; dismissing the menu never loses the draft. |
| Restricted / read-only | Availability follows the actual command guard; disabled help explains rights or unavailable context. |
| Missing / stale dependency | Remove stale dynamic targets or retain a disabled identity with a reason; never invoke a recycled ID. |
| Narrow / short area | Clamp to the active OS work area, scroll long lists, preserve keyboard focus and submenu access. |

**Keyboard / focus:** Arrow keys move focus, Enter invokes only an enabled command, Escape closes the innermost popup. Text fields keep their own context menu.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/m09.svg) · [State sheet](media/editor-ui-catalog/m09-states.svg).

<a id="m10"></a>

## M10 — Area editor-type selector

**Current surface; proposed layout.**

**Purpose:** Expose context-appropriate commands without changing the document merely by opening the menu.

**Entry:** Pane header type control

**Access and scope:** Actual source/Team, selected scope and command guards remain authoritative.

**Sources:** [crates/editor/src/dock.rs::EditorType](../crates/editor/src/dock.rs), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- Choose an editor for this area, not another project or engine instance.

- Listing reads metadata only; selection retains permitted per-area navigation and gives preparation feedback if the actual tool needs it.

- Unavailable persisted types show a safe explanatory shell, never silently become an unrelated working-looking tool.

**Actions / controls:** `Scene and resources`, `Engine tools`, `Legacy root tools`, `Module panels`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Readable command names, checked state, actual shortcut and availability are visible. |
| Empty / no selection | Show a no-match message only for a dynamic/search list; a static menu keeps its disabled entries where useful. |
| Busy / preparing | Disable conflicting commands with the operation reason; opening/closing a menu never starts that operation. |
| Draft / modified | Not applicable: the menu owns no data draft; the destination command resolves applicable drafts. |
| Error / rejected | A failed command reports through its owning tool or operation result; dismissing the menu never loses the draft. |
| Restricted / read-only | Availability follows the actual command guard; disabled help explains rights or unavailable context. |
| Missing / stale dependency | Remove stale dynamic targets or retain a disabled identity with a reason; never invoke a recycled ID. |
| Narrow / short area | Clamp to the active OS work area, scroll long lists, preserve keyboard focus and submenu access. |

**Keyboard / focus:** Arrow keys move focus, Enter invokes only an enabled command, Escape closes the innermost popup. Text fields keep their own context menu.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/m10.svg) · [State sheet](media/editor-ui-catalog/m10-states.svg).

<a id="m11"></a>

## M11 — Scene tool options

**Current surface; proposed layout.**

**Purpose:** Expose context-appropriate commands without changing the document merely by opening the menu.

**Entry:** World/Local, Snap step or overlay overflow

**Access and scope:** Actual source/Team, selected scope and command guards remain authoritative.

**Sources:** [crates/editor/src/scene_view_ui.rs::SceneViewportState](../crates/editor/src/scene_view_ui.rs), [docs/editor-ui-spec.md](editor-ui-spec.md)

### Requirements

- Keep current World/Local value visible; do not replace it with an ambiguous icon alone.

- Step values are finite/valid for their operation; view/tool settings do not mark the authored game dirty.

- Options consume input, preserve an already captured gesture and remain reachable in compact/short Scene views.

**Actions / controls:** `Coordinate space`, `Snap enabled`, `Move / rotate / scale steps`, `Frame Selection`, `Active tool`

### States

| State | Required behavior |
| --- | --- |
| Ready / normal | Readable command names, checked state, actual shortcut and availability are visible. |
| Empty / no selection | Show a no-match message only for a dynamic/search list; a static menu keeps its disabled entries where useful. |
| Busy / preparing | Disable conflicting commands with the operation reason; opening/closing a menu never starts that operation. |
| Draft / modified | Not applicable: the menu owns no data draft; the destination command resolves applicable drafts. |
| Error / rejected | A failed command reports through its owning tool or operation result; dismissing the menu never loses the draft. |
| Restricted / read-only | Availability follows the actual command guard; disabled help explains rights or unavailable context. |
| Missing / stale dependency | Remove stale dynamic targets or retain a disabled identity with a reason; never invoke a recycled ID. |
| Narrow / short area | Clamp to the active OS work area, scroll long lists, preserve keyboard focus and submenu access. |

**Keyboard / focus:** Arrow keys move focus, Enter invokes only an enabled command, Escape closes the innermost popup. Text fields keep their own context menu.

**Data / lifecycle:** Preserve exact authored data, stable IDs and personal view state; use existing validated history/persistence paths.

**Implementation acceptance:**

- Reach the surface through its stated entry; verify normal and every applicable state without changing another scope.

- Check permissions, narrow layout, keyboard focus, Cancel/error preservation and the actual command result.

**Mockups:** [Layout](media/editor-ui-catalog/m11.svg) · [State sheet](media/editor-ui-catalog/m11-states.svg).
