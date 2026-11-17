# Complete editor UI catalogue

[English](editor-ui-catalog.md) | [Русский](ru/editor-ui-catalog.md) | [Documentation](README.md)

**Design proposal, 14 November 2026; editor implementation unchanged.** This source-bounded catalogue contains 74 surfaces: 19 builtin area types, 2 real module panels and a future-extension contract, 13 nested/planned views, 2 shell surfaces, 7 existing app-modal purposes and 5 proposed confirmations/forms, 7 native picker purposes and a separate OS overwrite confirmation, 11 menus and 4 shared-control families.

Complete means coverage of the listed current production registrations/purposes and already requested UI directions. It is not every possible data combination or a bespoke design for an unknown future plugin. New registrations require a new entry/mockup. Root, GPU, Gameplay Systems and Rust graphs keep different data, execution and access contracts.

[Mockup atlas](media/editor-ui-catalog/index.html) lets you select every surface and switch layout/state sheets. It is a documentation viewer; its controls do not operate the engine.

[Visual hierarchy refinement](editor-ui-hierarchy.md) revises P01 Engine Overview, P08 Inspector and P09 Assets: grouping, selection, primary actions and three additional compact-area SVGs. Choose Compact area in the atlas. Other layouts are not redesigned by this refinement.

## Rules and boundaries

[Design](editor-ui-design.md) and [metrics/tokens](editor-ui-spec.md) remain the shared baseline. Game HUD/menu screens, future G2 animation/AI graphs and independent-engine management are outside the current editor UI. Game may display project-owned content; P22 does not impose combat, inventory or puzzles on a game.

States: ready, empty/no selection, busy, draft, error/rejection, restricted, missing/stale dependency and compact area. Every entry specifies all eight with contextual rules or an explained N/A. Focus/Cancel and persistence are separately defined. The primary image shows the proposed arrangement; state sheets show local variants/copy, not eight working builds.

Exact JSON, null, unknown payloads, stable IDs, authored/Play separation, Full/Team, pins and absent-module restrictions survive. Publication errors may leave contract-permitted recovery files: identify their paths instead of promising no filesystem traces. Saved is not applied. Read-only/Unknown is not replaced with a fabricated default.

## Consolidated proposed choices

These are concrete choices for design review, not a claim that the user accepted a new implementation:

- D08: + clones the current layout after naming; available presets are alternatives. D09/D10 affect custom layouts; D12 explicitly resets the selected arrangement.
- S02: move an area, do not duplicate it; Close returns it to the main window, not closing the project.
- P09: Show address exposes selectable read-only project-relative text; Copy absolute path is separate, with no manual path navigation.
- P17–P19 remain legacy routes with stable IDs; preset replacement goes through D11.
- Icons use shared project-owned vector geometry; overlays start with fixed sensible anchors/overflow.

## Mockup geometry

Primary SVGs: 1280×900 sheet logical units; state sheets 1280×1024. The outer caption is a documentation annotation, not product UI. Topbar 32 + projectbar 36 + status 24; area header 32. UI/menu rows 28, toolbar glyph 20 in target 32, Scene glyph 20 in target 36. Standard app form width 560, wide form 720; fields wrap/grow, body scrolls and footer remains reachable. Menu container up to 336; rows never clip wrapped labels. Native picker schematic dimensions do not prescribe actual OS geometry. Narrow-screen behavior is specified per entry and in the shared spec.

## Inventory and traceability

| ID | Surface | Status | Requirements | Layout / states |
| --- | --- | --- | --- | --- |
| P01 | Engine Overview | Current surface; proposed layout | [→](editor-ui-windows.md#p01) | [SVG](media/editor-ui-catalog/p01.svg) / [SVG](media/editor-ui-catalog/p01-states.svg) / [Compact](media/editor-ui-catalog/p01-compact.svg) |
| P02 | Module Blueprint / root graph | Current surface; proposed layout | [→](editor-ui-windows.md#p02) | [SVG](media/editor-ui-catalog/p02.svg) / [SVG](media/editor-ui-catalog/p02-states.svg) |
| P03 | Module Library | Current surface; proposed layout | [→](editor-ui-windows.md#p03) | [SVG](media/editor-ui-catalog/p03.svg) / [SVG](media/editor-ui-catalog/p03-states.svg) |
| P04 | Engine Exposure | Current surface; proposed layout | [→](editor-ui-windows.md#p04) | [SVG](media/editor-ui-catalog/p04.svg) / [SVG](media/editor-ui-catalog/p04-states.svg) |
| P05 | Project Settings | Current surface; proposed layout | [→](editor-ui-windows.md#p05) | [SVG](media/editor-ui-catalog/p05.svg) / [SVG](media/editor-ui-catalog/p05-states.svg) |
| P06 | Scene | Current surface; proposed layout | [→](editor-ui-windows.md#p06) | [SVG](media/editor-ui-catalog/p06.svg) / [SVG](media/editor-ui-catalog/p06-states.svg) |
| P07 | Hierarchy | Current surface; proposed layout | [→](editor-ui-windows.md#p07) | [SVG](media/editor-ui-catalog/p07.svg) / [SVG](media/editor-ui-catalog/p07-states.svg) |
| P08 | Inspector | Current surface; proposed layout | [→](editor-ui-windows.md#p08) | [SVG](media/editor-ui-catalog/p08.svg) / [SVG](media/editor-ui-catalog/p08-states.svg) / [Compact](media/editor-ui-catalog/p08-compact.svg) |
| P09 | Assets | Current surface; proposed layout | [→](editor-ui-windows.md#p09) | [SVG](media/editor-ui-catalog/p09.svg) / [SVG](media/editor-ui-catalog/p09-states.svg) / [Compact](media/editor-ui-catalog/p09-compact.svg) |
| P10 | Game | Current surface; proposed layout | [→](editor-ui-windows.md#p10) | [SVG](media/editor-ui-catalog/p10.svg) / [SVG](media/editor-ui-catalog/p10-states.svg) |
| P11 | Gameplay Systems | Current surface; proposed layout | [→](editor-ui-windows.md#p11) | [SVG](media/editor-ui-catalog/p11.svg) / [SVG](media/editor-ui-catalog/p11-states.svg) |
| P12 | Custom / Rust — side by side | Current surface; proposed layout | [→](editor-ui-windows.md#p12) | [SVG](media/editor-ui-catalog/p12.svg) / [SVG](media/editor-ui-catalog/p12-states.svg) |
| P13 | Custom / HLSL | Current surface; proposed layout | [→](editor-ui-windows.md#p13) | [SVG](media/editor-ui-catalog/p13.svg) / [SVG](media/editor-ui-catalog/p13-states.svg) |
| P14 | Project File | Current surface; proposed layout | [→](editor-ui-windows.md#p14) | [SVG](media/editor-ui-catalog/p14.svg) / [SVG](media/editor-ui-catalog/p14-states.svg) |
| P15 | History / Merge | Current surface; proposed layout | [→](editor-ui-windows.md#p15) | [SVG](media/editor-ui-catalog/p15.svg) / [SVG](media/editor-ui-catalog/p15-states.svg) |
| P16 | Diagnostics | Current surface; proposed layout | [→](editor-ui-windows.md#p16) | [SVG](media/editor-ui-catalog/p16.svg) / [SVG](media/editor-ui-catalog/p16-states.svg) |
| P17 | 3D Viewport — legacy preview | Current surface; proposed layout | [→](editor-ui-windows.md#p17) | [SVG](media/editor-ui-catalog/p17.svg) / [SVG](media/editor-ui-catalog/p17-states.svg) |
| P18 | Object Tree — legacy runtime | Current surface; proposed layout | [→](editor-ui-windows.md#p18) | [SVG](media/editor-ui-catalog/p18.svg) / [SVG](media/editor-ui-catalog/p18-states.svg) |
| P19 | Object / Components — legacy properties | Current surface; proposed layout | [→](editor-ui-windows.md#p19) | [SVG](media/editor-ui-catalog/p19.svg) / [SVG](media/editor-ui-catalog/p19-states.svg) |
| P20 | Participating Modules — panel 100 | Current surface; proposed layout | [→](editor-ui-windows.md#p20) | [SVG](media/editor-ui-catalog/p20.svg) / [SVG](media/editor-ui-catalog/p20-states.svg) |
| P21 | Crate Garden Level Palette — tool 110 | Current surface; proposed layout | [→](editor-ui-windows.md#p21) | [SVG](media/editor-ui-catalog/p21.svg) / [SVG](media/editor-ui-catalog/p21-states.svg) |
| P22 | Extension panel contract | Future extension contract, not a real registration | [→](editor-ui-windows.md#p22) | [SVG](media/editor-ui-catalog/p22.svg) / [SVG](media/editor-ui-catalog/p22-states.svg) |
| V01 | GPU rendering graph | Current surface; proposed layout | [→](editor-ui-views.md#v01) | [SVG](media/editor-ui-catalog/v01.svg) / [SVG](media/editor-ui-catalog/v01-states.svg) |
| V02 | Rendering configuration route | Proposed surface; not implemented | [→](editor-ui-views.md#v02) | [SVG](media/editor-ui-catalog/v02.svg) / [SVG](media/editor-ui-catalog/v02-states.svg) |
| V03 | Assets / Trash | Current surface; proposed layout | [→](editor-ui-views.md#v03) | [SVG](media/editor-ui-catalog/v03.svg) / [SVG](media/editor-ui-catalog/v03-states.svg) |
| V04 | Merge conflict workspace | Current surface; proposed layout | [→](editor-ui-views.md#v04) | [SVG](media/editor-ui-catalog/v04.svg) / [SVG](media/editor-ui-catalog/v04-states.svg) |
| V05 | Module node inspector | Current surface; proposed layout | [→](editor-ui-views.md#v05) | [SVG](media/editor-ui-catalog/v05.svg) / [SVG](media/editor-ui-catalog/v05-states.svg) |
| V06 | Rust visual-node inspector | Current surface; proposed layout | [→](editor-ui-views.md#v06) | [SVG](media/editor-ui-catalog/v06.svg) / [SVG](media/editor-ui-catalog/v06-states.svg) |
| V07 | Rust / Text mode | Current surface; proposed layout | [→](editor-ui-views.md#v07) | [SVG](media/editor-ui-catalog/v07.svg) / [SVG](media/editor-ui-catalog/v07-states.svg) |
| V08 | Rust / Graph mode | Current surface; proposed layout | [→](editor-ui-views.md#v08) | [SVG](media/editor-ui-catalog/v08.svg) / [SVG](media/editor-ui-catalog/v08-states.svg) |
| V09 | Custom state recovery | Current surface; proposed layout | [→](editor-ui-views.md#v09) | [SVG](media/editor-ui-catalog/v09.svg) / [SVG](media/editor-ui-catalog/v09-states.svg) |
| V10 | Architecture map | Proposed surface; not implemented | [→](editor-ui-views.md#v10) | [SVG](media/editor-ui-catalog/v10.svg) / [SVG](media/editor-ui-catalog/v10-states.svg) |
| S01 | Main editor shell and workspaces | Current surface; proposed layout | [→](editor-ui-windows.md#s01) | [SVG](media/editor-ui-catalog/s01.svg) / [SVG](media/editor-ui-catalog/s01-states.svg) |
| S02 | Secondary native editor window | Proposed surface; not implemented | [→](editor-ui-windows.md#s02) | [SVG](media/editor-ui-catalog/s02.svg) / [SVG](media/editor-ui-catalog/s02-states.svg) |
| N01 | Open project file | Current surface; proposed layout | [→](editor-ui-dialogs.md#n01) | [SVG](media/editor-ui-catalog/n01.svg) / [SVG](media/editor-ui-catalog/n01-states.svg) |
| N02 | Save Project As | Current surface; proposed layout | [→](editor-ui-dialogs.md#n02) | [SVG](media/editor-ui-catalog/n02.svg) / [SVG](media/editor-ui-catalog/n02-states.svg) |
| N03 | New project destination | Current surface; proposed layout | [→](editor-ui-dialogs.md#n03) | [SVG](media/editor-ui-catalog/n03.svg) / [SVG](media/editor-ui-catalog/n03-states.svg) |
| N04 | Windows Player destination | Current surface; proposed layout | [→](editor-ui-dialogs.md#n04) | [SVG](media/editor-ui-catalog/n04.svg) / [SVG](media/editor-ui-catalog/n04-states.svg) |
| N05 | Team editor destination | Current surface; proposed layout | [→](editor-ui-dialogs.md#n05) | [SVG](media/editor-ui-catalog/n05.svg) / [SVG](media/editor-ui-catalog/n05-states.svg) |
| N06 | Full source destination | Current surface; proposed layout | [→](editor-ui-dialogs.md#n06) | [SVG](media/editor-ui-catalog/n06.svg) / [SVG](media/editor-ui-catalog/n06-states.svg) |
| N07 | Import asset file | Current surface; proposed layout | [→](editor-ui-dialogs.md#n07) | [SVG](media/editor-ui-catalog/n07.svg) / [SVG](media/editor-ui-catalog/n07-states.svg) |
| N08 | OS overwrite confirmation | Current surface; proposed layout | [→](editor-ui-dialogs.md#n08) | [SVG](media/editor-ui-catalog/n08.svg) / [SVG](media/editor-ui-catalog/n08-states.svg) |
| D01 | Unsaved changes / draft resolution | Current surface; proposed layout | [→](editor-ui-dialogs.md#d01) | [SVG](media/editor-ui-catalog/d01.svg) / [SVG](media/editor-ui-catalog/d01-states.svg) |
| D02 | Operation result / report | Current surface; proposed layout | [→](editor-ui-dialogs.md#d02) | [SVG](media/editor-ui-catalog/d02.svg) / [SVG](media/editor-ui-catalog/d02-states.svg) |
| D03 | New folder | Current surface; proposed layout | [→](editor-ui-dialogs.md#d03) | [SVG](media/editor-ui-catalog/d03.svg) / [SVG](media/editor-ui-catalog/d03-states.svg) |
| D04 | Rename asset | Current surface; proposed layout | [→](editor-ui-dialogs.md#d04) | [SVG](media/editor-ui-catalog/d04.svg) / [SVG](media/editor-ui-catalog/d04-states.svg) |
| D05 | Move asset | Current surface; proposed layout | [→](editor-ui-dialogs.md#d05) | [SVG](media/editor-ui-catalog/d05.svg) / [SVG](media/editor-ui-catalog/d05-states.svg) |
| D06 | Delete asset / resolve usages | Current surface; proposed layout | [→](editor-ui-dialogs.md#d06) | [SVG](media/editor-ui-catalog/d06.svg) / [SVG](media/editor-ui-catalog/d06-states.svg) |
| D07 | Asset usages | Current surface; proposed layout | [→](editor-ui-dialogs.md#d07) | [SVG](media/editor-ui-catalog/d07.svg) / [SVG](media/editor-ui-catalog/d07-states.svg) |
| D08 | Create workspace | Proposed surface; not implemented | [→](editor-ui-dialogs.md#d08) | [SVG](media/editor-ui-catalog/d08.svg) / [SVG](media/editor-ui-catalog/d08-states.svg) |
| D09 | Rename workspace | Proposed surface; not implemented | [→](editor-ui-dialogs.md#d09) | [SVG](media/editor-ui-catalog/d09.svg) / [SVG](media/editor-ui-catalog/d09-states.svg) |
| D10 | Delete custom workspace | Proposed surface; not implemented | [→](editor-ui-dialogs.md#d10) | [SVG](media/editor-ui-catalog/d10.svg) / [SVG](media/editor-ui-catalog/d10-states.svg) |
| D11 | Review render-preset replacement | Proposed surface; not implemented | [→](editor-ui-dialogs.md#d11) | [SVG](media/editor-ui-catalog/d11.svg) / [SVG](media/editor-ui-catalog/d11-states.svg) |
| D12 | Reset selected layout | Proposed surface; not implemented | [→](editor-ui-dialogs.md#d12) | [SVG](media/editor-ui-catalog/d12.svg) / [SVG](media/editor-ui-catalog/d12-states.svg) |
| M01 | File menu and submenus | Current surface; proposed layout | [→](editor-ui-menus.md#m01) | [SVG](media/editor-ui-catalog/m01.svg) / [SVG](media/editor-ui-catalog/m01-states.svg) |
| M02 | Edit menu | Current surface; proposed layout | [→](editor-ui-menus.md#m02) | [SVG](media/editor-ui-catalog/m02.svg) / [SVG](media/editor-ui-catalog/m02-states.svg) |
| M03 | Tools menu | Current surface; proposed layout | [→](editor-ui-menus.md#m03) | [SVG](media/editor-ui-catalog/m03.svg) / [SVG](media/editor-ui-catalog/m03-states.svg) |
| M04 | Windows menu | Current surface; proposed layout | [→](editor-ui-menus.md#m04) | [SVG](media/editor-ui-catalog/m04.svg) / [SVG](media/editor-ui-catalog/m04-states.svg) |
| M05 | Workspace context menu | Proposed surface; not implemented | [→](editor-ui-menus.md#m05) | [SVG](media/editor-ui-catalog/m05.svg) / [SVG](media/editor-ui-catalog/m05-states.svg) |
| M06 | Area-border menu | Proposed surface; not implemented | [→](editor-ui-menus.md#m06) | [SVG](media/editor-ui-catalog/m06.svg) / [SVG](media/editor-ui-catalog/m06-states.svg) |
| M07 | Graph Add Node search | Current surface; proposed layout | [→](editor-ui-menus.md#m07) | [SVG](media/editor-ui-catalog/m07.svg) / [SVG](media/editor-ui-catalog/m07-states.svg) |
| M08 | Node actions menu | Proposed surface; not implemented | [→](editor-ui-menus.md#m08) | [SVG](media/editor-ui-catalog/m08.svg) / [SVG](media/editor-ui-catalog/m08-states.svg) |
| M09 | Asset context menu | Current surface; proposed layout | [→](editor-ui-menus.md#m09) | [SVG](media/editor-ui-catalog/m09.svg) / [SVG](media/editor-ui-catalog/m09-states.svg) |
| M10 | Area editor-type selector | Current surface; proposed layout | [→](editor-ui-menus.md#m10) | [SVG](media/editor-ui-catalog/m10.svg) / [SVG](media/editor-ui-catalog/m10-states.svg) |
| M11 | Scene tool options | Current surface; proposed layout | [→](editor-ui-menus.md#m11) | [SVG](media/editor-ui-catalog/m11.svg) / [SVG](media/editor-ui-catalog/m11-states.svg) |
| C01 | Schema-driven property fields | Shared control family; proposed standard | [→](editor-ui-views.md#c01) | [SVG](media/editor-ui-catalog/c01.svg) / [SVG](media/editor-ui-catalog/c01-states.svg) |
| C02 | Object/asset reference pickers | Shared control family; proposed standard | [→](editor-ui-views.md#c02) | [SVG](media/editor-ui-catalog/c02.svg) / [SVG](media/editor-ui-catalog/c02-states.svg) |
| C03 | Icon / pin help and feedback | Shared control family; proposed standard | [→](editor-ui-views.md#c03) | [SVG](media/editor-ui-catalog/c03.svg) / [SVG](media/editor-ui-catalog/c03-states.svg) |
| C04 | Transforms, colour and standard controls | Shared control family; proposed standard | [→](editor-ui-views.md#c04) | [SVG](media/editor-ui-catalog/c04.svg) / [SVG](media/editor-ui-catalog/c04-states.svg) |
| V11 | GPU graph / Deferred example | Current surface; proposed layout | [→](editor-ui-views.md#v11) | [SVG](media/editor-ui-catalog/v11.svg) / [SVG](media/editor-ui-catalog/v11-states.svg) |
| V12 | Rust / scalar graph example | Current surface; proposed layout | [→](editor-ui-views.md#v12) | [SVG](media/editor-ui-catalog/v12.svg) / [SVG](media/editor-ui-catalog/v12-states.svg) |
| V13 | Gameplay execution JSON | Current surface; proposed layout | [→](editor-ui-views.md#v13) | [SVG](media/editor-ui-catalog/v13.svg) / [SVG](media/editor-ui-catalog/v13-states.svg) |
| D13 | Scene source operation | Current surface; proposed layout | [→](editor-ui-dialogs.md#d13) | [SVG](media/editor-ui-catalog/d13.svg) / [SVG](media/editor-ui-catalog/d13-states.svg) |
| D14 | Rename object | Current surface; proposed layout | [→](editor-ui-dialogs.md#d14) | [SVG](media/editor-ui-catalog/d14.svg) / [SVG](media/editor-ui-catalog/d14-states.svg) |

## Verification and reproduction

Canonical bilingual data: [catalog.json](ui-catalog/catalog.json); generator: [generate.py](ui-catalog/generate.py). Generation does not modify the editor, projects or personal settings. Only Python 3.9+ standard library is required.

```text
python docs/ui-catalog/generate.py
python docs/ui-catalog/generate.py --check
```

Verification covers source registration/dialog mapping, EN/RU/state coverage, presence/consistency of every mockup, links, geometry and contrast. QA requires rasterizing all SVGs and inspecting every sheet; atlas checks test selection/search/state navigation, not engine commands. Final independent evidence/source binding lives in `outputs/reviews/editor-ui-catalog-20261114/`. Pictures do not prove native input, DPI, OS accessibility, persistence, FPS or implementation of these requirements.
