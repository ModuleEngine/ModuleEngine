# Editor UI: technical baseline

[English](editor-ui-spec.md) | [Русский](ru/editor-ui-spec.md) | [Documentation](README.md)

<a id="status"></a>

## Status, units and source of values

**Design baseline, 14 November 2026; native adoption is in progress in stage 2A.**
See [actual adoption and evidence](editor-ui-rollout.md); this specification alone
does not establish acceptance of all controls. The [design](editor-ui-design.md)
defines why and scope. [editor-ui-tokens.json](editor-ui-tokens.json) is the
machine-readable source of the proposed numeric/color baseline; this document
assigns roles and behavior. Changes must update both language versions and the
tokens together. The JSON is documentation data, not a loader or new dependency.
These values are ModuleEngine proposals, not measurements copied from competitors.

The [surface catalogue](editor-ui-catalog.md) assigns these rules to every current
window/dialog purpose, nested view and queued interface, with dedicated layouts
and state sheets. Native OS picker drawings are schematics, not prescribed OS
geometry. The generator and offline atlas are documentation tools, not editor code.

All lengths/font sizes are **egui logical points**, not physical pixels, printer
points or CSS units. At the current native scale and user zoom,
`physical_pixels = logical_points × pixels_per_point`; do not multiply geometry
twice by DPI. Graph zoom changes graph content only, not app text, workspace tabs
or mouse thresholds. Snap thin strokes to the current physical pixel grid.
Use a coherent sans-serif UI family with verified English/Cyrillic coverage;
retain the current font until replacement coverage/licensing is checked.

<a id="tokens"></a>

## Shared metrics and colour

| Role | Proposed value | Rule |
| --- | --- | --- |
| Spacing | 4 / 8 / 12 / 16 / 24 / 32 | Base step 4; related controls 4–8, panel padding 12, groups 16–24. Exceptions need a reason. |
| Typography | body 14; secondary 12; section 16; title 20; code 13 | Body line 20. No essential 10-point microcopy. Use weight/hierarchy before more sizes. |
| Controls/list/menu rows | 28 high | Center icon/text; pad fields horizontally by 8. Expand vertically for wrapped content, never crop its last line. |
| Toolbar icon button / glyph | 32 / 20 | Glyph on a 24-unit vector grid, 2-unit stroke; hit box is larger than the glyph. |
| Topbar / project toolbar / status | 32 / 48 minimum / 24 high | Project toolbar: 32-high controls with 8 top/bottom padding; wrapped content grows the row. OS titlebar excluded. Base chrome totals 104 logical points. |
| Pane header / padding | 32 / 12 | One pane title/type selector, not another identical heading beneath it. |
| Focus / corner radius | 2 / 4 | Visible keyboard focus independent of hover and selected state. |
| Area divider | 1 visible / 8 hit band | Center the hit band on the divider; resolve intersections once, not two menus. |
| Workspace tab width | 88–180 | Long names ellipsize with full name help. Overflow, never unreadably small tabs. |
| Tooltip | delay 500 ms; padding 8; gap 8; max width 320 | Clamp/wrap inside the current OS work area; keyboard focus help is immediate. |

| Palette token | Value | Use |
| --- | --- | --- |
| canvas / panel / raised | `#111823` / `#192230` / `#202D3E` | Canvas, normal area, popup/overlay surfaces. |
| control / hover / selected | `#28394D` / `#344A63` / `#234D50` | Interactive surfaces and selected tool background. |
| text / secondary / icon | `#E6EDF5` / `#B6C4D5` / `#DCE7F4` | Normal, supporting text and standard pictograms. |
| accent / on_accent | `#66D6C4` / `#0E2224` | Focus/selection outline; dark text on a solid accent surface. |
| boundary / separator | `#91A1B8` / `#334155` | Meaningful control outline versus decorative group separator. |
| error / warning / success | `#FF9F9F` / `#F1C477` / `#8ED4A8` | Always accompany state text/icon; never colour alone. |
| disabled | `#7B8798` | Inactive controls only, with explanation; not the colour for ordinary secondary text. |

Use at least **4.5:1 for all enabled text** and **3:1 for meaningful icons,
focus outlines and necessary boundaries** against each actual adjacent surface.
These are internal benchmarks adapted from W3C references in the design, not a
native accessibility certification. Check actual compositing, not only hex pairs;
overlays use opaque surfaces for controls. Disabled/decorative exceptions must
not hide essential status. Pin colours are type hints, supplemented by names,
port direction and explicit mismatch details. Do not assign gameplay ownership
from a pin's colour.

<a id="hierarchy"></a>

## Focused hierarchy prototypes

[P01/P08/P09 refinements](editor-ui-hierarchy.md) apply role-based density to
the three selected surfaces. The scoped `hierarchy` token group defines
40-point navigation rows, 56-point two-line subsystem rows, 28-point facts,
36-point field rows, 24/32-point group/section gaps and a 112/720-point
Inspector label/content limit. Inputs remain 28 high. The selected subject
uses 20 semibold, sections 16, fields 14 and metadata 12. Meaningful selection
uses background plus a marker/check, separate from keyboard focus.

Only the contextual primary action uses a solid accent fill with on_accent
text. Related facts are grouped by spacing instead of repeated outlines;
input boundaries remain visible. A 480-point compact SVG is a dock-area
variant with unchanged text sizes, not whole-window/DPI acceptance. Inspector
Apply stays component-local; Assets operations keep existing guards. These
are proposed documentation layouts, not global restyling of the native editor
or of all other catalogue sheets.

<a id="shell"></a>

## Editor shell, workspaces and windows

Topbar: File/Edit/Tools/Windows on the left, named workspace tabs and **+** next
to them. Project toolbar: project name/path access, dirty marker, actual rights,
Play/Pause and Stop, Save and current operation. Keep play controls together in
a stable location; saving is a separate group. Statusbar: concise operation state
and an expandable diagnostics entry. Do not repeat version/demo slogans in the
primary task area. Product/version can live in About or diagnostics.

The native [compact chrome follow-up](release-042-ui-density-verification.md)
supersedes the earlier 8-point Project padding in the implemented shell. Shared
local chrome uses 1-point padding and 2-point gaps at 100% UI zoom: Project
buttons remain 32 high with equal 1-point top/bottom padding, Area headers are
30 high with 28-point selector/Area controls, and pane bodies have a 1-point
inset. The divider reserves only 2 visible points while retaining its 8-point
grab band over content; actual selector/Area buttons retain their own input.
Menu/workspace spacing and the collapsed operation/status rows are compact.
Expanded diagnostics preserve ordinary rows, progress, errors and actions.
These local overrides do not change fonts, form/menu rows, Assets or Inspector
spacing. The proposal table above still describes the broader design baseline.

<a id="command-ownership"></a>

### Command ownership and duplicate controls

For one Game Play session, the shared shell toolbar owns the single
always-visible session controller: Play changes to Pause/Resume with state,
followed by Stop and Restart where supported. Save is a separate project
command. Do not place another Play/Stop/Restart row inside Game. Its local
toolbar owns viewport-specific actions such as Focus game and visible input
focus/session status. This is a target mockup layout, not a native refactor.

Project File keeps Apply file and Discard draft locally; Save Project stays in
the shared shell/File menu. Apply Rust/HLSL, Apply schedule, Apply level,
component Apply, checkpoints and merge resolution have distinct targets or
transactions and must not be removed by matching labels to global Save.

The catalogue-wide control audit distinguishes these cases:

| Surface / route | Decision |
| --- | --- |
| P10 Game | Remove the duplicate local lifecycle row; illustrate Playing with Pause/Stop/Restart once in the shell. |
| P14 Project File | Remove the duplicate local Save; keep both draft commands. |
| P17 legacy 3D preview | Remove Start/Pause: native viewport_panel and the shell PlayPause share Editor.playing. Keep labelled Reset preview (legacy runtime/time) and Open configuration. |
| C01–C04 component specimens | Suppress the shell lifecycle/Save toolbar. C03 labels icon specimens; C04 labels tool/state specimens and omits its redundant Save glyph. Samples are not executable editor controls. |
| File/Tools menus and shortcuts | Preserve discoverable alternate entry points for the same command; they are not a second always-visible controller. |
| D01 save/transition guard | Preserve Save and continue: it resolves a pending transition, not an ordinary second Save toolbar. The modal owns input; its background shell is not a second active controller. |
| Other catalogue routes | No additional same-target persistent controller found in actions and renderer-specific command rows. Distinct scopes/transactions remain available. |

The shared native PlayPause flag also reaches the legacy preview; this design
does not invent a separate preview clock. Its Reset clears that legacy runtime
and time, whereas Game Restart targets the Game host. The audit includes all
72 entries and the three compact variants; it checks
command purpose/target as well as repeated icons. These are source-bounded
documentation findings, not new native UI acceptance. Future controls must
identify their owner/target before adding another visible command row.

The shell must retain current project identity when space is tight. Overflow
workspace tabs first, abbreviate the displayed path but retain copy/full-path
access, and move secondary commands into a labelled menu. Do not shrink text.
At an available client width below 480 logical points or height below 240,
use one primary area and an area switcher in both main and floating hosts;
keep other areas accessible. Ordinary 760×510 floating hosts show their actual
split tree. This is a responsive route, not a mandatory minimum window size. Minimum
window dimensions must be clamped to the actual monitor work area, including
OS chrome. A 1080p display at 200% scale cannot fit a 1100-logical-point window.

Preserve existing layout IDs and personal storage; no silent reset on adoption.
Workspace persistence is separate from authored Undo/history and dirty state.
Border menu labels include **Split Horizontally**, **Split Vertically**, and a
join/close command naming or previewing the area retained. Expose the same menu
through the pane header for discoverability and keyboard access. Closing the
last main area must leave a usable fallback, not an empty unopenable workspace.

Separate OS windows need stable viewport and area IDs, one shared document and
command history, and per-view navigation state. The implemented secondary header shares the type selector, Area menu, free drag
space with docked areas. The Area menu is a vertical three-dot vector button at
the right edge, with Area as its hover/accessibility name. The OS title/name, system × and native resize frame are
retained, including during client-header moves. The client header has no second
× or File/Edit row. Each floating host has its own split tree and shares the
main host's Area menu and operations. Area Close removes one leaf; OS × closes
all current leaves in that host. The host ID survives removal of its original
leaf. Splitting and choosing an editor preserve host membership; drag docking
moves a leaf explicitly. There is no Return menu command. The final main leaf
is protected, with the reason available on disabled Close hover. Restore
offscreen windows onto an available monitor without resetting their contents.
Moving between mixed-DPI monitors recomputes physical sizes once. Never call an
embedded egui floating panel a separate native OS window.

Editor root and floating hosts use the original ModuleEngine ICO, retained unchanged
with all seven frames (16/24/32/48/64/128/256). Native creation
receives the same icon data; Windows additionally assigns small and large icons
to the exact owned HWND when it is created or rebound. Never use the foreground
window as the identity of a floating host. This does not embed EXE resources or
change Player branding. [Icon evidence and visual limits](release-042-window-icon-verification.md).

<a id="icons"></a>

## Icons, tooltips and states

Use a shared vector catalogue and one command source for toolbar/menu/shortcut
invocation. This is an implementation recommendation; the current application
has no new command abstraction from this document. Avoid emoji, font-dependent
Unicode substitutes and a different icon family in each panel. Keep small icons
simple and distinct at 100/150/200% scale. Third-party icon assets require a
licence compatible with source and binary distribution. The catalogue proposes
shared project-owned vector geometry; no third-party icon family is installed.

| Control | Pictogram | Tooltip/accessibility content |
| --- | --- | --- |
| Play / Pause / Resume | triangle / two bars / triangle | Actual action and current state. Play runs a copy; Pause preserves it. Use only supported states. |
| Stop | square | Stop Play and return to authored Scene. Disable when there is no active session. |
| Save | recognisable disk/save symbol | `Save Project · Ctrl+S`; distinguish no changes, dirty data/drafts and save errors. |
| Move / Rotate / Scale | four-way arrows / circular arrow / scale handles | `Move · G`, `Rotate · R`, `Scale · S`; selected tool stays visibly checked. |
| Frame selection | frame corners | `Frame Selection · F`; no selection gives an explicit reason. |
| Snap | magnet | Checked toggle, current step and a labelled options menu. Do not hide numeric step configuration behind an unexplained icon. |
| World/Local | coordinate axes with state | Keep the current value `World` or `Local` visible beside/inside the compact selector; icon alone is ambiguous. |

No new Play/Stop shortcut is assigned by this proposal. Show only a real current
binding. Shortcut labels reflect the active keymap; do not import W/E/R from
another editor. Text-bearing menus remain the discoverable equivalent for icons.

States are normal, hover, pressed, keyboard focus, selected/checked, disabled and
busy/error where relevant. Selected uses background plus an accent marker/check;
focus uses its own 2-point outline. An unsaved indicator is distinct from failed
Apply. A busy icon must not hide Stop when Stop is actually supported. Every
interactive icon provides a semantic action name and applicable checked/disabled
state to the accessibility layer. Static SVG mockups do not demonstrate that API.

On a solid accent-filled control, use an inset `on_accent` focus outline or
separate the outer accent ring with a canvas-colour gap. An accent ring directly
against the same accent fill does not meet the meaningful-state contrast rule.

Tooltip structure: **action + assigned shortcut**, one short effect sentence,
and current value or disabled reason when relevant. Example: `Play` / `Run a
copy of the authored scene.` / `Unavailable: preparing game logic.` Hover and
keyboard focus expose equivalent help, including disabled controls via a
focusable/help-bearing wrapper where needed. Keep help visible while hovered or
focused, permit entering the tooltip, and dismiss with Escape. Never require
hover as the only way to discover a critical error or destructive consequence.

<a id="scene"></a>

## Scene overlays

Coordinates below are relative to the **image content rectangle**, after the
32-point pane header. They do not reserve another full-width Scene tool row.

| Group | Geometry and placement |
| --- | --- |
| Transform rail | Left/top inset 8; width 44 = padding 4 + hit box 36 + padding 4. Three buttons, gap 4: total height 124. Glyphs 20, vertically centred. |
| Transform orientation | One cycling button: globe = World axes, cube = Local axes. Its tooltip names the current mode and next action; Local is highlighted. Group width 44 = padding 4 + hit 36 + padding 4. No snap fields in this group. |
| Camera speed | Beside orientation, a 120-wide group with one 112×36 Speed numeric field. Default 8 world units/s; range 0.1–1000. Editable during Play, saved per personal area, not in the game project. |
| Snapping | Separate 84-wide group: magnet toggles snapping; sliders opens only Move/Rotate/Scale increments. Group gap 8, controls 36 with gap 4. Editing a step does not switch axes. |
| View actions | Separate 44-wide Frame selected group. The four top/right groups occupy 316 points, inset 8 from top/right, with gaps 8. All group boxes are 44 high, controls 36 high with identical top/centre alignment. No shared dropdown combines axes and snapping. |
| Compact behavior | Below image width 400 or height 160, use a compact menu with separate Transform axes, Snapping and Camera sections, one orientation toggle, Speed field and a named Snap steps submenu. At width 400 the 316-point cluster does not overlap the transform rail. |

Group surfaces are opaque `raised`; use small gaps, subtle separators and no
large blank header. Clamp menus/tooltips inside the current viewport/work area.
If an area is too short for the rail, collapse it to a tool selector with named
choices. Keep the active tool discoverable. Fixed anchors are the first baseline;
free dragging and saved overlay presets remain proposed future options.

Overlays are drawn after the scene image and before transient menus/help. Their
hit regions consume input before scene picking/gizmos; transparent space outside
their rectangles does not block navigation. A pointer gesture keeps its owner
until release/Cancel, including crossing an overlay. One completed transform
creates one Undo; Escape restores the pre-gesture transform. Hidden/collapsed
tools cannot retain invisible pointer capture. The Game viewport retains its
game controls; these are Scene authoring overlays, not a game HUD.

The Scene follow-up supports held RMB + WASD flight, Q down/E up on world Y,
and mouse look around the
eye, independently of the game camera and authored/Undo state. The RMB press
must originate on that canvas; overlays/text/popups/active transform drags own
their input. Wheel ownership follows event order even when RMB press/release
and scrolling arrive in one frame. S is backward during flight, not the Scale shortcut. RMB + wheel
adjusts flight speed (×1.25 per line/40 points) without zoom or a delayed zoom
tail; a plain wheel still changes distance. Combined axes are normalized at the
selected speed. Release,
Escape, focus loss, a popup, a hidden area or a document transition ends flight.
MMB orbit, Shift+MMB pan, wheel distance and focused F remain. See the
[Scene controls](scene-assets.md) for speed, time clamping and pointer limits.

<a id="graph"></a>

## Graph and module catalogue

Show a human scope breadcrumb, for example `Engine / Rendering / Main`, and the
current validation/apply state. It names the actual open graph; the example is
not an additional serialized hierarchy. Preserve current root/GPU/gameplay
execution boundaries. Adding a node uses that scope's real registry and rights,
not a global catalogue of incompatible technical and gameplay items.

Root Module Blueprint has no permanent Add/Open/Frame/Delete row; the canvas
uses the reclaimed height. Background RMB opens Add Node; double LMB click on
an eligible expandable node opens its existing subgraph; F frames the current
graph; Delete removes the selection. Node menus expose the same actions and
shortcut labels for discovery/keyboard access. An empty graph may show a
contextual Right-click to add hint. These are target mockup bindings; this
documentation change does not implement them in the native editor.

Node baseline: width 220, min 180/max 360; header 28; body padding 12; pin rows
24; pin hit region 16; wire stroke 2 with 6-point distance hit tolerance. Node
labels wrap or ellipsize with full text help; pin meaning remains inspectable.
Port direction and labels supplement colour. On a low graph zoom, simplify node
detail rather than creating illegible essential labels; selection/fit/details
remain available. Zoom proposal is 0.25–2.5 around the pointer. Preserve loaded
view state; do not rewrite authored files merely to adopt these limits.

Catalogue/popup: width 280–360, clamped to available area; search at the top,
28-high list rows with category and readable names. One scroll region takes
remaining height; details/help do not depend on a fixed 360/420-point list cap.
Use an explicit readable destination when several graph areas exist. Node
context actions reflect actual support and permissions. Export workspace/source
actions belong in File/Tools, not the primary add-node list.

Pin help shows friendly meaning and technical type, direction, constraints and
relevant values. Example: `Depth texture · input` plus its actual format/size
requirements, not fabricated example requirements. A failed wire explains the
specific mismatch and leaves prior connections intact. The optional “Connection
types” help section replaces a permanent unexplained `PIN CONTRACTS` footer.
Do not relax the validator to make simplified labels appear compatible.

### Input ownership and priority

| Context/gesture | Behavior |
| --- | --- |
| Modal, popup, text field or overlay | Its own events first. Text Delete/Undo and native text context menus remain text operations. |
| Area divider RMB | Area management; no graph node menu underneath. Divider is outside graph content. |
| MMB drag in graph content | Pan that graph, no document edit. |
| RMB in graph content | Delay the menu decision. After 6 logical points of movement, pan; on release without a drag, open background/node actions at the original target. Drag release never opens a menu. |
| Plain wheel over graph canvas | Zoom under pointer; scroll lists/popup controls when they own the pointer. Graph zoom never rescales the shell. |
| Alt+LMB on a wire | Remove that wire once; consume the click before node/background actions. |
| LMB on node / node drag | Select / move nodes. Pin and embedded control hit tests take precedence over node body dragging. |
| Double LMB click on an expandable node | Open its existing permitted subgraph; a drag, pin hit or embedded text/value control does not navigate. |
| F in the focused graph canvas | Frame the current graph without editing it; text fields, popups and modals retain their keys. |
| Menu key / Shift+F10 in the focused graph | Proposed keyboard equivalent: node actions for the focused node target, otherwise background Add. Text fields retain their native context menu. |
| LMB background drag | Selection marquee, not graph panning. Proposed modifiers: Shift adds, Ctrl toggles; check against the current keymap before implementation. |
| Wire LMB / mixed selection | Select a wire or add it to a node/wire selection; show a distinct selection outline. |
| Delete in the focused graph | Remove selection plus incident wires, clean associated settings, validate, and record one atomic Undo transaction. No action in another graph scope. |
| Escape / lost focus | Cancel the active uncommitted gesture, dismiss its transient UI and release capture; never silently commit a partial deletion. |

Thresholds/hit tolerances are in screen logical points and do not shrink with
graph zoom. Resolve overlapping candidates deterministically to the nearest
visible wire/port; no hidden offscreen connection may steal the click. Semantic
graph changes follow existing validation/last-good rules; pan/zoom/selection do
not trigger compilation or rebuild. Large lists need clipping/virtualization
and bounded string/layout work; measure before adding caches or abstractions.

<a id="assets"></a>

## Assets and detail panels

Folder tree starts at width 200, usable minimum 160; indentation 16, row height
28, separate disclosure and folder icons. Selected item identity persists in
Tiles/List. Tile 96×96 with 48-point icon/preview and gap 8; names wrap to two
lines and expose full text. List rows 28 with type icons and readable columns.
Address/breadcrumb row height 32. Details start at 240 and can collapse.

Below Assets width 640, collapse the details region first and offer its explicit
Details command. If tree and content still cannot fit, use one column with a
Folders/back route. Search/filter, current folder and selection stay visible.
Breadcrumb overflow preserves root/current folder; hidden middle segments go
into a menu. Address display and copy use the same canonical current location.
Relative/absolute format and switching gesture remain the open product choices
in the design; no manual navigation is implied by a selectable text field.

Folder pictograms must differ from file icons at all scales and in monochrome.
No rename/move/delete merely from selecting, copying a path or changing view
mode. File operations keep the [Assets transaction contract](asset-workspace-api.md),
references and project trash behavior. When a resource is unavailable, keep its
identity/reference visible with a useful diagnostic instead of an empty tile.

<a id="integration"></a>

## Implementation hooks and preservation

| Current source | Bounded future integration |
| --- | --- |
| `crates/editor/src/app.rs::configure_style` | Shared styles/metrics instead of more isolated hard-coded values. |
| `crates/editor/src/app.rs::library`, `graph_panel` | Scoped catalogue, connection help and graph inputs; inspect the actual graph widget path before editing. |
| `crates/editor/src/app.rs::viewport_panel`, `switch_world`, `switch_gpu_path` | Separate legacy preview navigation from explicit document-mutating presets. |
| `crates/editor/src/dock.rs`, `panes.rs` | Area header/border commands, stable area IDs and existing layout deserialization. |
| `crates/editor/src/editor_workflow.rs` | Existing File/Edit/Tools/Windows, guards and shortcuts; one action path for icons and menus. |
| `crates/editor/src/scene_view_ui.rs`, `scene_tools.rs` | Overlay placement/input before picking; retain transform math and transaction semantics. |
| `crates/editor/src/assets_ui.rs`, `assets_workspace_ui.rs` | Browse presentation and address view over existing asset identity/operations. |
| `crates/editor/src/engine_overview.rs` | Preserve evidence/ownership/navigation instead of introducing another composition database. |

The pinned egui 0.36.2 source defines `Context::pixels_per_point` and
`show_viewport_deferred`/`show_viewport_immediate`. Their existence is not proof
that this editor already supports independent native panels. Embedded fallback,
shared state, resource registration, focus and close persistence need explicit
design and native checks. Prefer event-driven repaint and independent windows
where supported; inspect actual integration before choosing an API. Do not
double-render all views per input event or wait for GPU/Custom on the UI thread.

Keep authored/Play data separation, exact JSON and unknown payloads, stable
asset/module/area IDs, Full/Team restrictions, optional-feature absence and
last-good Apply/error behavior. This visual design is not permission to migrate
serialized data, reset layouts or add an always-on optional service.

<a id="verification"></a>

## Verification checklist

Each implemented slice gets before/after evidence bound to HEAD, source digest,
artifact, target/features and the actual scenario. Independent review follows
[the project contract](review.md). The current documentation/skills delivery
checks links, language parity, token consistency/contrast, mockup rendering and
skill behavior; it claims no new native or performance acceptance.

| Check | Required observation for a future implementation |
| --- | --- |
| Geometry/readability | 1280×720, 1600×900 and 1920×1080 physical work areas at 100/150/200%, plus a mixed-DPI monitor move; clamp to real OS work area. Inspect long labels and narrow/short individual areas. |
| Icons/help | Recognize Play/Pause/Stop/Save and transforms; keyboard focus, checked/disabled/busy states, actual shortcuts, readable hover help and Escape. Semantic accessibility names/states checked in the actual backend. |
| Layout/window safety | Existing personal layout loads unchanged; named tabs/+ and border-menu Cancel; secondary OS-window close/return/restart, shared Undo and no duplicate project state. |
| Graph input | Every input-table conflict, RMB drag-release, pointer-centred zoom, mixed Delete/Undo and unavailable node action; text editing and other graph scopes remain intact. |
| Data/rights | Save/reopen, Cancel, failed Apply/export/file transaction, unknown payloads and Team/missing-source/omitted-module fixtures; no implicit activation. |
| Assets/Scene | Folder recognition/path copy, stable references after permitted operations, tool selection and gestures; no picking through overlays, one Undo and Escape rollback. |
| Operation feedback | Dirty/saved/applied/failed remain distinct; last-good frame labelled; no fake percentage or nonfunctional Cancel. |
| Performance | Compare identical graph drag/zoom, idle, Scene and Playing workloads before/after on the same machine/artifact settings. Record callback/frame distributions and workload; no compilation during pure view movement. |

Use existing render/game budgets unchanged when those paths are affected; a
static mockup or fast callback does not certify Player FPS. Document untested
native OS/accessibility/scanout conditions instead of inferring them from a
headless or small fixture check.


Native window resizing records the actual size for the next session. Reading a
size or position from the OS does not issue a new size/position command. Explicit
layout moves, host creation/recreation and monitor/scale changes retain their
placement route. [Resize feedback evidence and limits](release-042-resize-jitter-verification.md).
