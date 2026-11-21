# Engine Overview

[English](engine-overview.md) | [Русский](ru/engine-overview.md) | [Documentation](README.md)

Engine Overview implements tasks 2.1–2.5 of the
[editor clarity plan](editor-clarity-plan.md#stage-2). It is the entry point for
understanding **one configurable ModuleEngine and the current game project**.
It reads the [source capability inventory](engine-capabilities.md); it does not
introduce another executor, an Engine catalogue or a new authored document format.

## Opening the overview

With Full Engine authoring available, choose **Windows → Composition → Engine Overview** or
select **Engine Overview** in an area's type selector. A fresh Engine layout
starts with this pane. Existing saved personal layouts keep their areas, types
and graph navigation. The Project default remains Scene/Hierarchy/Inspector/Assets.
The window command reuses an existing matching area or adds one beside the active
area under the [existing navigation rules](editor-workflow.md).

Overview retains the existing Engine boundary: Full and checked matching sources
are required. Team does not acquire Engine access or an editable gameplay schedule
through semantic gameplay ownership. This stage does not change source checks,
Project exposure, native features, package bindings or SDK/lock rules.

## Reading identity and state

The header shows the current game name and project path, pending author edits,
the retained profile's ID/version as the **configuration basis**, the renderer
connected to the unique root viewport and the installed package/editing rights.
The working configuration is the current project's Engine graph and exposure;
local Full edits can differ from the published basis. An absent or invalid profile
is reported as unknown rather than inferred from a crate or game name. No unique
viewport/connection means no unique connected renderer, not an assumed default.

The pinned **Technical / Gameplay / Game project** selectors keep the ownership
groups visible without scrolling through render passes; **Unknown owner** appears
when needed. Their counts include installed/unselected capabilities, not only
participating ones. Search uses label, ID and purpose within the selected group.
The participating renderer appears first in its group. Ownership is source-owned:

- Technical engine subsystems.
- Reusable gameplay packages.
- Game-specific rules and tools.
- Ownership not described, when metadata is absent.

An installed optional package can be listed without participating in this game.
This includes concrete rules/tools from other installed example games: project
ownership identifies game-specific code, while participation identifies its use here.
The initially selected capability is the connected renderer where its description
exists. The detail shows purpose, implementation package, kind, declared contract
version, dependencies and component contracts. Host activation alternatives remain
“any of”; static component consumption is not dynamic per-instance Custom data.

Installed, available and participating are separate evidence-backed states.
Unknown is preserved. Participation describes authored configuration or declared
usage/request relationships, **not execution, compilation or readiness**. For
example, an installed audio implementation does not certify an output device,
and a participating helper library is not an executable graph node.

## Navigation and limits

The detail offers only existing tool categories from source metadata, resolving
them against the current project. Graph actions name the exact root/nested scope
and node; renderer actions open its actual internal graph. Equal local pass IDs
in alternative renderers remain separate targets. A missing pass can lead to the
active Vulkan graph for placement; missing renderer/source/schedule evidence is
explained rather than creating a replacement.

Rust/HLSL actions select the assigned source ID. Gameplay Systems, Project Settings,
Inspector and registered module tools use their existing availability and editing
rules. Inspector navigation explicitly requires selecting a scene object; it does
not guess a component instance. Native-source-only capabilities explain the need
to edit the implementation package and rebuild; no fake live toggle is offered.
Disabled actions explain missing targets or permissions. Applying/discarding a
Project File draft remains necessary before navigating from the overview.

Navigation is deferred until the dock's pane-memory swaps finish. It reuses/opens
the target area, then updates that area's own scope, selection or source. It does
not replace the Overview pane or alter authored data/history. A vanished target
is rechecked before navigation and leaves the document and layout intact.

Inventory construction is lazy and cached, with no renderer/system/service/tool
factory or callback execution. Author changes/history invalidate the cache; a
150 ms quiet period avoids repeatedly inspecting the entire configuration during
a continuous edit, and the pane marks pending refresh. **Refresh** updates it
immediately. Project adoption clears the previous inventory and queued navigation.
The game/path, rights and action validation use current data; the configuration
basis is refreshed with the inventory. No per-frame project JSON
serialization or capability inventory rebuild is added. Group, search and selection are
transient per-area UI state, not new project fields.

Stage 2 adds discovery and navigation. The following audio workflow extends that
entry point for the first 0.5 scenario.

<a id="audio-composition"></a>

## ARPG audio composition

In Full, choose **Gameplay → ARPG sound events** in Overview, or **Technical →
WAV playback service**. The details link authored cue bindings, the scheduled consumer,
the `audio.wav` request and the selected `audio` Player build feature. **Show
game.audio** selects the exact node in Gameplay Systems. The Player feature
line reports the build plan for the current working document.

**Remove audio system…** lists the events and WAV paths that would lose playback
and blocks confirmation while the request is active. **Disable WAV playback
request** leaves the cue bindings in the authored project and drops the request.
Open **Remove audio system…** again and confirm to remove `game.audio` from the
schedule and remove links that order other systems after it. The removal dialog
can be cancelled without changing the document. These two edits each create an
Undo step. Undo twice in the same editor session to restore the original
schedule, parameters and playback. Save stores the current state for reopening.

In Full and Team, **Edit → Project Settings → Audio** shows existing ARPG cue
bindings while the playback request is active. Import a WAV through Assets to
make it available as a replacement for an existing binding. In Team the checked
package continues to control the schedule. The Audio section disappears when
the ARPG request is inactive. Other project audio implementations use their own
contracts and tools.

<a id="stage2-verification"></a>

## Stage-2 verification — 13 November 2026

Tasks 2.1–2.5 are implemented in the existing checkout at base HEAD
`120910656383e2579c87de1fd2d7119ee1c91ba5`. The recovered regressions use source04
`f4441deb7d482934d0f95ea8e5e9d766c8fc78d2d315fd2957f3b7a943366afc`.
Current code is source05 `41e4f457c5e498ac037cdf669c2f716ee359f68f37f9f99c45996a242a504218`:
its only subsequent code change clarifies the game-specific group caption; the
seven Overview tests, formatting, Clippy, normal build, native capture and Linux
compile were repeated. Final documentation rebinding and the independent conclusion are retained in
`outputs/reviews/editor-overview-stage2-20261113/`. The independent report governs
the exact final source snapshot. Existing stage-1/prior work and authored data are
preserved. No commit, publication or later stage is part of this delivery.

| Check | Actual result and scope |
| --- | --- |
| Current Windows Overview tests | 7 passed, including actual egui clicks, a real multi-pane `Editor.draw` graph/Rust navigation route, exact nested scope/source targets, repeat reuse, fresh/saved layouts, cache/history/project refresh, unknown evidence and Full/Team/missing-source/draft guards. |
| Rust navigation consistency | Changing source clears the previous code-node selection/pending state; repeating navigation to the same source preserves its selection. Authored data stays unchanged. |
| Broad ordinary Editor run | Initially 243 passed, 20 failed, 6 ignored. Old checkout-root fixtures hit the stale Full inventory; the frozen-template assertion followed its locked inheritance branch. This remains a failed run, not a clean full-suite result. |
| Recovered scenarios | All 20 original failures passed together on source04 from a normally checked isolated Full fixture as working directory, using existing tools. Three stale FXAA/SSAO test captions were updated to the existing friendly descriptors; add/wire/render/history assertions remain. An intermediate missing-caption panic caused a texture-output destructor panic during unwinding; it was not a demonstrated GPU-driver fault. Original failures/logs are retained. |
| Static checks and normal build | Formatting and strict Windows workspace/all-target/all-feature Clippy pass; normal `module-engine-editor` build passes. Current EXE SHA-256: `80dd1a8a58ed26be35130c8f6796ece0ad5a5800fe93f065459b79b48ccbe40e`. |
| Native Windows UI | `native04/native-ui.png` captures the actual current EXE at 1580×940, checked Full sources, pinned ownership groups, selected renderer and real graph actions. Actual GUI is WGPU/DX12 on Windows; it exits naturally. Fixture JSON/source hashes are unchanged. No Computer Use session was opened. |
| Diagnostic wrapper | 11 Python tests pass after adding the `overview` capture view; existing views remain supported. |
| Linux compile | Existing Ubuntu/WSL Rust/Cargo 1.99.0 `cargo check --offline --locked --target x86_64-unknown-linux-gnu -p module-engine-editor --all-targets` passes. No tools were installed. This is compilation, not Linux UI/GPU/hardware acceptance. |
| Documentation/preservation | English/Russian contracts, plan, workflow, architecture, indexes and status are aligned; local links/anchors and the complete tracked/untracked source snapshot are checked. The final report records exact results and unchanged code inputs for the documentation-only closure. |

This is stage-2 discovery/navigation acceptance. The six ignored native tests were
not credited; the broad run plus targeted recovery are separate runs and sources.
There is no new complete workspace/package, clean-machine, FPS/cold-start or
hardware-Linux certification. Tests use owned verification packages; the old
working checkout's package manifest, personal layouts and authored JSON are not
resealed or replaced to make checks pass. Normal render execution, exported game
performance and future render controls retain their previous contracts.
