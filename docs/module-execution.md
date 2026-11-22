# Executable systems and the authored schedule

[English](module-execution.md) | [Русский](ru/module-execution.md) | [Documentation](README.md)

The original render graph describes the selected world, extraction and renderer.
The optional `GraphDocument.game.execution` describes simulation. Its stages and
nodes execute in saved order. Changing this schedule is Engine authoring, while
scene/component values are Project authoring.

After graph validation, Core adds one system per saved node to a single-threaded
`bevy_ecs::Schedule` and explicitly orders consecutive nodes across stages. Each
scheduled call runs the node's registered instance, including native ARPG and
compiled Rust Custom. All nodes work on the same staged tick, so later nodes see
the world, resources and events written earlier in that tick. The schedule uses
the factories available in the selected composition. A Crate Garden composition
can run without registering ARPG.

## Why this boundary exists

Gameplay previously required the host to recognize special nodes. `SystemRegistry`
now installs a `SystemDescriptor` and an executable factory. `SystemExecutor`
creates every configured instance from its factory and invokes its lifecycle. A
new module does not require a new branch in the executor. The tests register a
counter producer and consumer that are absent from the built-in registry and
verify their execution and rollback.

Each descriptor has a module ID, explicit state schema, typed resource ports and
optional preceding module requirements. `components` declares the component
types consumed by this system. Their fields/defaults come from separately
registered component schemas, and a schema owner need not be a scheduled system.
Inactive data are absent from runtime entities without deleting authored values.
A node's `params.components` can explicitly declare additional component
types consumed by Custom Rust. Each saved `SystemNode` has a stable
instance ID, module ID, parameters, pin-to-resource bindings and explicit `after`
constraints. Before building the schedule, Core checks `after` against the saved
stage and node order without reordering the graph. Each referenced node must
already appear earlier. Validation rejects:

- Missing factories, duplicate stages/instances/resources and unknown bindings.
- Resource types that do not match the declared port type.
- Reading a resource before an earlier node has produced it, unless external.
- A required predecessor appearing later or missing entirely.

External resources are host-owned initial values. Current gameplay declares
`GameWorld`, `GameInput`, `GameEvents` and produced `Positions`. `GameWorld` is a
typed capability exposed as `SystemContext.game`; it is not copied into a large
JSON blob for every node. `world(port)`/`world_mut(port)` check a declared service
binding and return the portable world; `resource(port)` explicitly rejects a
GameWorld service rather than returning its null envelope. Input and events also
have typed context access through `input_resource`, `events_resource` and
`events_resource_mut`. Their JSON resource envelopes are refreshed before each
module: the input is the current host input, and events contain earlier modules'
events. Within a module the typed events access is canonical; its JSON envelope
is an entry-time snapshot. Custom
resource types use `ResourceStore` JSON envelopes and exact type-name comparison.
They do not yet provide a general language-independent binary ABI or schema
checker for arbitrary custom JSON payloads. Native source is trusted and can use
the public context directly even without a pin; metadata declares scheduling and
editor contracts, not a security boundary.

The native movement/collision `Positions` resource now publishes each ID as
`{"position":[x,y,z],"lifetime":token}`. Collision also reads the previous
coordinate-array representation for compatibility. Current records prevent a
system inserted between movement and collision from applying an old start point
to a newly spawned incarnation with the same saved ID. Native consumers of this
resource should read the explicit record; it is not a stable binary ABI.

The mutable world is `context.game.world`, the canonical ECS store. Native
systems use component membership queries and column access. Owned entity DTOs
are reserved for serialization, UI, IPC and validation; see
[the ECS API and lifetime rules](ecs-world.md).

## Installing a source module

```rust
use module_engine_core::{ExecutableSystem, SystemContext, SystemDescriptor,
    SystemRegistry};
use serde_json::Value;

struct ClockCounter;
impl ExecutableSystem for ClockCounter {
    fn step(&mut self, ctx: &mut SystemContext<'_>, state: &mut Value)
        -> Result<(), String> {
        *state = Value::from(state.as_u64().unwrap_or(0) + 1);
        ctx.events.push(module_engine_core::GameEvent::Custom {
            name: "counter".into(), payload: state.clone(),
        });
        Ok(())
    }
}

fn install_counter() -> Result<SystemRegistry, String> {
    let mut registry = SystemRegistry::builtin();
    registry.register(SystemDescriptor {
        id: "example.counter".into(), state_schema: "counter/1".into(),
        components: vec![], ports: vec![], requires: vec![],
    }, |_| Ok(Box::new(ClockCounter)))?;
    Ok(registry)
}
```

Add its `SystemNode` to the saved execution graph, then construct
`GameSession::new_with_registry(project, world_kind, registry)`. Native source
modules currently require rebuilding the application, as in the earlier stage.
The host may register a compiled Rust Custom adapter through this same boundary.

The installed `custom.rust.update` adapter declares `world: GameWorld` ReadWrite,
`input: GameInput` Read and `events: GameEvents` ReadWrite. Its saved node must bind
all three services to matching declared resources, just like a native gameplay
module; the canvas shows these ports and validates their connections. The node's
`params.source_id` selects code from `custom_sources`, and optional
`params.components` declares extra component names that this code consumes.

## Lifecycle, state and errors

`initialize` runs before an instance's first tick. `step` receives the fixed tick,
typed input, mutable game world, events, resource bindings, and that instance's
owned JSON state. `stop` runs in reverse order when a scene is replaced/restarted
and on session shutdown (Editor Stop or Player exit). Each initialized instance
is stopped once; shutdown tries all hooks even if one fails. Drop performs
best-effort cleanup because destructor errors cannot be returned to the caller.
During panic unwinding, Core takes back the live instances from the schedule
before propagating the panic, so session `Drop` can still attempt `stop`.
If the caller catches a panic from a scheduled node, drop the affected session
and start a new one before running more ticks. The interrupted tick may have
left the in-memory session partially updated.
Put persistent state in the supplied state value instead of an unobservable
process-global variable. The built-in combat module stores cooldowns, windup,
attack direction and already-hit targets in that state.

A tick rejected with an error discards the [ECS journal](ecs-world.md#lifetime-and-transactions)
and restores session metadata, owned system states, resource values and events
to their pre-tick values. Systems stage external outputs during `step`.
`SystemExecutor::step_with_transaction` runs every system step, every
`validate_tick`, then the host `TickTransaction::validate_changes` and `commit`,
and then the system `commit_tick` hooks. `validate_changes` receives the previous
state and defaults to the existing `validate` hook. In `GameSession`, the host
validates the candidate and applies a portable command packet to the selected
compatibility World provider, if present. The ECS journal is applied after all
commit hooks succeed.
Thus a provider rejection happens before script state or renderer overrides are
published. The plain `SystemExecutor::step` uses a no-op host transaction.

A failure at any step, validation or publication invokes `rollback_tick` on
entered systems in reverse order, rolls back the host transaction, then restores
owned values. A rejected scheduled node prevents later nodes from entering the
tick. The compiled Rust adapter uses these hooks to retain the previous
script state and renderer overrides. A custom host target must implement
`TickTransaction::rollback` for changes outside the executor's owned data. Native
module effects such as file writes, OS calls and hidden mutable fields require
their own rollback; they are not made reversible by the JSON snapshot. A rollback
failure is reported explicitly. Source modules are trusted code, not sandboxed
plugins.

`replace_system(instance, registry, migration, reset)` prepares a replacement
without changing the active implementation. Its ports (including types/access),
component contributions and preceding module requirements must match the
installed contract. Matching state schemas retain portable JSON state; a
different schema requires an explicitly supplied migrated value or `reset = true`.
Contract, schema and factory failures retain the active and any already prepared
replacement. `SystemExecutor::states()` shows the prepared portable state, or
null for a reset awaiting initialization. Repeated preparations supersede the
uninitialized candidate; shutdown cancels it without lifecycle calls.

Activation waits until the next tick reaches this instance's authored schedule
position, so preceding producers run before its `initialize`. Every replacement
is initialized, including those retaining or migrating state. Its initializer
receives the selected portable state, but that explicit retained/migrated value
wins over any JSON defaults the initializer writes. A reset or an instance
replaced before its first initialization uses the new initializer's defaults.
Initialization writes to game data, resources and events join the same host tick
transaction as `step` and publish only when the tick succeeds.

After successful initialization the old initialized implementation receives one
`stop` attempt before the candidate is installed. This replacement teardown uses
copies of its portable context/state; teardown writes are discarded. A failed
candidate initialization is cleaned up with `rollback_tick` and `stop`, then
discarded while the old implementation remains installed. A failed old `stop`
also rejects and cleans up the candidate, retains the old portable state/context,
and marks the old implementation for reinitialization on a later tick. Its stop
attempt is not repeated until it has initialized again. Native private or
external lifecycle effects still require module-owned cleanup; copying JSON does
not reverse an OS call or a private mutation.

Once installed, a replacement follows ordinary tick rollback semantics. If its
first tick or host publication fails, it remains installed with pre-initialization
portable state and retries initialization on the next tick; the retired old
implementation is not restored or stopped again. This executable boundary is
independent of the compiler-specific Custom source editor. Session transitions
and restarts still construct their systems from the session's installed registry;
preparing one live instance does not rewrite that registry.

## Simulation and rendering

`GameSession::step(real_dt, input)` advances at exactly 1/60 second per tick. It
retains an attack edge until a tick occurs and consumes that edge once when
catching up multiple ticks. Movement remains held. Rendering consumes the most
recent immutable scene snapshot independently of the number of simulation ticks.
Catchup is capped at 15 ticks per call; excess wall time is discarded, preventing
an unbounded backlog after a pause. Render interpolation and selectable simulation
rates remain future work.

`GameSession::new_with_world_factory` permits a separately implemented editable
world store. The default constructors use the actual independent Object and ECS
stores, not a single dictionary disguised as two providers. Both consume the same
portable objects and produce equivalent snapshots.

The session retains one World provider across fixed ticks and direct
`GameSession::apply_commands` calls. It creates a replacement only when entering a
scene, transitioning or restarting; provider-specific storage and caches are not
rebuilt every tick. A provider must expose stable portable objects and implement
side-effect-free `validate_commands` plus atomic, all-or-none `apply_commands`.
After publication the host checks that the provider's portable objects match the
candidate game state. If a later system commit fails, the host computes and
applies the inverse portable packet. Private provider state or external effects
must honor this rollback contract themselves; the host cannot reconstruct them
from `WorldObject` records.

<a id="r15-verification"></a>

## R1.5 verification

On Windows x64 with Rust 1.99.0, replay the recorded input and check late-tick
rollback through `GameHost` with:

```powershell
./scripts/verify-r15.ps1
```

The Windows Full checkout needs cached Cargo dependencies and the native Rust
toolchain for the checked-in ARPG Custom worker. The script runs the same input
twice, verifies the input and complete event stream against SHA-256 values
recorded on merge `99884e6794737a3e40f3128fe14d3a919388dc23`, and writes evidence
under `outputs/r15/`. With the original `input.json` and `events.ndjson` in
`outputs/r15/baseline/` (or a directory passed via `-Baseline`), it replays that
file and locates the first differing scene and tick. Otherwise it reconstructs
the recorded input from the witness moves and ARPG controls and verifies its hash.
Every accepted tick has one log row, including ticks with no events. Each event
has its type and complete data in emission order.

Crate Garden uses an ARPG-free composition and completes `level01` through
`level10` with 689 moves in 700 ticks. The ARPG replay runs the Small monsters
encounter (`arena`) from an in-memory migration of `game-project.json`, including
its real Custom worker. In 180 ticks it emits 12 attacks, 35 hits, 15 deaths and
62 sound events. The runs cover fixed-step simulation in the headless host.
Native presentation, audio and persistence effects, the boss scene and
cross-platform determinism are outside this replay's coverage.
