# Custom Rust 0.4

[English](custom-rust.md) | [Русский](ru/custom-rust.md) | [Documentation](README.md)

Custom stores real Rust in custom_sources[source_id]. The visual representation helps edit known fragments; the compiler checks and executes the entire source. Visually unsupported code is preserved as opaque text rather than replaced with approximate behavior.

<a id="исполнение-и-применение"></a>

## Execution and Application

Two entry points are supported. One source_id in the graph belongs to one mode: speed or update; using the same ID in both node types is rejected by graph validation. One file can contain helper functions, but the node mode determines the selected entry point.

In `game.execution`, a `custom.rust.update` node has required bindings `world: GameWorld` ReadWrite, `input: GameInput` Read, and `events: GameEvents` ReadWrite. The Gameplay Systems window displays them; an incorrect type or missing binding is rejected during candidate validation. Code is selected through `params.source_id`, and additional components it uses through `params.components`. This is a separate game scheduler contract; visual editing of Rust itself remains partial, as described below.

Scalar example for the previous graph Runtime:

~~~rust
fn speed(time: f64, base: f64) -> f64 {
    base + time.sin() * 0.25
}
~~~

~~~rust
fn update(ctx: &mut ScriptContext) {
    let x = ctx.time.sin() as f32;
    ctx.set_position(1, [x, 0.4, 0.0]);
    ctx.set_color(1, [75, 205, 245, 255]);
    ctx.set_render_parameter("tonemap", "exposure", 1.4);
}
~~~

The ID "tonemap" here is the pass node ID, not its module name. In the legacy root Graph, a custom.rust.update node emits WorldCommands and RenderCommands; only connected outputs execute. For both destinations, connect commands to the world and renderer. In game.execution, a node of the same module installs the GameHost adapter: world/render commands and events pass through the shared tick transaction without these legacy output pins. Changing ctx.objects itself does not change the world in either route: use commands.

In the editor, the **Apply Rust** button starts candidate compilation in the background. A visual edit also changes the source and requests compilation. Success replaces the active executable; failure preserves the previous working version, erroneous text, and diagnostics. If the first update compilation fails, there is no active command program yet. For speed, a limited scalar preview is retained until a compiled version exists.

ScriptCompiler creates a Cargo worker with ordinary rustc type checking. Dependencies are std, module-engine-core, and serde_json from the root Cargo.lock. The worker lock is checked as a subset of root packages by name/version/source, followed by an offline --locked build. Adding arbitrary external crates to Custom has no UI/contract yet.

The native target can be set through `CARGO_BUILD_TARGET` or `[build].target` in Cargo config. The compiler takes the actual executable from Cargo JSON, including the `target/<triple>/debug` layout, rather than assuming `target/debug`. The worker targets the current native host: a generated build script confirms HOST=TARGET, and PE/ELF format and architecture are checked before the SDK probe. Cross-target Custom is rejected; a standalone Player for another OS is built on that OS. Build errors give the editor readable Rust diagnostics; logs and JSON size are bounded.

Cleanup after building or cancellation touches only confirmed products of its own worker package in default/native profile scopes. Shared dependencies and similarly named user files are preserved. When the compiled-program store is enabled, successful root-package products are retained for a real Cargo revalidation until the entry is released; the active instance retains its own separate lease. The copied executable lives until the last owner releases it; the Linux runnable copy stays on the native target filesystem even when sources are on a mounted Windows drive.

Workbench uses one bounded store for Game, Scene, and Rust application. Stop releases the game session but retains verifiable compiled code: a new Play creates a new worker and initial state. Reuse still checks inputs by content and runs Cargo `--offline --locked`, requiring actual `fresh=true` for the root binary. Changed or unconfirmed inputs require a new build; this is not unconditional execution of an old executable. Proof boundaries, limits, and counters are described in [custom-program-store.md](custom-program-store.md).

The compiled worker remains running between frames and exchanges bounded JSON-lines packets. This eliminates starting a new process on each tick. Working state is stored explicitly in `ctx.state` and belongs to the host; the candidate returns it with commands. The host saves it only after successful validation and application of the entire packet. On timeout, panic, an invalid packet, or rejected commands, the previous state is preserved and the worker is stopped. `static` and globals are not a preservation and migration contract: use declared state. Format, compatibility, and API are described in [custom-state.md](custom-state.md).

The current editor call limit is 150 ms; the API accepts an explicit timeout. Compilation has a separate limit. The worker runs in the package root; for Team, Core sources come from .sdk and game assets from the parent package. SDK/lock/tools are selected from the current distribution; a missing SDK does not borrow the original native workspace. This is ordinary Rust with access to the OS, filesystem, and processes: the worker is not a security sandbox. Exported Player loads a prebuilt executable and `script.json`; Rust, Cargo, and SDK are not needed for this launch.

<a id="переносимое-управление"></a>

## Portable Control

ScriptContext contains time, dt, base, objects, world_queries_available, permitted parameters, active resolved properties, and accumulated commands. Version 0.4 adds state/state_schema, input, and events. ctx.parameter/ctx.property/ctx.component_property read settings; global/module data is available through ctx.properties.global/modules. The inactive archive is not passed in. With an initialized Engine policy, render commands are restricted to exposed ranges and ready variants, including for compiled Rust. For legacy render-only Update, a provider without WorldObject reading can supply a context with objects=[] and world_queries_available=false; the script distinguishes an unavailable query capability from an empty world. World commands require corresponding provider support. In an old JSON context without world_queries_available, the value is false: query capability is not considered declared.

<a id="named-object-references"></a>

For scheduled GameHost Update, `ctx.object(id)` reads an object and its parent ID,
and `ctx.reference(id, name)` reads a named reference target in the same snapshot.
All IDs are exact `u64` values. The optional `references` field is a map from
source object IDs to named target IDs. Older v3 workers ignore the additional
JSON field. Updated workers read an absent field as empty, including for legacy
Update and Speed. A script using `ctx.reference` needs the updated SDK sources.
SDK v4 keeps the same reference format and world commands.

The same snapshot supplies the native local transform through
`ctx.object(id)?.transform` and only active, registered schema components through
`ctx.component_property(id, component, key)`. The latter looks up an exact
top-level JSON key, including a key containing a dot. Read nested objects and
arrays from the returned JSON value. Absence returns `None`, and a present null
returns `Some(Value::Null)`. Scene validation and the worker packet use the
same [component registry](component-schemas.md), so values may include effective
schema defaults. Saving a scene writes its authored component data.

A nonfinite Transform in a request or returned command produces a packet
error. The host discards that tick and retains the last committed script state.

| World | Renderer |
| --- | --- |
| object(id) — read local state. | set_render_parameter(pass, name, value) — a finite numeric scalar. |
| set_position, translate, set_transform. | enable_render_pass(pass, enabled). |
| set_color, spawn, remove. | clear_resource(name, value), copy_resource(from, to). |

Transform is local to parent; remove deletes a subtree. Built-in worlds validate the entire packet: IDs/parents, finite values, absence of cycles, and limits. An error does not leave a partially modified world. An old third-party WorldProvider may support a scene snapshot while explicitly refusing edits.

Parameters and enabled remain in RenderOverrides; clear/copy apply to one frame. Resource names take the form pass_id.output_name, such as geometry.color. Vulkan checks resource existence, format/semantic compatibility, and operation order relative to producer/consumer. Only a pass with a valid compatible bypass, such as SSAO/FXAA, can be disabled. Geometry, HDR→LDR conversion, or color-encoding passes cannot be skipped arbitrarily.

In Team, only the preauthorized gameplay Update source is editable; scalar speed and shaders belong to the frozen Engine. Text may be erroneous and saved for repair, but the key set and bindings do not change.

<a id="script-settings-menu"></a>

## Settings and author-owned game menus

In the GameHost `game.execution` Update route, `ctx.setting(setting_id)` returns an optional `ScriptSettingSnapshot`. The stable setting ID is an exposed variant-group ID, not a render-pass ID. `options` contains current `ScriptSettingOption { id, label }` entries; enumerate it to build a custom option list. `selected: Option<String>` is the effective option, with `None` for ambiguous runtime controls. `source` is `local`, `project`, `runtime` or `mixed`; `project` includes defaults and conditions. `persistence.enabled/saved/error` and `rejection` distinguish durability from a refused request. The legacy root Runtime does not fabricate these GameHost capabilities.

`ctx.request_setting(setting_id, option_id)` queues a `ScriptSettingRequest` and returns a local validation result. The host revalidates the complete candidate against current settings and renderer contracts before accepting it; a successful helper call is not proof of a committed frame or durable save. See [precedence and local profiles](settings.md#player-menu-preferences).

The author may emit `ScriptUiPanel` values through `ctx.script_ui`. Panels have a stable `id`, authored `title`, logical-point `position` relative to the Game viewport, `width`, and ordered `Text`/`Choice` items. A Choice binds to a setting ID and the native renderer reads its current allowed options and selection from the host; the script does not send a stale selected value or duplicate the option list. The author controls the menu's content and placement. For example, an authorized gameplay Update can bind the configured anti-aliasing setting:

~~~rust
use module_engine_core::{ScriptUiItem, ScriptUiPanel};

fn update(ctx: &mut ScriptContext) {
    const SETTING: &str = "antialiasing";
    if ctx.setting(SETTING).is_none() {
        return;
    }
    let choice = ctx.ui_action("edge-choice")
        .and_then(|value| value.as_str()).map(str::to_owned);
    let problem = choice.and_then(|option| {
        ctx.request_setting(SETTING, &option).err()
    });
    let mut items = vec![ScriptUiItem::Choice {
        id: "edge-choice".into(),
        label: "Anti-aliasing".into(),
        setting: SETTING.into(),
    }];
    if let Some(text) = problem {
        items.push(ScriptUiItem::Text { text });
    }
    ctx.script_ui.push(ScriptUiPanel {
        id: "graphics-menu".into(),
        title: "Graphics".into(),
        position: [16., 16.],
        width: 320.,
        items,
    });
}
~~~

`ctx.ui_action(control_id)` reads the chosen option ID as a JSON string for this Custom **instance**. Native actions use `script_ui_action_key(instance, control)` rather than splitting IDs on punctuation. Panel IDs are unique within an instance, and control IDs are unique across that instance's panels; different instances may reuse IDs. Emit panels each accepted invocation to keep them visible; an empty output clears that instance's menu. Limits are 16 panels, 128 items and 128 setting requests per invocation, within the existing bounded packet contract.

Menu interaction owns its pointer, popup and keyboard input rather than forwarding a click into the game world. Removed/unbindable settings hide their controls. The menu reports **Applying** when accepted and displayed-frame selections differ, **Refused** for an invalid request, and **Not saved / works until exit** for an accepted choice whose storage failed. Clicking the current option can retry persistence. A malformed menu candidate preserves the previous accepted description; the UI can keep it disabled with its error instead of accepting new input from the rejected packet.

<a id="worker-protocol"></a>

### SDK and worker protocol

The Custom API, including [synchronous physics queries](physics-rapier.md),
uses SDK `0.4.0` with the exact data/API fingerprint
`module-engine-data-api/v4`. The selected `engine-sdk.json`, linked Core,
worker SDK probe and Engine/Player pins must agree.

Worker protocol 4 carries the physics requests and final responses as JSON
lines on a dedicated response pipe. The host sends requests and query results
on stdin. Worker stdout, including Windows CRT and inherited child stdout,
goes to a sink. Stderr remains a bounded diagnostic stream. The 1 MiB packet
limit applies to the JSON payload before the line terminator.

Player checks its package's SDK pin and each loaded `script.json` worker
protocol. It requires protocol 4 for SDK v4 workers from this build. Earlier
protocol 2 and 3 exports need the Player and workers they were built with. A
protocol 3 worker from an earlier SDK v4 physics build also needs a fresh
export to run with this Player.

For a source project that opens under SDK v4, export a fresh Team or Player
folder with this Full Editor. Export recreates the SDK or prebuilt workers
and the package pins together. This also replaces a protocol 3 Player export
from a v4 source.

Normal Open rejects a saved project with active Custom pinned to SDK v3
before Full authoring commands become available. Use the previous compatible
Full with that source and keep its matching Player export. **Tools → Publish
Engine profile / update contracts** operates on an already opened project.
The existing explicit copy migration handles document formats 1–3.
[Engine profiles and migration](engine-profiles.md#миграция-существующего-проекта).

<a id="визуальные-scalar-выражения"></a>

## Visual Scalar Expressions

CustomProgram understands an ordinary function without generic parameters, with named f64 arguments and an f64 result; it selects speed or the only free function. Supported constructs are sequential let, references to previous bindings, finite f64 float literals, parentheses, + - * / %, unary minus, a final expression, or return expr;.

Supported functions: sin, cos, tan, abs, sqrt, exp, ln, floor, ceil, round; min, max, powf, atan2; clamp. Calls are allowed as methods or f64::function. Changing a number/operator and reconnecting supported f64 nodes changes the corresponding Rust ranges. Cycles, invalid types/ports, unknown typing, and use of let before definition are rejected without changing the source.

Integer/f32 expressions, branches, loops, macros, arbitrary calls, and attributes affecting semantics remain opaque to scalar preview. If the active function contains an unsupported construct, the interpreter refuses it. That Rust can still compile successfully as a worker. syn checks syntax; rustc performs general type checking.

<a id="визуальные-команды"></a>

## Visual Commands

CommandProgram displays known ctx calls at the top level of update and their parameters. Numeric literals, strings, and bools can be edited, including individual position/color array values. u8/ID ranges and finite floats are checked; suffixes, comments, and the remaining Unicode text are preserved. Calls after ctx is shadowed are treated as opaque.

Rust text determines command order. Reconnecting command parameters and a full representation of loops/conditions, functions, and all Rust types are not yet implemented. They are edited as text and compiled normally.

<a id="раскладка-и-сохранность-текста"></a>

## Layout and Text Preservation

Manual positions are stored in params.code_layout, while params.code_layout_keys stores structural keys for matching after reparse. The editor transfers positions only when the node kind and its nearest connections uniquely match in the old and new graph. Changed literal values are not part of the key. Ambiguous matches and changed structures receive a default position; this is local matching, not persistent AST IDs. Old code_layout without keys is read for saved IDs. Full stability under copying/moving arbitrary code remains further work.

The source is saved even on a parsing or compilation error. Unknown fragments are not rewritten in full; graphical edits patch supported ranges. On a syntax error, the last valid visual representation may remain visible, but its editing is disabled until the text is repaired.
