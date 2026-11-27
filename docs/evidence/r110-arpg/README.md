# R1.10 typed ARPG tick

[English](README.md) | [Русский](../../ru/evidence/r110-arpg/README.md) | [Documentation](../../README.md)

The native ARPG schedule retains a typed `ArpgFrame` in a tick-scoped Bevy
resource. Its eight nodes share one decode and one final encode. Portable
nodes between them receive current JSON and cause a flush and a later reload.
The [schedule contract](../../module-execution.md) defines those boundaries.

## Paired Windows measurement

The baseline is `a38f6426480c97f20c9d9fab1e62b7800841d427`, with the
[benchmark example](../../../crates/game-host/examples/r110_arpg.rs) added.
Both arms use Rust 1.99.0, `x86_64-pc-windows-msvc`, Release `opt-level=2`,
default flags, Windows 10.0.26300 and AMD64 Family 23 Model 113 Stepping 0.
The fixture repeats the [RV-C encounter](../rv-c-arpg/README.md), with
120 warmup ticks and 600 measured ticks, 20 living enemies, the unchanged
eight-node native schedule, and deterministic movement and melee input.
Each process builds a fresh headless `GameHost`. Rendering, compiled Custom
and native services are outside this fixture.

Seven pairs ran sequentially with alternating arm order and no competing
build. The table gives the median of the seven run means and of the seven
nearest-rank per-run p95 values, in microseconds. Every paired fixed-tick
mean improved, with changes from −34.74% to −48.89%.

| Scope | Baseline mean / p95 | Typed mean / p95 |
| --- | ---: | ---: |
| Fixed tick | 980.00 / 1,285.90 | 559.55 / 716.30 |
| `GameHost::step` mean | 982.55 | 562.33 |

The median run-mean fixed tick fell 42.90%. Both arms produced exactly
159 `Attack`, 391 `Hit` and 550 `Sound` events. All 14 runs have identical
fixture, complete event-trace and accepted-state hashes. The accepted-state
hash includes the world, runtime incarnation IDs, portable module data,
control and per-instance JSON state. [Raw samples, hashes and environment](measurements.json)

## Rejection and replay

The benchmark adds a node after ARPG audio that writes a resource, instance
state, `module_data`, an event and control, then rejects in `commit_tick`.
The accepted state and incarnation IDs remain identical to the pre-tick
checkpoint, and the rejected event list is empty. Retrying with zero new
wall time reaches tick 121. The resource counter is checked against restored
instance state when the retry enters the node. Both binaries' retried state
and event hashes match their fresh-process oracle and each other.

Core regression tests separately check one decode/encode across contiguous
native nodes, independent executors, portable interleaving, invalid portable
edits both before native reentry and at final validation, and late publication
rejection followed by replay. Existing ARPG package regressions cover partial
schedules and source controllers before movement. The local Release suites
passed 140 Core tests and 44 ARPG tests, with one existing Core test ignored.
Strict Clippy passed on all targets and features of those two packages.

## Reproduction and limits

Build each arm sequentially with the same example and command, preserving
the first executable before building the second.

```powershell
cargo build --offline --locked --release -p module-engine-host --example r110_arpg
target/release/examples/r110_arpg.exe total outputs/run.json
target/release/examples/r110_arpg.exe rollback outputs/rollback.json
target/release/examples/r110_arpg.exe oracle outputs/oracle.json
```

Each command requires a new report path. Run seven `total` pairs, alternating
their order. Compare fixture, event and accepted hashes across both arms.
Compare `accepted_before_sha256`, `accepted_after_sha256`,
`accepted_events_sha256` and `ticks_after` between rollback and oracle.
The example derives from the RV-C fixture and retains its `probe.rv_c4`
diagnostic identifiers for a direct workload comparison.

The original build logs, executable hashes, binaries and reports are retained
locally in `outputs/r110-arpg/`. Baseline executable SHA-256 is
`8c0cf387c38aac8aa09a5a10cc17d397f35de7adf3917c34296a30ba2159fc62`.
This evidence covers one headless Windows CPU encounter. Linux timing,
rendered frame rate, alternative ECS backends and a broader component-storage
migration require separate evidence.
