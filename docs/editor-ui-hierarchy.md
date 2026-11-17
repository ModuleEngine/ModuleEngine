# Editor UI: visual hierarchy refinement

[English](editor-ui-hierarchy.md) | [Русский](ru/editor-ui-hierarchy.md) | [Documentation](README.md)

**Design proposal, 14 November 2026.** This refinement changes the documentation
mockups for Engine Overview (P01), Inspector (P08) and Assets (P09), including
their state requirements and a compact-area variant each. It does not change
the native editor, commands, project formats, permissions or saved layouts.
Other catalogue layouts retain their previous design; completeness of the
catalogue is not evidence that all its compositions are equally usable.

## Problem and intended reading order

The previous layouts gave navigation, property values, diagnostic facts and
actions similar containers and visual weight. Short facts occupied 72–88-point
rows; nested outlines and repeated separators dominated grouping. Selection in
Overview could be indicated by the word `Selected` without a clear selected row.
Passing source/link/contrast checks did not establish a useful reading order.

The refined order is **current context → selected subject → task and primary
action → supporting evidence**. Object/component origin and draft state remain
visible where they affect a decision. Unknown/missing/rejected states and access
restrictions must not disappear into a collapsed technical-details section.

## Shared spacing and emphasis

Values use egui logical points, following the [technical baseline](editor-ui-spec.md).
The `hierarchy` group in [tokens](editor-ui-tokens.json) is scoped to these three
prototypes. It is not a global runtime theme or a migration of all 72 surfaces.

| Role | Geometry and treatment |
| --- | --- |
| Related items | 4–8 gap inside a group; do not separate every readonly fact with a line. |
| Distinct groups | 24 gap; 32 for major sections. Keep the section title with its contents. |
| Navigation / subsystem rows | 40 / 56 high. A two-line subsystem row has a name and short purpose. |
| Readonly fact / field rows | 28 / 36 high; editable controls remain 28 high. Multiline values grow instead of clipping. |
| Inspector form | Label width 112; content width capped at 720. Three vector fields remain one row; wide areas do not stretch values across the whole window. |
| Type hierarchy | Selected subject 20 semibold, section 16 semibold, normal 14, metadata 12. Pane and subject titles have different meanings. |
| Primary action | At most one contextual solid `accent` button with `on_accent` text. Readonly detail needs no invented primary action. Secondary navigation is text/icon; destructive file/component commands are separate. |
| Selection | Selected background plus marker/check and subject name; never colour alone. Keyboard focus remains a separate outline under the shared baseline. |
| Borders | Retain meaningful input boundaries and region dividers. Use headings/spacing for groups instead of nested outlined cards around all facts. |
| Surfaces | Quiet canvas for navigation/preview, panel for content, raised for chrome/temporary surfaces. Existing palette and contrast budgets remain unchanged. |

The extra SVGs are **480-point dock-area variants**, not scaled-down 1280-point
screens or entire OS windows. They retain 14/12-point text and real control
sizes. A future native implementation must also clamp against actual work areas,
handle long values/scrolling and verify focus/DPI/accessibility separately.

## Engine Overview — P01

[Wide mockup](media/editor-ui-catalog/p01.svg) · [Compact area](media/editor-ui-catalog/p01-compact.svg) · [States](media/editor-ui-catalog/p01-states.svg)

Three visual levels separate composition groups (184 wide), subsystem list
(280 wide), and selected detail, with 24-point group gaps. `Technical Engine`
has a group marker; `Rendering` has a persistent highlighted row/check. The
detail explains purpose and current implementation before exposing a primary
`Open render graph` route and secondary `Show renderer node` navigation.
Both map to existing supported targets; this introduces no new architecture
selector or implicit module activation.

`Installed`, `Offered` and `Participating` remain three independent labelled
facts. Their normal example values do not become a green `Ready` badge. The
runtime-readiness limitation remains visible. Only supplementary provenance
can collapse into `Source details`; errors and unavailable-action reasons stay
outside that disclosure. A 480-point area shows the selected detail with
`Back to composition`, preserving list/search/selection state on return.

## Inspector — P08

[Wide mockup](media/editor-ui-catalog/p08.svg) · [Compact area](media/editor-ui-catalog/p08-compact.svg) · [States](media/editor-ui-catalog/p08-states.svg)

Object identity and prefab context lead into Transform, then separate
source-owned Actor and Health sections. Numeric controls are aligned; labels
are close to their values. Component menus carry existing component actions,
including `Inherit defaults` or `Remove component` where actually supported,
while `Add component` stays a secondary document command. Auxiliary wide-view
context does not compete with the field column and is omitted in a narrow area.

The example Actor speed has a **local draft 6.5**, distinct from its **authored
override 6.0**. `Apply Actor fields` means the existing component-local Apply;
it does not batch Transform, Health or other drafts into a new transaction.
Transform gestures keep their current Undo behavior. Prefab/Health origin text
distinguishes inheritance from a local override, without treating defaults as
authored values. A no-op Apply must preserve inheritance. Stable IDs stay
readonly; changing appearance does not change `ComponentSchema`/`PropertySchema`
ownership, exact keys, unknown payloads or authored/Play separation.

**Pre-2A Inspector draft limitation:** [inspector_ui](../crates/editor/src/game_ui.rs)
previously set `field_key = None` after component Apply; the next pass reconstructed the
whole `component_drafts` map from resolved component values. A selection mismatch
also took that reconstruction path. Other components' authored values were
preserved, but their unsaved field drafts were not protected by those guards.
The mockup shows only one pending Actor draft. [Stage 2A's implementation](editor-ui-rollout.md)
now introduces object-scoped draft retention, explicit conflict/discard handling
and guarded transitions; pointer-driven regressions cover component Apply and
selection. The full stage's native/error/DPI acceptance remains separate.

## Assets — P09

[Wide mockup](media/editor-ui-catalog/p09.svg) · [Compact area](media/editor-ui-catalog/p09-compact.svg) · [States](media/editor-ui-catalog/p09-states.svg)

The reading order is folder/location, scoped results, selected asset. A quiet
folder rail uses recognizable folder glyphs and indentation; file-type glyphs
distinguish models, textures, audio and other files. Folder selection has a
marker and asset selection has an outline/check. The illustrated eight items
are example data, not a scan or alteration of the user's project.
Search keeps its existing recursive scope; `Search folder and subfolders`
states that scope. A URI such as `asset://a-hero` is labelled `Asset reference`,
distinct from the bare manifest ID `a-hero`.

Breadcrumbs stay close to the search/results context. `Show address` exposes
selectable readonly text for the same project-relative folder, rather than a
second editable navigation source. The compact detail demonstrates this
alternate address mode and `Back to models`; returning preserves location,
search and stable asset selection. The earlier address proposal keeps
`Copy absolute path` in location options; it is not a second address value or
a claim that this proposed control is implemented in the current editor.

`Import…` is the primary folder command. Selected-asset details use a bounded
preview, name, path and stable reference; `Usages…` and `Asset actions…` remain
secondary. Rename/Move/Trash are explicit commands of the selected stable ID,
not tile-click side effects. Their existing save-first guards, reference
preflight, protected paths, transactions, recovery and Undo stay unchanged.
The vector model preview is schematic; it does not claim a decoded live model.

## Maintenance and verification

Canonical requirements: [catalogue data](ui-catalog/catalog.json). Rendering:
[generator](ui-catalog/generate.py) and [focused layouts](ui-catalog/hierarchy.py).
Regenerate or compare without writing:

```text
python -B docs/ui-catalog/generate.py
python -B docs/ui-catalog/generate.py --check
```

The [atlas](media/editor-ui-catalog/index.html) offers `Compact area` only for
P01/P08/P09 and falls back to Layout when switching to another surface. The
catalogue keeps 72 entries and 144 primary/state sheets, plus these three
compact variants. Full English/Russian requirements remain aligned.

Acceptance requires visual inspection of all six revised layout/compact images,
the three changed state sheets, selected/primary/control contrast and actual
text/control geometry. Check generation, source references, links, atlas
selection/keyboard/compact fallback, narrow viewer widths and exact preservation
of all unrelated source/user data. Independent review must inspect reading
order and task clarity as well as the mechanical checks. Evidence/source
binding belongs to `outputs/reviews/editor-ui-hierarchy-20261114/`.
Static/browser evidence does not establish native commands, usability testing
with users, DPI/accessibility, persistence or runtime performance acceptance.
