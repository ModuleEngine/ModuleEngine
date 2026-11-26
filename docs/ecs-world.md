# Gameplay ECS storage

[English](ecs-world.md) | [Русский](ru/ecs-world.md) | [Documentation](README.md)

`GameState.world: EcsWorld` owns mutable gameplay data. In R1.2 each native row
has an immutable Bevy `StableId(u64)` component. A private ordered index maps
the stable ID to a generation-checked `Entity`. Transform, parent, color,
named references and demo motion live on that row. `Transform` is a typed,
reflected Bevy component with a portable codec in Core's
[`ComponentRegistry`](component-schemas.md). Schema component values remain
in their JSON columns keyed by stable ID.

## Module access

Native systems select membership before reading values:

```rust,ignore
let world = &context.world("world")?.world;
for id in world.entities_with(&["example.spin"]) {
    let settings = world.component(id, "example.spin").unwrap();
    let object = world.object(id).unwrap();
    // Stage portable WorldCommands, or edit a component in this tick's world.
}
```

`entities_with` resolves component columns once and starts with the smallest
column. It intersects nearby-sized columns in stable ID order and uses keyed
lookups for larger columns when few candidates remain. The query incorporates
the current staged edits and removals even when a requested column is absent
from the committed world. An empty requirement selects every entity. Dynamic
values remain JSON described by registered component schemas. The
[authored schedule](module-execution.md) runs nodes on the staged view of this world.

`component_mut`, `insert_component`, `remove_component`, `transform_mut` and
`references_mut` edit the tick's view of the canonical world. A native system may stage intermediate
values, but the whole tick must pass structural, reference and active schema
validation before publication. `upsert_entity` replaces a complete boundary DTO
without beginning a new object lifetime; its references are checked at the
publication boundary, allowing a packet to create mutually referring objects.

`entities_snapshot` and `entity` return owned `GameEntity` DTOs for Inspector,
serialization and boundaries that need complete entities. Apply edits to an
owned DTO explicitly or issue commands. Gameplay and presentation read the live
world through Core's membership and value accessors. `transform(id)` reads the
local transform directly from the current view. The column maps and staged
rows are private to Core.

<a id="r17-consumers"></a>

## R1.7 ECS consumers

Crate Garden and Puzzle read live boards and cells with `entities_with`,
`object` and `component`. Their authored-scene validation uses the same layout
rules on resolved entity DTOs. ARPG systems and presentation select stable IDs
and read individual values. The public ARPG player DTO is still available for
callers that need the complete entity. Two Arenas supplies authored scenes and
prefabs to Core and delegates live game rules to ARPG.

`EcsWorld::decode_component` returns `Ok(None)` for an absent component and leaves
the counter unchanged in every build. A malformed value produces a decode
error and increments `EcsWorld::malformed_component_reads()` in memory, including
in Release. The ARPG and Crate Garden readers use the same gameplay fallback for
either outcome. Only builds with `debug_assertions` print the first eight
failed reads per world to stderr, with the component name, object ID and decode
error. In a standard Release build, a malformed read only updates the counter
before the gameplay fallback.

The counter is available through the Core getter. An Editor built with
`--features verification` publishes it for the controlled missing and malformed
`health` fixture under `component_reads` in the ARPG `--game-check`
`game-report.json`. These two values measure the fixture. Tick checkpoints
share their world's counter through rollback.
An explicit world clone starts with the current count and tracks later errors
independently. The counter lives outside saved game data.

GameWorker steps and renders through GameHost. The Host uses the R1.6 retained
scene path and asks Core to project the current tick's world into
`ScriptContext` for scheduled Rust. The projection includes stable-ID ordered
objects, named references, and verbatim fields from active registered
components. runtime-script transports this owned packet and the returned typed
commands. The Host prepares the commands against the same staged world before
the tick publishes. The Object provider remains a compatibility adapter.

Check the Windows workspace suites and use the
[R1.5 replay](module-execution.md#r15-verification),
[stable ID and Rust IPC fixture](#r12-verification), and
[R1.6 S3 presentation check](shared-game-host.md#r16-scene-updates) for
the corresponding integration paths. Run native checks sequentially.

```powershell
cargo test --offline --locked --release --target x86_64-pc-windows-msvc --workspace --all-features -- --test-threads=1
```

<a id="r18-storage-audit"></a>

## R1.8 storage audit

The R1.2 storage change replaced the separate maps for native fields in the
R0.6 `EcsWorld` with Bevy rows. Core still owns the stable-ID index and the
JSON columns used by component queries, codecs and tick publication. The
`entities` map belongs to the saved DTO format. The tick journal holds pending
writes, and the Object provider retains its recursive hierarchy for explicitly
selected compatibility sessions.

The production `GameSession` factory creates an `ObjectsWorld` for the Object
kind and uses the canonical `EcsWorld` directly for ECS gameplay. Tests still
construct explicit compatibility projections to exercise rejection and retry
with either built-in provider. The graph-only runtime can still construct the
demo ECS provider through `ModuleRegistry`.

The native [paired benchmark](bench.md#not-worse) uses the R0.6 S2 reference,
`game_step`: the UI-observed distribution of `GameHost::step` durations from
the player worker. A host call can service zero or several fixed ticks and
includes presentation bookkeeping. S3's `ecs_tick_ms` times the eight-column
query and 1,000 motion-payload and transform updates in the fixture. The
transform extraction and render submission have separate timers. A second
S3 probe uses `GameHost` and records the actual checkpoint metadata clones
and the entire fixed transaction separately. The two S3 routes measure
different scopes.

<a id="r19-batch-motion"></a>

## R1.9 batch motion edits

`EcsWorld::edit_transforms_with_component` edits the local `Transform` and an
existing named JSON component together. The callback runs in input order once
per matching ID occurrence. Missing IDs and missing components skip the
callback. A present `null` is passed through as a value.

On committed storage, the batch borrows the rows and looks up the JSON
column once. Bevy records each edited transform and each edited `mesh` or
`material` binding. The render cursor's write counter advances once when
the batch invokes a callback. Within a fixed tick, the same edits go through
the row journal and transform validation. Publication and rejection follow
the fixed-tick contract below.

S3 still queries eight columns and edits 1,000 transforms and motion payloads
on 2,000 ECS entities. Updating the existing `offset` member borrows its key
and writes the same JSON number. The Release probe times the full tick and
the isolated operations. Each operation reports 240 raw samples after 30
warmup batches. Isolated timings include their own loop and memory access
costs and cannot be added to get the total tick time. Run this probe
explicitly outside the automatic verification profiles.

```sh
cargo test --offline --locked --release -p module-engine-core --lib world::s3_cost::s3_motion_update_cost -- --exact --ignored --nocapture --test-threads=1
cargo test --offline --locked --release -p module-engine-host --lib r14_tick_rejection -- --test-threads=1
python scripts/bench.py --no-profiling --compare b6d9baf0a0222d5d4c25141754d1d4a67f822777 --scenes S3
python scripts/bench.py --no-profiling --compare 731661c47e181847450e00480e60c0645abd4753 --scenes S2
```

Run sequentially without competing builds or measurements. The S3 comparison
uses a post-R1.8 `main` that includes PR #38. Compare the run medians of the
mean and p95 `ecs_tick_ms` and check every scored S3 ECS sample in the raw
candidate reports against 2 ms. Apply the [paired comparison rule](bench.md#not-worse)
to S2 `game_step_ms` p95 against the rebuilt R0.6 base.

## Lifetime and transactions

Subtree removal clears every removed object's columns and outgoing references.
Surviving references to removed targets reject publication until explicitly
cleared. Removing and spawning the same ID starts a fresh lifetime: old components,
references and demo angular velocity cannot leak into the replacement.
Despawn removes the stable-ID index entry. A new ID can reuse Bevy's slot with
a different generation, and a reference to the deleted ID stays unresolved.
Ordinary updates retain the existing row and lifetime. A rejected structural
change leaves the rows and index in their previous state.

When a publication despawns rows, Core advances Bevy's removed component
message buffers once. The private world keeps the latest publication's messages
and reuses the buffers. This applies to a fixed tick and to portable commands
and `clear()` issued outside a tick.

`lifetime(id)` identifies the current runtime incarnation separately from the
saved ID. Cloning a checkpoint preserves it; spawning, rebuilding from a DTO or
clearing and recreating an object gets a fresh token. Module caches use this
token to discard per-object cooldowns and death markers on ID reuse. Tokens are
process-local bookkeeping, excluded from semantic world equality and the saved
entity DTO format. Bevy handles belong to one world. A cloned checkpoint rebuilds
its own index and retains the runtime lifetime tokens.

Per-tick intentions and collision start positions also carry the incarnation.
A source system inserted between authored stages may replace an object, so these
transient records cannot safely use its saved ID alone. Captured combat visuals
retain their original role/style, and target queries or hit flashes do not bind
an earlier action to a replacement entity.

The executor holds the last accepted Bevy rows and JSON columns read-only
during a fixed tick. The first mutable access to an entity stages its row in
a journal. Queries see the staged rows, new lifetimes and removals immediately
in authored system order. A portable command buffer validates as a batch
before its writes join the journal. Custom Rust still receives a world IPC DTO.
Its world buffer joins the journal only after its other outputs pass staging.

A nested executor checkpoint shares the earlier journal and starts an empty
layer for its own writes. Success merges that layer, and rejection restores
the earlier view. Value-only command packets validate their targets and the
transforms exposed to mutation since the last applied, validated packet.
Discarding a prepared packet also discards its proposed repairs. For retained
Bevy rows, publication marks a native component as changed only when its final
stored value differs, even when the batch also despawns rows. Rebuilding or
replacing the Bevy storage starts a fresh change history.

On success, the executor validates the candidate, publishes any compatibility
provider commands, runs system commit hooks, and applies the journal to the
canonical ECS. On error, it discards the journal and restores the portable
system state, resources, events and session metadata. Existing rollback hooks
restore external targets, including published Rust worker state and renderer
controls. A retry starts from the last accepted tick.
The checkpoint still copies the session's scene ID, `module_data`, and
control data. The cost depends on the size of those values, which source
systems can edit directly through `&mut GameState`. [R1.8 tracks and measures
this cost](product-roadmap.md#r18-session-checkpoint).

An unchanged ECS world with no populated active semantic validators needs no
world traversal for the transaction.
For ordinary writes, structural and named-reference checks cover touched rows. Topology
changes or wholesale world replacement can still require a full structural and
reference check. Spawn/remove command packets can walk the world to resolve
descendants and validate the final hierarchy. An edited world with active
schema components still runs full schema validation, including cross-object
references. Source semantic validators for populated active components also
run on unchanged worlds because they can consult external state.
The compatibility provider projection, Custom Rust IPC snapshot and snapshot-only
presentation paths can still traverse the world. Explicit
clones for lifecycle scratch state and scene operations retain their semantics.

The default ECS session has no secondary provider. Object and explicitly
registered third-party worlds remain compatibility projections of portable object
data. Their validation/commit/rollback still participates in publication, and
their private caches survive ordinary ticks. New gameplay capabilities belong in
the canonical ECS.

<a id="r16-render-extraction"></a>

## R1.6 render extraction

`GameSession::read_render_changes` reads committed rows using an independent
`RenderChangeCursor`. With writes since the last read, Bevy filters for
`Changed<Transform>`, `Changed<ObjectData>` and `Changed<RenderBindings>`.
The render binding marker tracks the lossless `mesh` and `material` columns.
Other schema writes leave these filters unchanged. A write counter skips the
query on a write-free read. Bevy's change filter can still inspect every live row
when there are writes. The retained projection resolves matrices and enriches
drawables for affected owners. The binding and transform checks compare final
committed values, so a staged write undone within the tick produces no movement.

A bounded history retains the most recent 4,096 stable-ID removals across
publications. An expired cursor or replacement Bevy world reinitializes the
projection from the current rows. The Host expands changes to transforms,
parents and lifetimes through the affected hierarchy. Final drawable comparison
filters out changes to unrelated data, unchanged world matrices and unchanged
materials, including multiple writes before a render.

See the [renderer synchronization contract and S3 verification](shared-game-host.md#r16-scene-updates).

<a id="saved-compatibility"></a>

## Saved compatibility

The `GameState` wire representation still uses the `entities` map of stable IDs
to DTOs, emitted in stable ID order. Loading rejects zero, duplicate and
mismatched IDs, invalid hierarchy, nonfinite transforms and missing named
targets before returning a world. Authored scene files, prefab instances and
sealed Player data keep their existing formats. GameHost sends IDs, parents
and, where present, [named references](custom-rust.md#named-object-references)
through Rust IPC as exact `u64` values. The portable data omits Bevy handles.

Objects and ECS require nonzero stable IDs in their constructors, in portable
command validation and in `Spawn` application. A `Spawn` with ID 0 rejects the
whole batch, even if a later command would remove that object. The prior world
remains available unchanged.

<a id="r14-verification"></a>

## R1.4 verification

```sh
cargo test --offline --locked --release -p module-engine-host --lib r14_tick_rejection -- --test-threads=1
cargo test --offline --locked --release -p module-engine-host --lib r14_s3_tick_cost -- --ignored --nocapture --test-threads=1
```

The rejection test runs an accepted tick, then a failing tick in which a later
system reads earlier writes, replaces a subtree root, and returns an error.
It checks the accepted world and lifetime tokens, state, resources and events,
then retries the tick without adding time. The Host's compiled Rust tests cover
rollback of worker state and render controls and publication of committed events.

The timing probe runs the authored schedule in `GameHost` with the neutral CPU
renderer and canonical `world.ecs`. It reuses S3's IDs, transforms and eight ECS
payloads at 200, 2,000 and 10,000 entities. After 30 warmup ticks, it measures
240 ticks with either zero writes or one transform and one component write to
the same ID per tick. Each cell is the median of three sequential Release runs
on an AMD Ryzen 9 3900X, Windows x86-64, Rust 1.99.0. The baseline is
`d7389ca2d75835a50d3831effa2c99d7808ec074` with the same probe.

| Entities | Writes | Fixed tick before, mean / p95 (µs) | Fixed tick after (µs) | `GameHost::step` before (µs) | `GameHost::step` after (µs) |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 200 | 0 | 1532.570 / 1662.100 | 0.893 / 1.200 | 1935.508 / 2128.100 | 190.932 / 253.500 |
| 200 | 2 | 1066.685 / 1499.800 | 4.588 / 5.300 | 1378.088 / 1883.100 | 191.142 / 233.600 |
| 2,000 | 0 | 17251.587 / 20035.200 | 3.878 / 6.800 | 22155.184 / 25306.400 | 2628.726 / 2909.600 |
| 2,000 | 2 | 16784.722 / 19349.800 | 14.350 / 23.500 | 21450.796 / 24151.400 | 2626.197 / 2959.900 |
| 10,000 | 0 | 103857.167 / 111397.400 | 8.867 / 11.300 | 130191.694 / 139514.300 | 15686.828 / 16957.600 |
| 10,000 | 2 | 104344.325 / 110555.400 | 27.593 / 33.000 | 130599.360 / 138174.800 | 16268.487 / 17763.200 |

The inner timer includes the transaction, schedule, and the probe's single
system. The empty case leaves the world journal empty, uses a shared checkpoint,
and skips unchanged world validation. The two-write case stages one row and
validates and installs that row, with stable-ID lookups. The outer timer also
includes before/after presentation extraction and identity capture, which still
scale with world size. This is CPU simulation evidence. Native Vulkan frame
timing and the R1.8 S3 budget have separate acceptance routes.

<a id="r13-verification"></a>

## R1.3 verification

Run these checks from a Full source checkout with the pinned toolchain and
offline dependencies restored. Run the commands sequentially.

```sh
python scripts/verify.py --profile contracts --plan
python scripts/verify.py --profile contracts
cargo test --offline --locked -p module-engine-editor component_codec_corpus_roundtrips_through_editor_source_files -- --nocapture
cargo test --offline --locked -p module-engine-editor inspector_noop_apply_respects_sparse_local_values_without_a_prefab
cargo test --offline --locked -p module-engine-editor stable_ids_survive_authored_save_player_export_and_rust_ipc -- --ignored --nocapture
```

The corpus performs two fresh Editor source writes in separate temporary
folders. It compares the parsed JSON, preserving numeric type, signed zero,
object keys, member presence and array order. It also validates each game
scene with the installed composition and checks the native ECS reconstruction
against the resolved DTO. The mini-project with the compiled Rust probe checks
values against independent literals, including a `u64::MAX` member, a literal
dotted key, explicit null, absence, signed zero and the schema's effective axis.
It also exercises rejection of null for an active numeric field during Save.
A repeat Save tests a change solely from negative to positive zero in both
the native Transform scene file and the schema JSON in the project file.

The checked-in example JSONs are legacy graphs without authored scenes. The
corpus compares their format-4 copies after the explicit migration decoder.
The source files keep their bytes. JSON equality is checked between the two
fresh writes of the accepted format-4 document. This leaves equality against the
raw legacy input outside the comparison. The game cases cover Crate Garden
`level01` through `level10`, the Two Arenas template and `game-project.json`
after migration, both with `arena` and `boss`. The old SDK pin in
`game-project-v4.json` retains its existing rejection. Two Arenas is the
available technical baseline for G2. The completed G2 campaign is pending.

For a manual Editor check on Windows, choose **File → New Project → Empty
scene** in a separate folder. Add an object, set its **Position** X to
`1.25`, then install the [Rotation example](component-schemas.md) in
**Gameplay Systems**, bind its `world` port and choose **Apply graph draft**. Attach **Rotation** in
Inspector and apply a speed of `4`. Choose **Create template from selected**.
Click **Apply fields** on the instance with no edits, then change the
template speed to `6` and use **Apply template defaults to instances**.
The instance should inherit `6` and its **Advanced** JSON should keep
an empty `overrides` map. Save and reopen, then Play to see the rotation.

<a id="r12-verification"></a>

## R1.2 verification

Run from the Full source checkout with the pinned native dependencies restored.
Run the Editor checks sequentially because they compile workers and the Player.

```sh
cargo test --offline --locked -p module-engine-core
cargo test --offline --locked -p module-engine-editor existing_game_project_save_load_save_keeps_exact_bytes -- --ignored --nocapture
cargo test --offline --locked -p module-engine-editor stable_ids_survive_authored_save_player_export_and_rust_ipc -- --ignored --nocapture
```

- Core checks structural rejection and real Bevy slot reuse. The deleted
  target's slot gets a new generation, and its old named reference cannot
  resolve to the replacement. Existing checks cover ID validation and
  transactional publication.
- The existing `game-project.json` uses format 3. The test explicitly migrates
  a disposable copy, saves and reloads format 4, then compares the saved project
  and external scene bytes after the second save. It also compares the original
  input bytes.
- The integration fixture resolves a prefab instance with ID
  `9007199254740993` and both links to it. It compares two independently written
  project and external scene copies, exports through Editor, and launches the
  exported native Player. Its compiled Rust worker reads the parent and named
  target over IPC and uses the target ID in a returned world command. The
  Player's state confirms the command and preserves both links. Serializing,
  reading and reserializing that state gives identical bytes.

The native fixture uses the software renderer. The [R1.1 prototype
measurements](ecs-storage-decision.md#r11) describe their historical snapshot.
