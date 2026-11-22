# Gameplay ECS storage

[English](ecs-world.md) | [Русский](ru/ecs-world.md) | [Documentation](README.md)

`GameState.world: EcsWorld` owns mutable gameplay data. In R1.2 each native row
has an immutable Bevy `StableId(u64)` component. A private ordered index maps
the stable ID to a generation-checked `Entity`. Transform, parent, color,
named references and demo motion live on that row. Schema component values
remain in their existing JSON columns keyed by stable ID.

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
values remain JSON described by registered component schemas. R1.3 covers
native codecs, and R1.5 covers the scheduler.

`component_mut`, `insert_component`, `remove_component`, `transform_mut` and
`references_mut` edit the canonical world. A native system may stage intermediate
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

The executor checkpoints the complete world together with portable system state,
resources and events. Custom Rust receives an IPC DTO and returns commands; those
commands apply to an ECS candidate before the renderer controls and worker state
are committed. Failed candidates retain the previously accepted state.

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
