<a id="настройки-engine-и-project"></a>

# Engine and Project settings

[English](settings.md) | [Русский](ru/settings.md) | [Documentation](README.md)

EngineConfiguration enables feature modules and defines the allowed Project controls. ProjectConfiguration stores values, ready variants and conditions. PropertyStore stores module-contributed data separately from the architecture graph.

The image rectangle edges in Scene/Game are aligned to the physical pixel grid; image placement and pointer coordinates use that same rectangle. This also avoids linear-filter blur from fractional dock boundaries.

In 0.4.2, ordinary Player and editor Scene request 100% of their image area's
physical pixels, including resize and DPI changes. A single Game pane also
requests its full physical size; multiple Game panes still share one Play frame
and are not certified as simultaneous per-pane 1:1 outputs. The former 1280×720
editor preview cap is removed. The Vulkan safety bound is 8192 per axis and
16,777,216 pixels; unsupported allocations fail explicitly rather than silently
lowering resolution. A control to reduce render scale is deferred. During a
Player resize, the retained last-good frame keeps its proportions until the
new size completes. This does not change FXAA or establish the cause of a reported
motion trail; see the [0.4.2 plan](release-042-plan.md).

Where the Engine author exposes them, Project Settings changes exposure and selects ACES/Reinhard or FXAA/off. These are real GPU variants. Time/parameter conditions select a ready variant; the first matching rule in a group takes priority, otherwise selection/default is used. A new request for a missing variant is rejected; loading an obsolete local player preference has the separate fallback described below.

<a id="rendering-controls-041"></a>

## Rendering controls in 0.4.1

From an ordinary launch, Full and Team use **Edit → Project Settings** in 0.4.2; 0.4.1 used **Tools**. **Windows → Authoring → Project Settings** remains an alternate route. Full with checked matching sources can open **Windows → Composition → Engine Overview**, select the connected renderer and use **Open render graph** to reach its actual graph. Saved personal layouts retain their arrangement; these routes do not change the document or grant Engine rights. Engine authors define passes and exposed controls; Project authors choose the prepared variants. The selected option uses its authored friendly label, including custom labels.

Project Settings puts the section list on the left and the current heading and label/value rows on the right in a wide pane. A narrow pane uses a focusable section selector; Up/Down switches sections while it is focused. Rendering appears only for actual exposure, and module sections use available Global/Module property schemas. `?` beside a field and the general **Help** menu provide explanations on request. **Variant conditions** is collapsed initially. Only unsaved changes, pending preparation and errors occupy status space; an idle pane has no permanent save/rights/preparation paragraphs. Errors retain their reason and a correction/Undo route. Section selection is temporary per project and pane, not authored data or a new layout field.

The shared rendering summary explains **AA** as smoothing jagged edges and **AO** as darkening occluded areas where surfaces are close. It inspects the renderer connected to the root viewport, identifies its built-in FXAA/SSAO passes and distinguishes **pass absent** from **pass present, disabled**. A custom replacement is not identified as a built-in pass. **Base pass enabled** is the graph's authored setting; Project variants and conditions can override it. The summary follows actual command targets, including custom control IDs, and shows the resolved authored enable at an explicit reference time. For game projects that reference is zero; live Game time and permitted runtime render commands can produce a different current result. Pass presence and this reference calculation do not establish live readiness.

Selecting a variant commits the authored setting immediately; no separate Apply button is needed. Game/Scene graphics refresh automatically, including during paused Play, while an image using a new GPU candidate depends on successful preparation. Execution-graph changes retain their existing Stop requirement. Preparation feedback distinguishes running work and failures; an idle preparation queue does not by itself prove the displayed image matches the latest authored settings. Open Game/Scene and inspect the result.

Saved, dirty, draft and prepared are separate states. **Save Project (Ctrl+S)** stores the choice for reopening and delivery; resolve pending drafts through their existing Apply/discard or file-operation guards. Successful preparation does not save the project, and an in-memory choice does not prove an exported Player contains it. On a rejected settings commit or incompatible wire, the previous authored version remains. A failed renderer candidate retains the previous working renderer when available; the rejected authored edit can remain for correction or Undo. Error details identify the failure. Removing a pass cleans its associated controls and conditions in the same history edit; Undo restores that edit as a whole. Fresh placement creates a new pass with defaults rather than recovering all earlier custom values.

The [0.4.1 verification record](release-041-verification.md) defines the evidence and remaining limits separately from these implemented UI rules; this section does not claim human usability or native/package acceptance.

<a id="player-menu-preferences"></a>

## A setting in the game's menu and local player preferences

A game Custom Rust system binds a control to an Engine-exposed variant-group **setting ID**, such as `antialiasing`. It reads `ctx.setting(id).options` for option IDs and labels, `selected` for the effective option, and `source`, `persistence` and `rejection` for status. It requests a permitted option with `ctx.request_setting(id, option_id)`. The author owns the menu's title, position, labels and order; neither a render-pass ID nor a copied list of options is needed. The [Custom Rust example](custom-rust.md#script-settings-menu) uses `ctx.script_ui` and instance-scoped `ctx.ui_action` input. Team can edit its authorized gameplay source without gaining topology rights.

The resolver exposes only bindable persistent parameter/enabled variants. Every option must define the same complete set of targets and differ from the other options. Resource-operation recipes, overlapping targets owned by different groups, repeated target commands within an option, incomplete target sets and identical option commands make a group unavailable for this binding. A removed/unbindable setting hides its Choice control. A bindable group whose effective controls do not identify exactly one option has `selected: None` and is shown as **Runtime controlled**, rather than claiming an arbitrary option.

Persistence is opt-in through `game.player_preferences`:

~~~json
{ "player_preferences": { "namespace": "com.example.my-game" } }
~~~

Keep the namespace stable across builds and package relocation; it identifies this game's local preferences, not its current folder or version. The application supplies a per-user writable state root. The standalone Player can use a separate profile via `--state-root <absolute-profile-folder>`. This uses the selected persistence service and native feature; it does not implicitly activate an omitted module. Without an available persistence owner, an accepted choice is session-only and must say so.

Without a valid saved local choice, the current authored Project choice/default and its conditions apply, followed by permitted raw runtime controls. For a group with an accepted local choice, **local choice overrides that group's conditions and raw commands**. Untouched groups keep their existing precedence. An obsolete saved option is ignored for active resolution and reports fallback to the current valid Project choice; removed setting data can remain in the opaque saved payload. Unknown payload fields are preserved. Malformed/incompatible saves are not silently replaced.

A refused request preserves the previous accepted selection, saved preference and working state and reports **Refused**. A valid accepted choice whose write fails remains active for this session and reports **Not saved / works until exit** with the storage error; this is distinct from refusal. Choosing the current option again can retry saving. An accepted selection and the currently displayed frame are separate: **Applying** remains visible while their setting snapshots differ. Authored Project files and sealed package files are not rewritten by player preference changes. Host transaction, storage ownership and exact SDK rules are in [shared-game-host.md](shared-game-host.md#player-settings).

In the previous demo project.json, select object 1 in Object Tree. The Legacy Object Inspector shows game.motion.speed of the game.motion component. Its compiled controller reads the saved value; this is a separate typed PropertyStore example, not the game controller of the new game-project.json:

~~~rust
if let Some(speed) = ctx.component_property(1, "game.motion", "game.motion.speed")
    .and_then(|value| value.as_f64())
{
    ctx.set_position(1, [(ctx.time * speed).sin() as f32, 0.4, 0.0]);
}
~~~

Other objects do not receive the field merely because a descriptor is installed. Attach adds participation; Detach archives values. Disabling a feature module removes its contribution from active views/runtime while preserving data for restoration. The archive is included in Full save/export and local history, and excluded from a new Team delivery.

In the legacy PropertyStore, persistent values are bound to the demo's initial objects, while Transform/Color in the old Object Inspector change the live preview. For saved game scenes, use Game / Scene: Transform/Color, components, references, prefab and overrides are written to game.scenes. New objects are created in this authoring window; runtime spawns during Play are not transferred to the source automatically. Actor/health/melee settings in the new example belong to those components, without the legacy Scalar chain.

PropertySchema contains a namespaced id, label, scope, value_type, default, requires_modules/components. Scope: Global, Module, Object, Component {component}. Types: finite Number with inclusive min/max, Boolean, Enum and Text with a UTF-8 byte limit. Visibility depends on the actual active modules/component participation.

Global/Module fields are edited in Project Settings; Object/Component fields are filtered by the selected object. resolved_properties supplies defaults; the archive is not passed to ScriptContext. ctx.parameter/property/component_property and ctx.properties.global/modules read active resolved values.

Engine Exposure binds an exposed numeric scalar to a pass ID/name/range. VariantGroup contains ready variants of allowed RenderCommand. Engine defines the contents/order of variants; Project selects variants/conditions. The current renderer parameter boundary is numeric; Boolean/Enum pipeline choices are represented by ready variants.

Compiled commands are checked against the same policy: an undeclared target, out-of-range value or forbidden pass is rejected before state changes. Legacy schema1/2 is read; GUI/Team initializes the new policy. Team requires initialized:true.

The Networking schema exists only in tests. This mechanism lets a module contribute its own fields; it does not mean a networking subsystem is ready.

<a id="компоненты-игрового-маршрута-04"></a>

## Components in the 0.4 game route

SystemDescriptor.components declares the contribution of a connected game system. The normal inspector filters by active module and whether the object has the component; a separate Advanced JSON view shows the saved authored archive. The game receives only active components, and Team/Player export prepares the same copy. The current form's field structure comes from JSON values: complete enum/range/help schemas for game components are not yet provided by every native module.

A template stores defaults; an individual edit to an inherited field is recorded as a separate dot-path override. Editing defaults affects other instances. System order, bindings, after and the list of connected modules belong to Engine and are frozen for Team. Details and buttons: [editor-game.md](editor-game.md); data format: [scenes.md](scenes.md).
