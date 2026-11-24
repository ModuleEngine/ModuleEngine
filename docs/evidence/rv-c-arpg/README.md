# ARPG fixed-tick measurement and copy-on-write trial

[English](README.md) | [Русский](../../ru/evidence/rv-c-arpg/README.md) | [Documentation](../../README.md)

The five checkpoint copies took **36.92 µs (3.85%)** of the baseline tick.
The scoped copy-on-write variant reduced that part to **13.00 µs (1.38%)**.
The median of the seven round means went from **966.0 to 962.6 µs**. The
paired deltas ranged from −5.80% to +3.32% (median −2.06%). In the original
arm, `ArpgFrame` JSON conversion and the clone feeding deserialization took
**463.03 µs (48.31%)**. [Measurements and receipt hashes](measurements.json)

## Workload and timing

The base was `9aa0e7633b2bfbf87fbd7d89bb6c6085cc04c8c5` on Windows x64
`26300.10075`, Ryzen 9 3900X, Rust `1.99.0` x86_64-pc-windows-msvc,
Release `opt-level=2`. The single-threaded fixed tick ran at `1/60` s
through a headless `GameHost` with ECS, the native eight-node ARPG schedule,
and the Two Arenas **Small monsters** scene. The fixture raised both sides'
health to 10,000, kept the authored spawns, and replayed deterministic
movement and melee input. Twenty enemies stayed alive. Each 600-tick
scored run produced 159 `Attack`, 391 `Hit` and 550 `Sound` events. The
renderer, compiled Custom, and external services were outside the fixture.
[Two Arenas content](../../../crates/example-two-arenas/src/content.rs) · [Fixture and counters](probe.patch)

Each process started from a fresh host, warmed up 120 ticks and measured 600.
Seven total-time pairs ran sequentially, alternating the order. The section
counters were disabled for those pairs. The production fixed-tick timer
measures `GameSession` including input, schedule, validation and publication.
A separate clock spans the entire `GameHost::step` call. The stats below are
medians of each run's mean and nearest-rank p95, in microseconds.

| Scope | Eager mean / p95 | Copy-on-write mean / p95 |
| --- | ---: | ---: |
| Fixed tick | 966.0 / 1,332.4 | 962.6 / 1,320.0 |
| `GameHost::step` | 968.7 / 1,334.9 | 965.3 / 1,323.5 |

Five additional pairs with the section clocks enabled provide the breakdown.
The denominator is all fixed-tick time in those pairs, 3,000 ticks per arm.
The very small empty-event and `control` copy timings include clock overhead.
The prototype shares existing resource and `module_data` JSON values through
`Arc`. Writes replace a shared value or update a unique allocation. `control`
detaches on mutable access. The event and system-state checkpoint code,
resource refresh clones and `ArpgFrame` codecs are identical in purpose and
workload between the arms. [Prototype](cow.patch)

| Operation | Eager µs/tick | Eager share | Copy-on-write µs/tick | Copy-on-write share |
| --- | ---: | ---: | ---: | ---: |
| Checkpoint resources | 6.28 | 0.65% | 0.46 | 0.05% |
| Checkpoint events (empty on entry) | 0.06 | 0.006% | 0.07 | 0.007% |
| Checkpoint system states | 11.97 | 1.25% | 12.19 | 1.30% |
| Checkpoint `module_data` | 18.58 | 1.94% | 0.24 | 0.03% |
| Checkpoint `control` | 0.035 | 0.004% | 0.038 | 0.004% |
| `ArpgFrame` JSON value clone for decode | 142.33 | 14.85% | 146.72 | 15.62% |
| `ArpgFrame` deserialize | 120.34 | 12.56% | 123.53 | 13.15% |
| `ArpgFrame` serialize to JSON value | 200.36 | 20.90% | 202.42 | 21.55% |
| Other resource-value clones during refresh | 40.61 | 4.24% | 40.04 | 4.26% |
| Event projection to JSON during refresh | 1.47 | 0.15% | 1.57 | 0.17% |

There were ten frame reads and nine writes per tick including `begin_tick`
and final state validation. A separate counter-only pair recorded 95,532
`actor`, 69,817 `health`, 24,600 `melee`, and 12,000 `ai` decodes per 600
ticks. That is 97.96% of the six component types read through the ARPG
JSON helper. It measures access frequency. [Typed access and Bevy decision](../../decisions.md#rv-c-arpg-tick)

## Late rejection through GameHost

An additional node after ARPG audio wrote JSON state, a resource, `module_data`,
`control` and an event, then rejected in `commit_tick`, after host validation
and publication had been entered. With both executables the accepted game
state including world and lifetimes, resources, system states and control
matched the checkpoint exactly. The rejected events were empty. Retrying via
`GameHost::step(0.0, input)` consumed the pending tick and reached tick 121.
A fresh-process run with the same inputs and no rejection produced the same
state and event hashes. Fresh processes align the lifetime ID counter for that
comparison. All 26 timing and count runs also agreed on fixture, final state
and complete scored-event-trace hashes. [Rollback and run receipts](measurements.json)

## Reproduce

Build the two probes sequentially in a disposable archive of the measured
base. The two patches are experiment inputs and require only the listed
example target. Keep the eager executable before the second build.

```powershell
$base = '9aa0e7633b2bfbf87fbd7d89bb6c6085cc04c8c5'
New-Item -ItemType Directory work/rv-c4-replay
git archive --format=tar --output=work/rv-c4-main.tar $base
tar -xf work/rv-c4-main.tar -C work/rv-c4-replay
git apply --directory=work/rv-c4-replay docs/evidence/rv-c-arpg/probe.patch
Push-Location work/rv-c4-replay
cargo build --locked --release -p module-engine-host --example rv_c4_arpg
Copy-Item target/release/examples/rv_c4_arpg.exe rv-c4-eager.exe
Pop-Location
git apply --directory=work/rv-c4-replay docs/evidence/rv-c-arpg/cow.patch
Push-Location work/rv-c4-replay
cargo build --locked --release -p module-engine-host --example rv_c4_arpg
Copy-Item target/release/examples/rv_c4_arpg.exe rv-c4-cow.exe
```

Run each executable with `<mode> <new-report.json>`. Run seven `total` pairs,
starting eager first, and five `sections` pairs, starting COW first. Reverse
the arm order in each successive pair. Run both in `components` once. For
each executable use separate `rollback` and `oracle` runs. Compare their
`accepted_before_sha256`, `accepted_after_sha256`, `accepted_events_sha256`
and `ticks_after` values. `rollback` also asserts the exact in-process
checkpoint. The two arms' fixture and accepted- and event-hash fields must
agree in `total`, `sections` and `components`. All original sample arrays,
run order, build logs and executables are retained under local `outputs/rv-c/`.

This experiment covers one 20-enemy encounter and one headless Windows CPU.
It supplies no frame-rate result for the rendered ARPG, no Linux measurement
and no comparison of alternative ECS backends. The measured COW result
applies to the three selected payload classes and the existing codec path.
