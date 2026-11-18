# 0.4.1 follow-up — compact settings and the game menu

[English](release-041-menu-verification.md) | [Русский](ru/release-041-menu-verification.md) | [Brief](release-041-plan.md#player-settings) | [Settings contract](settings.md#player-menu-preferences)

Engineering record, 16 November 2026: the scoped implementation and checks below pass. The source-bound independent conclusion is recorded separately; human usability acceptance remains open. The preceding [rendering-control delivery](release-041-verification.md) is preserved separately.

## Delivered behavior

Project Settings has section navigation, label/value rows, help on request and collapsed conditions. Wide panes put sections on the left; narrow panes retain keyboard access. Idle explanatory paragraphs have been removed. Unsaved changes, preparation and actionable errors retain concise feedback; the existing immediate commit has no additional Apply step.

Custom Rust can read a permitted setting by stable ID, enumerate option IDs and labels, inspect the effective selection and request another option. The game emits its own panel and Choice descriptions; the same native adapter draws them in Player and the Editor Game viewport. The sample binds `antialiasing` through `custom_sources["settings-menu"]`. Its script contains no renderer pass ID or duplicate list of FXAA/Off commands. See the [API and complete example](custom-rust.md#script-settings-menu).

The optional `game.player_preferences.namespace` supplies a stable game identity. A valid local choice overrides raw runtime controls and Project conditions/defaults for that group. Untouched groups retain existing behavior. Only unambiguous, complete, exclusive parameter/enabled presets are bindable; removed or unsupported settings hide their controls. Ambiguous effective controls are reported as Runtime controlled.

Preferences use the existing JSON persistence service with a separate Host owner. An invalid request preserves the accepted state. A later tick failure rolls back settings, menus, both Custom instances and world state before storage. An unavailable saved option falls back to the current valid Project choice without rewriting the save. Corrupt, future or malformed payloads remain intact and read-only. A failed write leaves the accepted runtime choice active and explicitly Not saved. Unknown saved payload fields are preserved.

Accepted choices and displayed pixels have separate provenance. Applying remains visible until an actual frame with the accepted selection is published. A render error remains visible while a valid menu can request recovery. Popup input is confined to the Game viewport, including a resize of an already-open popup; the dismissal gesture is not forwarded into the world.

The new fields require exact SDK identity `module-engine-data-api/v3`. Earlier v2 projects and packages are not silently repinned or relabelled. Use matching sources/SDK and the existing explicit contract-update route. The previous delivery retains its own binaries and pins. Binary/package version 0.4.0 is unchanged.

## Artifacts and checks

The ready launchers and editable example are under [`outputs/release-041-menu`](../outputs/release-041-menu): `launch-team.cmd`, `launch-full.cmd`, `launch-player.cmd` and `launch-clean-profile.cmd`. Read the [owner card](../outputs/release-041-menu/owner-card.en.md), [short binding example](../outputs/release-041-menu/binding-example.md) and [frame comparison](../outputs/release-041-menu/comparison.html). The adjacent sample `menu.rs` is a readable copy; the embedded Custom source is authoritative.

Team Custom editing requires the pinned Rust/Cargo and host linker/SDK. The default Cargo cache on this machine lacks `sha2`; the unsuccessful default-cache export is retained in `work/release-041-menu/export-team-player.*`. Full/Team launchers select the already prepared workspace offline cache only when `CARGO_HOME` is unset. The actual Team export with that cache succeeds using its own `.sdk`, with Engine authoring disabled. Player requires no Rust source compilation. This is not clean-machine toolchain portability evidence.

| Check | Result and evidence |
| --- | --- |
| Formatting; strict workspace release Clippy, all targets/features | PASS; `fmt-code-final.*`, `clippy-code04.*` |
| Core / ProjectTools / Host libraries | 119 / 75 / 93 PASS; four Host ignored tests are separate, not silently passed |
| Three explicit real Custom IPC contracts | 3 PASS: instance isolation and local precedence; late rollback and retry; invalid/stale requests and removed binding |
| Player library with all features | 63 PASS, including eight menu interaction/geometry tests and preference-only/shared-service capability inventory |
| Final exact Editor checks | 33 PASS: settings/summary, Full/Team inventory and runtime delivery, actual GPU commit/Undo/Redo, Stop/reuse and the existing slow-tick responsiveness budget; [report](../outputs/release-041-menu/editor-final-checks/report.json) |
| Standalone Player routes | 11 successful, fully exited processes; [`player-results.json`](../outputs/release-041-menu/player-results.json) and individual reports |
| Actual Full and Team exports | Separate deliveries; Team uses its checked SDK and exports a real standalone Player. A separate author-edited menu also compiles and runs; an unauthorized exposure edit is rejected. [Results](../outputs/release-041-menu/team-edit-results.json) |
| Normal native windows | Player completes 120 fresh GPU-resident frames with zero error frames and zero CPU readback/upload; Full/Team WGPU/Dx12 captures and an actual Player menu selection are recorded below |

The main Player starts a clean profile at FXAA. A real egui pointer selection emits scoped input to the sealed Custom worker; Off is accepted and saved. A new Player process reads Off, while a different clean profile reads FXAA. Relocation retains the choice. The same 960×540 Vulkan / NVIDIA GeForce RTX 4080 SUPER scene changes 3,246 pixels between FXAA and Off. Off after restart is byte-identical to the selected Off image; clean-profile FXAA is byte-identical to the original FXAA image.

A real Windows sharing lock denies save replacement after loading a valid FXAA save: Off remains active and visibly unsaved, the complete prior save tree remains unchanged, and a new process returns to saved FXAA. Separate corrupt/future/malformed profiles also accept unsaved Off without changing their bytes. Unavailable saved choices fall back to FXAA without changing the saved payload. Whole-tree hashes confirm the authored sample and main/relocated Player packages remain unchanged. The Team Player also retains Off from the same profile and accepts a real menu change in a separate profile. A separate fixture changes only the Custom panel title to My display, exports through Team's own SDK and shows that title in the received Player; its accepted/saved Off pixels match the original Off exactly. The original Team project remains unchanged. Changing the available option definition instead is rejected by the frozen Team boundary.

Normal Editor captures use WGPU/Dx12 on the same RTX 4080 SUPER: [Team](../outputs/release-041-menu/team-settings.png), [Full](../outputs/release-041-menu/full-settings.png). They show the compact controls during actual initial scene preparation and unsaved asset indexing, not a claimed idle/fully prepared state. The [native Player report](../outputs/release-041-menu/native-player/native-frame-report.json) completes its default run with 120 fresh GPU-resident frames, 30 warm-up callbacks and zero error frames. `target_met:false` remains explicit: this neutral zero-enemy example does not certify the ARPG performance target.

Computer Use additionally observed FXAA in the ordinary delivered Player, clicked Off and observed Off with changed edges and no Applying/Not saved status. After closing and relaunching, an unrelated floating window occluded the menu; that restarted-window menu observation is not accepted as evidence. The separate process checks above establish restart selection/pixels. The [window receipt](../outputs/release-041-menu/window-ui-receipt.json) records this limit. The Computer Use kernel was reset; unrelated content was not operated and its screenshot payload was not copied into artifacts.

These runs use the actual menu adapter, package Custom IPC and native pixels. The bounded diagnostic does not imply a desktop screenshot or human interaction. Initial failing UI fixtures and confirmed popup regressions remain in the logs; only the subsequent passing checks are acceptance evidence.

## Source, review and limits

Work stayed in the existing checkout at HEAD `120910656383e2579c87de1fd2d7119ee1c91ba5`, preserving the earlier dirty state. Initial source digest is `acb178ec32212d00f9c001d444a6d9ce2d6270f01a98b8771472ff5a640cbac9`. The tested/built code snapshot is [`source-code02.json`](../outputs/release-041-menu/source-code02.json); final documentation and artifact bindings are recorded externally in the final receipt and source manifest so this report does not embed a self-referential digest. The task delta uses the exact initial archive and inventory, including unchanged tracked work files omitted from that archive.

Independent reviewer `/root/review_041_menu` has rechecked API/IPC/transactions, all new production paths, UI fixes, contracts and actual author logs. Its separate [record](../outputs/reviews/release-041-menu/review.md) identifies the final reviewed snapshot, resolved findings and conclusion. The reviewer independently recomputed all 1,264 protected initial file hashes and the actual pixel/package comparisons; the final receipt also records source and preservation checks.

The owner and an independent author have not completed the owner card. Finding/editing/waiting times, observer hints and subjective edge quality remain unmeasured. Physical DPI, hardware Linux, clean-machine/cold-start portability and ARPG 60 FPS are not certified. Existing performance budgets are not relaxed. This follow-up does not implement 0.5/G2 or constitute unqualified release acceptance.
