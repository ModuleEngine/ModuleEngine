# Native editor UI rollout — stage 2A

[English](editor-ui-rollout.md) | [Русский](ru/editor-ui-rollout.md) | [Documentation](README.md)

The native UI implementation requested on 15 November 2026 is delivered. The
18 tasks of [stage 2A](editor-clarity-plan.md#stage-2a) have working routes and
bounded Windows/WSLg verification. **This is not full physical DPI, foreground
input or new Player performance certification.** Those limits are explicit below.
Stages 3/5/6 retain their rendering/configuration/map semantics; G2 is not started.

## What changed and how to use it

- Start composition discovery at **Engine → Windows → Composition → Engine Overview**.
  Group/list/detail regions separate technical systems, reusable gameplay and
  game-specific content. Presence, participation, permissions and readiness stay
  separate; a cached refresh is marked. Actions open real existing targets.
- Engine/Project are protected layout presets under existing package rights.
  **+** names a copy of the current arrangement. Custom tabs can be renamed or
  deleted; reset is scoped and confirmed. Existing personal layouts are retained.
  A failed reload retains the active arrangement and blocks overwriting bad data.
- Use **Area**, or RMB on an area divider, for split/join/move. The menu identifies
  and previews the retained neighbor. A moved area is an actual OS window sharing
  the document, history and Game session. Close removes only that instance; explicit Return preserves its stable ID.
  In a compact main region, **Area → Focus area** preserves the other areas.
- Graphs use MMB/RMB drag, ordinary wheel zoom at the pointer, RMB actions,
  Shift+F10, eligible double-click, F, Alt+LMB wire deletion and mixed Delete.
  Creation/deletion is one authored Undo transaction with render-setting cleanup.
  Pin selection chooses the nearest visible resource; Alt never creates a wire.
  The permanent Add/Open/Frame/Delete row is removed. The palette names its scope;
  pin help is local and the optional type reference starts collapsed.
- Scene has opaque transform/axes/snap/frame overlays with semantic vector icons.
  Controls reserve picking, camera and drop input; an existing gesture retains
  its owner across overlays. World/local, snapping and one gesture/one Undo remain.
- Inspector groups object/prefab context, Transform, ownership and local drafts.
  Assets has folder pictograms, selection, readonly selectable address/breadcrumb
  modes, path/reference copying and compact Details. Asset transactions/IDs keep
  their existing contract. Short areas scroll or use compact tools.
- The shell owns one persistent Play/Pause/Stop/Restart/Save controller. Game
  displays its viewport/HUD/focus/status; legacy Reset remains a different action.
  Other tools use grouped/wrapping/scrolling forms. Rust/Blueprint have compact
  inspector switching, HLSL navigates actual consumers, and History keeps distinct
  checkpoint/merge operations. An absent Rust source has guidance and no no-op Apply.

Standard controls have semantic names, checked/disabled state, shortcut/reason
help on hover/focus, and consistent logical-point spacing/type/surfaces. A tab
never grants Engine rights. Team retains Project editing and a locked Engine;
Engine Overview itself retains its Full-and-matching-sources requirement.

Personal view state uses separate `workspaces.json` and the existing version-1/2
layout formats. Drafts are not layout data. No authored document/schema migration,
new game mechanic, native dependency or platform service was introduced.

## Draft and source safety

Object Inspector drafts survive selection and component Apply; schema/source
conflicts require explicit Discard. Module parameter JSON is document-owned and
keyed by stable graph/node IDs, shared across docked/detached/hidden views.
Clean text follows live data; pending text survives selection and conflicts.
Apply creates one history step; an unchanged value creates none. Save/export/Play
and guarded document transitions account for hidden drafts. Destructive module
replacement/deletion and Undo require parameter-draft resolution. Cancel preserves
text and authored data. New document adoption clears the previous draft store.

Full source inventories and their omission regressions include the new UI files,
including `node_drafts.rs`. Missing sources do not gain authoring through a layout.
The authored project, SDK, bindings, Recent and previous personal layouts remain
byte-identical to the pinned starting data; initial unrelated work is retained.
Only the canonical Full source manifest was refreshed for the changed sources.

## Repairs found during verification

Independent reviews and actual UI tests found and corrected false prefab-origin
text, help reopening after Escape, stale refresh evidence, wrong workspace reset,
legacy type migration and a deep-layout close fallback. Graph repairs include
Add/history, explicit incident-wire deletion before bypass, keyboard-menu target,
node-body focus, dense/clipped pin ownership and the small-zoom Alt conflict.
Scene fixes include compact/disabled overlays and continuing camera gestures.

The integrated review also found the inherited module-parameter draft loss above.
Actual pointer/keyboard tests exercise selection, Save/report, export/Cancel,
Apply/Undo/Redo, conflict/Discard and exact saved bytes.

The unchanged 45-second native benchmark exposed an inherited Custom reuse defect:
Core capability metadata uses literal `CARGO_PKG_NAME`, while the SDK macro audit
only admitted `CARGO_PKG_VERSION`. Both already have exact Cargo manifest/metadata
provenance checks. The audit now admits those two literals; unknown SDK environment
and computed arguments stay uncached. The negative control detects the old code;
12 freshness regressions and real coalescing/independent-state/eviction checks pass.
No fingerprint, source check or readiness criterion was bypassed. The separate
sandbox-TEMP MSVC failure is retained in the evidence; owned workspace TEMP fixes
that environmental probe, but did not itself repair the macro audit.

The pinned eframe WGPU immediate-viewport painter does not forward child screenshot
requests. The nonworking diagnostic helper was removed. An owned-window Windows
PrintWindow fixture verifies the real secondary Scene geometry/gizmo/overlays;
root application captures keep their existing diagnostic path. Normal rendering
has no new readback or screenshot queue. No Computer Use session was opened.

## Verification and delivery

Code snapshot: `source-f23.json`, digest
`633cce1213baf1826de054a79539132e9ca07a99e4b35f75cbc3cad3939748e6`;
HEAD `120910656383e2579c87de1fd2d7119ee1c91ba5`.
Final documentation/source binding is recorded in `source-final.json` and the
[independent conclusion](../outputs/reviews/editor-ui-stage2a-20261115/independent-review-final.md).
Evidence, original failures, commands and package receipts are under
`outputs/reviews/editor-ui-stage2a-20261115/`.

| Check | Actual result and scope |
| --- | --- |
| Complete Windows Editor set | **295 passed, 0 failed, 6 ignored**, 585.90 s on F23; includes all 15 canvas and 23 Scene regressions. |
| Explicit native/artifact cases | All five applicable ignored cases passed separately: genuine G1 Team palette Apply/history/Save; actual Player target modes; concurrent immutable Player variants; Scene/Game code reuse with independent state/workers after Stop; drawing/Apply Rust during slow ticks. The sixth case emits an unrelated historical stage-4 fixture and was deliberately not run. |
| Static/tooling | Strict Editor + Script Clippy across all targets passes. Diagnostic Python tests passed (11). Twelve freshness tests and the real native program-store case pass. |
| Working file/export route | `workflow-f22` passes real New/Save As/import, four actual menu routes, relocated model rendering and precompiled Player pixels; original project unchanged. This CLI fixture revision is unchanged in F23. |
| Windows geometry | Eleven inspected native captures cover 1280×720, 1600×900 and 1920×1080 with UI zoom 1/1.5/2 across Overview/Scene/Assets/Systems/Rust and HLSL/History. UI zoom is not an OS DPI change. |
| Native windows | Two actual named OS windows in one PID; WM_CLOSE returns stable area ID 1 and leaves detached empty. An actual secondary Scene OS image shows geometry and tools. Physical DPI is 96. Source-bound F21 evidence covers this unchanged native-window code. |
| Full/Team/Player | Actual G1 Full and Team exports; Full available, required `node_drafts.rs` omission rejected and exact bytes restored; Team Engine locked. Team tool test and actual Full/Team startup pass. Player exported from Team verifies with selected `audio,g1,persistence,ui,vulkan`; unrelated gameplay features are absent. |
| Linux | F23 all-target compilation, native build and actual X11/WSLg Overview + Scene captures pass with WGPU/Vulkan and llvmpipe LLVM 20.1.2. This is software Vulkan, not hardware Linux certification. |
| Data | Source/scope checks retain all seven protected hashes and the authored `game-project-v4.json` hash. No reset of user layouts/Recent/SDK/bindings or authored files. |

The verified Windows Editor is `target/x86_64-pc-windows-msvc/debug/module-engine-editor.exe`
SHA-256 `D55D8F691A6EC3652020AECD2DFEECF392C90C8A3BCF23D1799F4F3F115A7E83`.
It is byte-identical to the tested F23 executable and replaces the old desktop
launch target **after verification**, not to prepare a test. The existing desktop
shortcut/launcher is retained. Its ordinary source query reports Full, the current
checkout compile root and no diagnostics; a native startup capture was inspected.
The old `80dd1a8a…` binary was retained until this delivery and is now replaced.

Matching local verification deliveries are in
`outputs/deliveries/editor-ui-stage2a-20261115/{full,team,player}`.
Full is a source project; Team contains the ready Editor and selected Player
variant, and Player is standalone. These are acceptance deliveries, not development
clones or new worktrees. No Git commit, PR, push or publication was requested.

## Measured responsiveness and remaining limits

F23's actual native Graph/Game/Scene benchmark completes 90 stopped-pan and 90
play-pan samples, verifies simulation advancement, retained owners and a new
paused time-zero worker after Stop, with no errors: **27.1335 s total**, within the
unchanged 45-second watchdog. It uses scripted egui focus/pointer events and
records native focus separately; it is not a physical-input latency check.

| Phase | Editor callback mean / p95 | Callback interval mean / p95 |
| --- | --- | --- |
| Stopped + graph pan | 12.4247 / 15.4082 ms | 16.4193 / 19.3164 ms |
| Play + graph pan | 14.0011 / 17.4396 ms | 18.0926 / 21.5283 ms |

The two pre-UI runs completed the same 90/90 phases but failed final Stop readiness
because of uncached preparation; they remain failed full benchmarks. Their phase
callback means/p95 were 12.1664–12.5524 / 15.3288–15.5400 ms stopped and
16.0520–16.4618 / 20.1676–20.6954 ms playing. The current samples show no large UI
regression and lower playing callback time, but short samples and uncontrolled
background load do not establish a causal speedup. This three-area Editor workload
is not the unchanged G1/ARPG standalone 60 FPS criterion, a cold-start result or a
new rendering-performance certification.

Only one 3440×1440 monitor at physical DPI 96 is available. Physical 150/200%,
mixed-monitor transitions, native picker selection and external foreground
keyboard/mouse-to-display QA remain unverified. Logical scaling and pure focus/
Cancel/input contracts have separate checks and are not substituted for them.
Linux native audio/file pickers and hardware Linux retain their existing limits.
The early authored-project Scene capture records preparation, not ready geometry.
The [72-row matrix](editor-ui-coverage.json) records 68 implemented current
surfaces, three explicitly deferred V02/V10/D11 routes and P22's registration
contract. It does not claim 72 bespoke windows or every native state certified.

## Scene storage and Hierarchy follow-up

The subsequent [scene-file implementation](scene-files.md) replaces the old
Hierarchy button stack, introduces mutable scene sources and adds D13/D14.
The original stage2A matrix had 72 entries; the current catalogue has 74.
Its verification remains bound to its original snapshot, while the follow-up
records its own file/UI/package checks and independent final review.
