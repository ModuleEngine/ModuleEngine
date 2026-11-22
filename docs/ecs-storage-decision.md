# Gameplay storage decision

[English](ecs-storage-decision.md) | [Русский](ru/ecs-storage-decision.md) | [Documentation](README.md)

Production: `GameState.world` owns gameplay in `EcsWorld`; the entity map is
its serialized DTO representation. See the [API and lifetime contract](ecs-world.md).

<a id="r11"></a>

## R1.1 — hybrid Bevy storage for the migration

**Proceed to R1.2 with the hybrid approach.** In a fresh paired Release
series on the S3 workload, the hybrid tick was **34.82×** faster than
`EcsWorld`; the dynamic JSON variant was **4.59×** faster. Both Bevy variants
passed the spike's authored-data cases. The R1.1 gate is **5×** on the S3 tick
plus those invariants. This decision selects the target for R1.2.

### Compared storage

- **Core** uses the existing `EcsWorld` and its JSON component columns.
- **Bevy dynamic** uses an independent table column per component ID via
  `ComponentDescriptor`, including one for an explicit JSON `null`. The tick
  mutates the JSON motion column through its resolved `ComponentId`.
- **Bevy hybrid** stores canonical motion as reflected native fields
  (`bool` and `f64`) and updates those fields through a typed Bevy query.
  A project JSON codec encodes those fields back to the same keys and value
  types. It keeps the entire payload as JSON when its shape cannot be
  reconstructed losslessly, including extensions and alternative numeric
  types. The other seven S3 components and unknown component IDs use the
  same JSON columns as the dynamic variant.

Use the hybrid approach for native components with lossless codecs and keep
JSON columns for schema-defined data. This combines the measured typed
mutation path with exact payload round trips. Reflection exposes the native
fields. The project codec owns the saved representation. The measured
all-JSON variant misses the tick gate. The hybrid restore costs more than
Core, so the result does not support restoring a full snapshot on every tick.
R1.4 owns the production transaction design.

### Matched Release measurements

Values are mean / nearest-rank p95 in ms. Lifecycle operations are for a
whole batch of 2,000 entities.

| Operation | Current `EcsWorld` | Bevy dynamic JSON | Bevy hybrid |
| --- | ---: | ---: | ---: |
| S3 tick | 0.5363 / 0.6636 | 0.1168 / 0.1251 | **0.0154 / 0.0153** |
| Spawn batch | 3.8199 / 4.3710 | 4.4148 / 4.9128 | 4.7268 / 5.2546 |
| Despawn batch | 15.9447 / 17.6617 | 14.8780 / 16.5193 | 14.7157 / 15.6583 |
| Take portable snapshot | 6.9000 / 7.9135 | 5.9892 / 6.7454 | 5.7628 / 6.4403 |
| Restore portable snapshot | 3.1994 / 3.6333 | 4.3411 / 4.9171 | 4.6022 / 5.1340 |

The speedup divides the pooled Core mean by the pooled candidate mean, using
unrounded samples. The fresh Core mean gives a fivefold target of **0.1073 ms**.
R0.6's native-window 0.756 ms mean and approximate 0.151 ms target are
[historical reference points](bench-r06-20261122.md). This series runs on
the CPU without a renderer. The native `bench.py --compare` protocol answers
the separate [not-worse question](bench.md#not-worse).

The Editor and spike share the [S3 input generator and Core tick](../examples/bench/s3_ecs.rs).
The timed tick queries membership in all eight components across 2,000
entities, produces all matching IDs in stable order, evaluates the same
motion formula at the same fixed 60 Hz fixture time and writes
`bench.motion.offset` and local Y to the first 1,000 matches. Bevy also keeps
the row handle from the query for those writes. Transform extraction,
rendering, scheduling, transactions, and snapshot work are outside this
timer. Spawn and despawn use checked batches of leaf objects, with input
preparation outside the timer. A portable snapshot is an owned stable-ID
`GameEntity` collection with transforms, parent, color, components and
references. Restore validates hierarchy and references while rebuilding
the store. JSON encoding, I/O, and runtime checkpointing are separate.

### What was verified

- Both Bevy variants matched the full ordered Core selection at every S3
  frame, and all entity values and transforms at frames 0, 29, 30 and 269.
  Reordered rows and a missing component changed the selection and writes
  in the same way in all three stores.
- A resolved prefab preserved the stable ID `9007199254740993`, the parent
  and named reference to it, the exact key `Exact-Key`, the distinct
  `source.Widget.Other` namespace, and nested unknown JSON. Explicit
  `null` stayed distinct from absence, integer `0` stayed an integer, and the
  typed float case kept negative zero. The stable-ID DTO map was byte-identical
  after save → load → save. The authored fixture kept its original bytes.
- A staged candidate with a missing reference target also changed a
  transform, component payload, tick counter, resource and events. Rejecting
  it kept the previous entity bytes, Bevy handles and all probed tick state.
  A subsequent valid candidate published successfully.

These results cover the standalone spike fixtures. The production stable-ID
contract and its integrated checks are described in [ECS world](ecs-world.md).
The CPU comparison does not establish native frame or game-step gains.

### Reproduction and evidence

From the repository root at the R1.1 merge
`ae2b06b0194d21bc2c64e41af03f631bace3dbe2`, with the native toolchain and
Python 3.9+. This revision supplies the original Core comparison store.

```sh
cargo fetch --locked --manifest-path crates/ecs-spike/Cargo.toml
python crates/ecs-spike/run.py outputs/bench/r11-reproduction
```

The runner uses `cargo test` and `cargo build` with `--release --offline
--locked`, the isolated manifest and `target/ecs-spike`. It runs one unscored
process per variant, then six scored rounds in all six variant orders. Each
variant runs in a fresh process, sequentially, from the same executable. Each
process discards 30 tick warmups and five lifecycle warmups, then records
240 tick samples and 30 samples per batch operation. Pooled counts per
variant are 1,440 ticks and 180 samples per batch operation.

Windows 11 Pro build `10.0.26300.10075`, Ryzen 9 3900X (24 logical CPUs),
AMD Ryzen Balanced, default CPU affinity. Rust/Cargo 1.99.0,
`x86_64-pc-windows-msvc`, Release `opt-level=2`, Python 3.9.13.
`bevy_ecs` and `bevy_reflect` 0.20.1 were confined to the
[standalone crate](../crates/ecs-spike/Cargo.toml) and its lockfile.

| Identity | Value |
| --- | --- |
| Base and measured HEAD, after PRs #20 and #21 | `3cc21936ee6d92ebbf5df9c1db297be3e5175b27` |
| Measured source digest, including the spike worktree | `436a98b0abb43f2c79fddefa7bee315dffb6b2df88bd8a7fae4270bc5d7c87ef` |
| Measured executable SHA-256 | `5b624e48b6d810810da0ef00f735713228f1c359167b461a1d2c27b2deb24033` |

The local raw results are in `outputs/bench/2026-11-22/r11-ecs-spike-02/`.
`summary.json` includes per-round statistics and both speedups.
`run-*.json` contains every scored sample. The same directory retains the
prewarm data, binary, source snapshot, source comparison and test/build logs.

### Native S3 check before the PR

The S3 fixture and tick sources still match the measured spike snapshot.
The native comparison used separate Release binaries for the
[R0.6 source revision](bench-r06-20261122.md) and the current Editor.
The first isolated three-round series matched the workload and passed its
timing gates. It rejected peak working set (305.6 → 330.8 MiB, +8.27 %).
A diagnostic reproduced both low and high peaks with the same candidate
binary. The high peak first appeared in the last sample before process exit.

A separate five-round control with one explicit DXC path for both sides
passed `bench.py --compare`. Median S3 tick mean was 0.754 → 0.786 ms
(+4.19 %) and p95 was 0.942 → 0.960 ms (+1.89 %). Median peak working set
was 305.7 MiB on both sides (+0.01 %). The baseline still produced an
outlying memory peak. Both reports retain the variation and their individual
verdicts in `outputs/bench/2026-11-22/015851-572010-compare/` and
`outputs/bench/2026-11-22/020800-901704-compare/`.

```powershell
$env:MODULE_ENGINE_DXC = (Resolve-Path tools/dxc/dxc.exe).Path
python scripts/bench.py --no-profiling --compare 731661c47e181847450e00480e60c0645abd4753 --scenes S3 --rounds 5
```

The Windows and Linux CI checks also run the standalone spike's Release
tests against its own lockfile.

## Stage 3 — previous storage decision

## Requirements

Source modules register dynamic component IDs and JSON schemas. A saved object ID
must survive scene/prefab resolution, commands, exports and the Rust IPC protocol.
The complete world, component values, resources and events must support an owned
checkpoint for failed tick publication. Queries must select component membership
without scanning every object's JSON map. Renderer extraction remains neutral.

## Compared approaches

| Approach | Fit | Additional work |
| --- | --- | --- |
| Extend the existing `EcsWorld` component tables | Existing stable IDs, deterministic ordering and owned cloning; dynamic schema IDs can address columns directly | Component columns, membership queries, reference policy, checkpoint validation and consumer migration |
| Adopt `hecs` | Typed component queries and contiguous archetype storage, independent of our scheduler | Stable ID mapping, schema/native codecs, deterministic extraction and explicit snapshot/restore of all installed component types |

The [hecs World documentation](https://docs.rs/hecs/latest/hecs/struct.World.html)
describes typed queries and archetype storage. Its
[serialization documentation](https://docs.rs/hecs/latest/hecs/serialize/index.html)
requires application-defined procedures because arbitrary components need not
implement serialization. These are useful capabilities, but they do not remove
this project's dynamic schema or transactional requirements.

Choose the smaller extension of `EcsWorld` for this iteration. This is a contract
and migration-cost decision, not a measured claim that it outperforms hecs.
Dynamic components belong in component-indexed columns, with transform, parent,
color and references owned by that same world. A relocated entity map with an ECS
name does not satisfy this decision. Native gameplay reads/updates those columns;
entity-shaped records are temporary serialization/UI/IPC DTOs.

The Object provider remains a compatibility adapter. Its independent hierarchy
continues to serve legacy examples, but it does not gain a second production
gameplay implementation. Stable object IDs remain independent of storage handles.

## Acceptance and limits

Verify dynamic membership/intersection queries, full component rollback after
validation/commit failure, remove/reuse without inherited data, hierarchy and
references, deterministic serialization, prefab compatibility, scene extraction
and ARPG behavior. The ECS route must have one mutable owner; the compatibility
projection is validated before publication. No cache-locality, parallel-query or
large-world performance guarantee follows from choosing component columns.
