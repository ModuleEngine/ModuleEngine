# 0.4.1 engineering verification — rendering controls

[English](release-041-verification.md) | [Русский](ru/release-041-verification.md) | [Milestone brief](release-041-plan.md) | [Settings contract](settings.md#rendering-controls-041)

**Engineering record, 15 November 2026: scoped checks, actual Full/Team UI routes and relocated Full/Team Player evidence pass.** Source/artifact bindings and the separate independent conclusion are linked below. This records work against editor clarity tasks 3.1–3.6. It does not mark an unqualified release or human usability acceptance.

## Scope and resulting behavior

Team retains the ordinary **Tools → Project Settings** route. Full with checked matching sources retains **Windows → Composition → Engine Overview → connected renderer → Open render graph**. Saved layouts and the Full/Team/source boundary are preserved. Opening the tools does not edit the project or activate an absent module.

The new shared summary explains AA and AO, identifies the connected renderer, distinguishes absent built-in FXAA/SSAO passes from present but disabled passes, and separates the graph's **Base pass enabled** from authored Project commands. It finds exposed controls by their actual pass targets, supports arbitrary control IDs and uses authored option labels. The displayed resolution is explicitly a reference calculation: game projects use time zero; live Game time and permitted runtime render commands may differ.

Project selection uses the existing immediate authored commit. Game/Scene graphics preparation and the accepted image remain separate from that commit and from saving. Graphics changes can refresh during paused Play; execution-graph changes retain their existing Stop requirement. Feedback reports actual preparation/failure state, last-working-version retention when available, and the need to inspect the image. Save/draft/export distinctions remain explicit. Existing pass removal/bypass/settings cleanup and atomic Undo/Redo are reused; fresh placement uses defaults rather than promising recovery of custom values.

This change adds UI explanation, discovery, focused tests and a safe separate “Contrasting edges” fixture/frame diagnostic. It changes no rendering algorithm, runtime behavior, dependency or project format. It does not add Apply to immediate selection, relax Team rights, replace personal projects/layouts, or implement 0.5/G2. The binary/package version remains 0.4.0; a version bump is not completion evidence.

## Source and artifact binding

Work continued in the existing checkout at HEAD `120910656383e2579c87de1fd2d7119ee1c91ba5`. Extensive pre-existing tracked and untracked work was preserved. The initial source snapshot has digest `b41eed198b2ce1f2c49927700eb2b73041a7e087affde279e6a92e24b1206d1d`; initial status, tracked patch and source archive are retained under `work/release-041/`. Snapshot manifests cover tracked and untracked source files, rather than relying on HEAD alone.

| Binding | Evidence |
| --- | --- |
| Selected test source, code02 | [`source-code02.json`](../outputs/release-041/source-code02.json), digest `37a1ce990a0678314cb3ca2a0d51a0ef75259d87cd3cfb1399d62e318fc45518` |
| Normal build/frame source, code03 | [`source-code03.json`](../outputs/release-041/source-code03.json), digest `9b85d434adbf6088026eb19c196b47c262ff51b927885ce0a3991b960f23f7d3` |
| Reviewed UI correction source, code04 | [`source-code04.json`](../outputs/release-041/source-code04.json), digest `a270d5745529fae2b2208c0fd4da6fb7d49cb28e3a09f45ee88fe6a0e11d5084` |
| Compact-pane correction source, code05 | [`source-code05.json`](../outputs/release-041/source-code05.json), digest `d006183ea44959e39da8e64712cc874e4d68d7b5c65d356eb07aa99eca2089a0` |
| Test executable | SHA-256 `cf723843fb84af4e9a1991eed73c13e51fcfbc117b7cee5b2175cb94168dac7a`, bound in the selected-test report |
| Corrected-source test executable | SHA-256 `f58f8167353b76b032d2151fe64828ca158d99314713b37d0914914a08ca6f2f`, bound in the successful code04 test report |
| Compact-pane test executable | SHA-256 `a578d89d5c6fc3530dbd66196f1f5d6e35443c27d4b1a97be2689ea50b52e3cd`, bound in the successful code05 affected-test report |
| Frame diagnostic executable | SHA-256 `6216b6e14e8c268530f1bb76dfe387c0de60c95adadb1663f2959f3121360149`, bound in the frame report |
| Relocated on/off Player executable | SHA-256 `93d3332f70ce478279f0c9e373532934edf5d8bf029af35ab6c84ea735cb2d31`, sealed in the package manifests and [`relocations.json`](../outputs/release-041/relocations.json) |
| Final documentation/review/package binding | **PENDING**; final external [`source-final.json`](../outputs/release-041/source-final.json) will bind the completed source/docs without embedding a self-referential digest here |

After the code02 test run, code03 changes only the diagnostic pixel iteration from `chunks_exact` to `as_chunks` to satisfy Clippy. The 38-test result remains bound to code02; code03 has the successful lint/build and frame results below. Later documentation does not retroactively change those bindings.

Independent review found two UI issues. **R1:** wording implied the image changed immediately with selection; it now explicitly separates immediate authored choice from an image refreshed after successful preparation. **R2:** the summary incorrectly used the legacy Runtime's `engine.initialized` guard for Game projects, although GameHost resolves Project commands even when that flag is false. Code04 now resolves commands for Game projects regardless of the flag, retains the legacy-only fallback, and adds a CPU regression for both cases. These are UI corrections, not Player/runtime changes. The full 39-test rerun passes on code04; final review remains pending. Earlier green results are not relabelled as code04 evidence.

Actual Team GUI inspection then found the AA selector below the initial visible portion of a roughly 450×292-point pane: the new introductory prose displaced the task's control. The [before capture](../outputs/release-041/team-settings-before-reorder.png) is retained. Code05 keeps a short AA explanation first and places actual exposed controls before the longer explanation/summary. The regression now uses the product style and a real ScrollArea at 480×300, asserting the entire selector text lies inside its clip rectangle without scrolling or authored/history changes. Seventeen affected tests pass on code05; the broader 39-test result remains explicitly code04 evidence. The reviewer reports no open code issues; final snapshot-bound acceptance is still pending.

## Completed checks

The pinned Windows tools are Rust/Cargo 1.99.0 on `x86_64-pc-windows-msvc`, with the existing DXC. Native/GPU work ran sequentially. Verification plans were inspected before selecting this bounded set; it is not a full workspace/profile acceptance run.

| Check | Recorded result and scope |
| --- | --- |
| 38 exact selected Editor tests on code02 | **38 PASS**, serial invocation with `--exact`, `--include-ignored`, `--test-threads=1`; full commands and logs in [`checks-final/report.json`](../outputs/release-041/checks-final/report.json) |
| 39 exact selected tests on code04 | **39 PASS**, serial execution including the new uninitialized Game/legacy policy regression; commands/logs and source binding in [`checks-code04/report.json`](../outputs/release-041/checks-code04/report.json) |
| 17 affected exact tests on code05 | **17 PASS**, serial UI/Overview/property checks including compact clipping and real selection/history; [`checks-code05/report.json`](../outputs/release-041/checks-code05/report.json). This is affected-scope revalidation, not a second full 39-test run. |
| Selected contracts exercised | Real Project combo click/commit/Undo/Redo; custom labels/IDs and reference conditions; absent/disabled/alternative renderer; 480-point inspection without document/history edits; Full/Team/source inventory; AA/AO graph removal/restoration and cleanup; Forward/Deferred; hidden drafts and keyboard focus; rejected connection/HLSL and last-good retention |
| Formatting and strict Editor lint | Formatting and release Editor Clippy across all targets/features with warnings denied pass on code03, code04 and code05. Logs: `work/release-041/fmt-final.*`, `clippy-final.*`, `fmt04.*`, `clippy04.*`, `fmt05.*`, `clippy05.*` |
| Normal Editor/test builds | Normal release Editor and test executable rebuilt on code05; retained Editor: [`ModuleEngine.exe`](../outputs/release-041/ModuleEngine.exe). Logs: `work/release-041/editor05-build.*`, `tests05-build.*` |
| Matched native GameHost frame diagnostic | **PASS**, [`frames-final/report.json`](../outputs/release-041/frames-final/report.json); details below |
| Full source delivery | `outputs/release-041/deliveries/full` refreshed with code05 source/helper/settings and Editor binary; normal sealing succeeds (`work/release-041/seal-full05.*`). Actual GUI route recorded below. |
| Team delivery | Code03 `deliveries/team` and code04 `deliveries/team-final` are historical attempts. Actual code05 Team export is `outputs/release-041/deliveries/team-ready`, used for the saved UI/export route below. |
| Saved on/off Player export and relocation | Actual Full exports in `outputs/release-041/deliveries/player-on` and `player-off`, copied byte-identically to `relocated/player-on` and `player-off`; hashes/manifests in [`relocations.json`](../outputs/release-041/relocations.json) |
| Direct relocated Player verification | Both relocated `Player.exe --verify-package` runs pass: [on report](../outputs/release-041/player-evidence/on/player-verification.json), [off report](../outputs/release-041/player-evidence/off/player-verification.json). Native features `ui,vulkan`, sealed package hashes and exact contract locks verified; `source_compilation:false`; authored project bytes unchanged. |
| Editor/received-Player frame agreement | Both Player PNGs are byte-identical to the corresponding code03 GameHost on/off PNGs; [`pixel-match.json`](../outputs/release-041/pixel-match.json). The independent reviewer separately confirmed pixel/hash agreement with standard-library tooling. |
| Saved Team → relocated Player | Actual Team export `deliveries/team-player-off` copied to `relocated/team-player-off`; direct `--verify-package` passes with `ui,vulkan`, verified hashes/locks and unchanged authored bytes. [Team Player report](../outputs/release-041/player-evidence/team-off/player-verification.json); PNG byte-identical to Full/Editor off, recorded in the [Team UI receipt](../outputs/release-041/team-ui-receipt.json). |

The first selected-test run was **37 PASS / 1 FAIL**. The new combo test omitted normal Engine Exposure initialization, so its fixture had no Project combo to click. The fixture was corrected; the final 38-test run passes. The initial diagnostic Clippy failure was the pixel-iteration lint corrected in code03. Original logs are retained, rather than presenting these attempts as clean runs.

The frame diagnostic uses native **Vulkan / NVIDIA GeForce RTX 4080 SUPER**, **960×540**, the same fixed camera and geometry, and the same **three ticks / 0.05 s**. FXAA/off changes **3,246 pixels**; restoring FXAA reproduces the original frame exactly. Authored documents and file bytes remain unchanged. The report retains camera, geometry/state, composition/lock identities and fixture hashes. These are real CPU-readback frames through the shared Editor/Player GameHost. Separate relocated Player checks establish the received-package frame agreement; native-window evidence is recorded below, and human edge-quality assessment remains unverified. Code04/code05 change only Editor UI/resolution logic and tests; the verified Player executable/runtime is unchanged.

Inspect the [frame comparison](../outputs/release-041/comparison.html), [FXAA](../outputs/release-041/frames-final/editor-on.png), [off](../outputs/release-041/frames-final/editor-off.png) and [restored frame](../outputs/release-041/frames-final/editor-restored.png). The [owner card](../outputs/release-041/owner-card.en.md) is prepared in both languages; participant completion is unverified.

## Actual native routes and remaining presentation limit

The author operated actual Windows Editor windows using WGPU/Dx12 on the RTX 4080 SUPER. Team followed ordinary launch → Tools → Project Settings, saved initial Assets indexing, selected Off and pressed Ctrl+S, switched a Scene area to Game, used Ctrl+Z to restore FXAA and Redo to return to Off, and closed with clean saved feedback. Reopening retained the saved Game/settings layout, Off choice and clean state: [Team receipt](../outputs/release-041/team-ui-receipt.json), [settings capture](../outputs/release-041/team-settings.png), [reopened capture](../outputs/release-041/team-reopened.png). The actual JSON delta is only `/asset_manifest` and `/project/variants/antialiasing`; scene bindings remain unchanged.

Full followed ordinary launch, saved initial Assets indexing, then Engine workspace → Overview → Open render graph `Root/renderer` → focused-canvas F to fit, and closed cleanly. Its graph was unchanged; JSON differs only in `/asset_manifest`: [Full receipt](../outputs/release-041/full-ui-receipt.json), [Overview capture](../outputs/release-041/full-overview.png). Initial Assets indexing is existing intentional metadata adoption: stable file IDs require Save; it is distinct from an AA edit. These are implementer observations, not independent human usability sessions.

The received Player was launched normally and returned the “Contrasting edges” window title, but Computer Use approval timed out before a screenshot. The session was reset; only the owned test Player was terminated after checking its executable path and diagnostic state root. The combined script's exit `-1` reflects that interruption, not a demonstrated product crash. No screenshot or human visual inspection is claimed for that attempt.

An additional existing Player `--benchmark` diagnostic completed automatically with exit 0: [native report](../outputs/release-041/player-window-diagnostic/native-frame-report.json), [summary](../outputs/release-041/player-window-summary.json). It records `complete:true`, 30 warmup callbacks, 286 measured UI callbacks, 120 unique GPU-resident game frames, zero error frames, physical 1920×1080 and GPU-only transport with zero CPU readback/upload. This is functional native-window evidence without a screenshot. `target_met:false` is retained: this neutral sample has zero enemies and does not supply the required 20-enemy ARPG workload. It establishes no new FPS certification.

The supplied Team launcher now opens `deliveries/team-ready` with the saved Off choice; the received-game launcher opens matching `relocated/team-player-off`. The comparison uses actual pairs and owner-card participant boxes remain unchecked.

## Final source binding and independent conclusion

The [final source snapshot](../outputs/release-041/source-final.json) binds HEAD and tracked/untracked source bytes. The [task-only delta](../outputs/release-041/task-delta.json) separates this work from the extensive initial changes; [artifact hashes](../outputs/release-041/artifacts.json) bind the actual deliveries and evidence. The [independent conclusion](../outputs/reviews/release-041/final-review.md) records the reviewer's decision, findings and remaining limits for that snapshot. This report does not embed its own final digest.

[Preservation evidence](../outputs/release-041/preservation.json) compares all initially inventoried source/authored files, preserving unrelated bytes and existing SDK/policy identities. Every native Editor used an isolated personal-settings root. Original ignored Recent/layout files have last-write timestamps before the initial task snapshot; their final hashes are retained, but no initial content hashes were captured for them. Their evidence is therefore metadata and route isolation, not a claimed byte-for-byte before/after comparison.

## Limits and acceptance

The owner and an independent author have not completed the owner card. Finding time, active editing time, waiting time, observer help and subjective visibility of the edge change remain unmeasured. Automated tests and the implementer do not replace those participants.

Physical DPI/mixed-monitor behavior, hardware Linux, clean-machine/cold-start portability and new FPS/performance budgets are not certified here. This work does not establish ARPG 60 FPS or broader G1/G2 acceptance. A scoped engineering success must retain these limits; final source binding and independent review are required before the final engineering conclusion, and human usability remains separately unverified.
