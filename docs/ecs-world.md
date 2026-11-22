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

`entities_with` intersects component columns, starting with the smallest column,
and returns stable ID order. An empty requirement selects every entity. Dynamic
values remain JSON described by registered component schemas. R1.5 covers
the scheduler.

`component_mut`, `insert_component`, `remove_component`, `transform_mut` and
`references_mut` edit the tick's view of the canonical world. A native system may stage intermediate
values, but the whole tick must pass structural, reference and active schema
validation before publication. `upsert_entity` replaces a complete boundary DTO
without beginning a new object lifetime; its references are checked at the
publication boundary, allowing a packet to create mutually referring objects.

`entities_snapshot` and `entity` return owned `GameEntity` DTOs for Inspector,
serialization, IPC and validation. Changing such a DTO does not change the world.
Apply it explicitly or issue commands. Native hot loops use column queries.

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

On success, the executor validates the candidate, publishes any compatibility
provider commands, runs system commit hooks, and applies the journal to the
canonical ECS. On error, it discards the journal and restores the portable
system state, resources, events and session metadata. Existing rollback hooks
restore external targets, including published Rust worker state and renderer
controls. A retry starts from the last accepted tick.

An unchanged ECS world with no populated active semantic validators needs no
world traversal for the transaction.
Ordinary writes validate only touched objects and named references. Topology
changes or wholesale world replacement can still require a full structural and
reference check. A dirty world with active schema components still runs full
schema validation when those components are present, including cross-object
references. Source semantic validators for populated active components also
run on unchanged worlds because they can consult external state.
The compatibility provider projection, Custom Rust IPC snapshot and rendered
`GameHost::step` presentation snapshots can still traverse the world. Explicit
clones for lifecycle scratch state and scene operations retain their semantics.

The default ECS session has no secondary provider. Object and explicitly
registered third-party worlds remain compatibility projections of portable object
data. Their validation/commit/rollback still participates in publication, and
their private caches survive ordinary ticks. New gameplay capabilities belong in
the canonical ECS.

## Saved compatibility

The `GameState` wire representation still uses the `entities` map of stable IDs
to DTOs, emitted in stable ID order. Loading rejects zero, duplicate and
mismatched IDs, invalid hierarchy, nonfinite transforms and missing named
targets before returning a world. Authored scene files, prefab instances and
sealed Player data keep their existing formats. GameHost sends IDs, parents
and, where present, [named references](custom-rust.md#named-object-references)
through Rust IPC as exact `u64` values. The portable data omits Bevy handles.

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
| 200 | 0 | 1532.570 / 1662.100 | 0.653 / 1.000 | 1935.508 / 2128.100 | 184.513 / 231.600 |
| 200 | 2 | 1066.685 / 1499.800 | 4.445 / 6.000 | 1378.088 / 1883.100 | 200.252 / 254.600 |
| 2,000 | 0 | 17251.587 / 20035.200 | 2.024 / 5.200 | 22155.184 / 25306.400 | 2437.820 / 2826.200 |
| 2,000 | 2 | 16784.722 / 19349.800 | 10.851 / 19.600 | 21450.796 / 24151.400 | 2380.663 / 2876.900 |
| 10,000 | 0 | 103857.167 / 111397.400 | 6.793 / 8.000 | 130191.694 / 139514.300 | 14993.799 / 15829.000 |
| 10,000 | 2 | 104344.325 / 110555.400 | 23.720 / 27.700 | 130599.360 / 138174.800 | 15049.292 / 16053.900 |

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
