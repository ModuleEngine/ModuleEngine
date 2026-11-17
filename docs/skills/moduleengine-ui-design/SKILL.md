---
name: moduleengine-ui-design
description: "Design or implement explicitly requested ModuleEngine native editor UI: workspaces, composition discovery, graphs, Scene tools, Assets, icons and operation feedback. Use the repository UI baseline and real contracts. Do not use for game HUD/art, unrelated apps or rendering algorithms; observed regressions also need moduleengine-debug."
---

# Design ModuleEngine editor UI

[English](SKILL.md) | [Русский](references/SKILL.ru.md)

Use this skill for ModuleEngine editor interaction/design work, including an
explicitly authorized implementation. It is not permission to implement a
discussion, reset a layout, add a subsystem/dependency or publish anything.
The user's current request takes precedence over this baseline; record a
deliberate exception rather than rejecting the user's authorized change.

## Establish scope and authority

Find the ModuleEngine root from the current workspace and confirm its AGENTS.md,
docs/current-status.md and editor sources. Paths below are repository-relative,
not relative to this installed skill. If the repository is unavailable, work
from supplied material and state the limitation; ask for its location only when
needed. Do not search unrelated personal directories for a substitute checkout.

Read AGENTS.md, docs/editor-ui-design.md and the affected sections of
docs/editor-ui-spec.md; load docs/editor-ui-tokens.json for actual measurements.
Check docs/editor-clarity-plan.md for implemented versus deferred work. Read the
affected runtime/persistence/rights contract only where the requested slice
depends on it. Do not load the whole documentation archive.

For an implementation, establish the initial diff and preserve authored JSON,
assets, personal layouts and unrelated work. State the user's task, expected
behavior, affected view/data scope, non-goals and a concrete acceptance route.
For analysis or a design document, remain in that mode; do not patch the editor.

## Shape a useful interaction

Trace the actual action path before redesigning it. Separate project identity,
workspace layout, semantic ownership, graph scope and editing rights. Engine/
Project tabs are layouts over the same open document. Full/Team/source checks
determine access; a Gameplay owner does not unlock a frozen Team schedule.
Technical engine mechanisms, reusable gameplay and concrete game content remain
separate. Unknown activity evidence is not an inactive/active claim.

Start composition discovery at Engine Overview. Use focused graphs for actual
processes, and filter node actions by the real registry/scope/permissions.
Pin help explains real data/constraints at the connection, rather than replacing
validation with a generic texture label or a permanent jargon legend.

Apply the shared metric/colour/icon baseline. Standard Play/Pause/Stop/Save and
Scene tools use recognisable vector icons, semantic names, checked/disabled
states and hover/focus help with real shortcuts/reasons. Keep module names and
ambiguous/destructive commands labelled. Scene overlays must consume their own
input without stealing an existing gesture or picking through to the scene.
Use responsive overflow rather than tiny fonts or clipped critical commands.

In egui, popup/submenu bodies are separate Areas: explicitly carry the invoking
UI's enabled/editing state through every menu level. Numeric configuration menus
need `CloseOnClickOutside` so click-to-type stays open. Verify an already-open
menu after entering Play/disabling its caller, and test typing as well as dragging.

Do not call other Context/layer/ownership probes inside egui input/memory/data
closures: nested access can deadlock. Copy the needed scalars or small relevant
events, release the guard, then check ownership. Test batched press/wheel/release
in event order and retain the owner when crossing another Scene area.

Preserve the agreed graph bindings: MMB/RMB drag pan, RMB click background adds/
searches, RMB click node opens actions, plain wheel zooms at the pointer,
Alt+LMB wire deletion, mixed Delete as one Undo with cleanup. Text/popups/borders
own their input first. Do not import another editor's shortcuts or Alt-click-pin
semantics. These are the agreed target bindings, not a claim about current UI.
Apply them when graph-input implementation is in scope; preserve unaffected
current bindings in other slices rather than silently migrating the whole UI.

Distinguish dirty/saved/applied/ready/failed and actual long-running states.
No fake progress percentage, nonfunctional Cancel or silent replacement of the
last-good version. Layout navigation, selection and graph pan/zoom must not
compile or rebuild the project. Opening tools must not activate absent modules.

## Verify and maintain

Implement only the authorized slice using existing command/validation paths.
Keep design proposals separate from current behavior. Exact JSON, stable IDs,
unknown payloads, authored/Play separation and layout/file-operation safeguards
survive visual changes. New move/duplicate-window or address-entry semantics
need an explicit decision if not settled by the task/contracts.

Choose checks by the change: doc/skill links and consistency for design-only
work; actual reachable command, keyboard/pointer conflicts, error/Cancel,
Undo/Save/reopen/rights for behavior. Inspect narrow areas and relevant DPI;
measure matched workloads when performance is affected. Static screenshots do
not prove working commands, persistence, accessibility APIs or Player FPS.
Use brief Computer Use batches only if necessary and follow the session-hygiene
skill; do not leave it open during code or documentation work.

Update complete EN/RU documentation and tokens when the baseline changes.
Substantial work requires the independent reviewer in docs/review.md; applying
this skill does not make the author's self-check independent. Report what
changed, actual checks, proposals/open decisions and unverified conditions.
