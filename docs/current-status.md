<a id="текущее-состояние-moduleengine"></a>

# Current ModuleEngine Status

[English](current-status.md) | [Русский](ru/current-status.md) | [Documentation](README.md)

**22 November 2026 — R2.3 keeps the accepted ECS scene in Vulkan buffers.**
On eligible devices stock geometry shares a GPU mesh pool and updates the
instance and material rows from GameHost's retained scene. The internal
vertex shader applies transforms. Custom scene shaders retain their
published vertex interface. See the [device and lifetime contract](hardware-tiers.md#r23-scene)
and [S3 measurements and boundaries](bench.md#r23-scene-checks).

**22 November 2026 — R1.6 streams changes in the accepted ECS scene to Vulkan.**
Game, Scene and the Player use the retained CPU scene through GameHost.
Bevy change tracking supplies candidates, and Host sends drawable edits after
hierarchy resolution, interpolation and material projection. A receiver
with a missing base revision gets a fresh snapshot. See the
[scene update contract and S3 check](shared-game-host.md#r16-scene-updates).

**22 November 2026 — R2.2 adds indexed Vulkan materials.** Forward and G-buffer
sample separate sRGB images with mipmaps and anisotropy on qualifying hardware.
S4 loads its original base-color images on that path. Older `scene_materials`
passes retain the atlas ABI, and new Built-in → Custom conversions carry
both variants. See the [device contract](hardware-tiers.md#r22-materials) and
[1,000-texture and S4 checks](bench.md#r22-material-checks).

**22 November 2026 — R1.5 completed. Saved game nodes run in a Bevy schedule at 60 Hz.**
Core and ARPG use the same single-threaded schedule in saved stage and node
order. Two Windows runs match the pre-change event log for all ten Crate Garden
levels and the Small monsters encounter from the checked-in ARPG project.
The Crate Garden run uses a composition without ARPG, and the `GameHost`
late-rejection test confirms whole-tick rollback and retry.
See [R1.5 verification](module-execution.md#r15-verification).

**22 November 2026 — R2.1 adds a Vulkan device layer.** Tier A adapters
with timeline support get two frame slots, gpu-allocator, a staging ring
and the dedicated transfer route when available. Existing compatible devices
can use the single-slot fence path. The S3 prototype measured a lower
window-observed frame interval and higher peak working set with two slots.
See the [measured decision](decisions.md#r21-device-prototype) and the
[device contract and native S1–S3 check](hardware-tiers.md#r21-device).

**22 November 2026 — R1.4 stages fixed-tick writes without cloning `GameState`.**
Systems see earlier writes in schedule order. Late rejection discards the ECS
journal and retains the accepted state for retry. On the 2,000-entity S3 CPU
fixture, the empty fixed tick averaged 17.25 ms before and 3.88 µs after, while
the surrounding `GameHost::step` averaged 22.16 ms and 2.63 ms respectively.
That measurement includes the pre-R1.6 full-snapshot extraction.
See [R1.4 verification](ecs-world.md#r14-verification).

**22 November 2026 — R1.3 connects component codecs to Inspector, save,
validation and gameplay Rust IPC.** Core's registry provides the reflected
native Transform codec and the source-owned schema contracts. The Windows
round-trip covers the 12 legacy example graphs, all ten Crate Garden levels
and both Two Arenas scenes, including the migrated checked-in ARPG project.
The completed G2 campaign is pending. See [R1.3 verification](ecs-world.md#r13-verification).

**22 November 2026 — R1.2 integrated stable IDs into the gameplay ECS.** The
Windows integration case preserved the object ID `9007199254740993`, its
prefab instance and both links through scene files, repeated saves, Player
export and compiled Rust IPC. The checked-in game passed the byte comparison
on a migrated copy. See [R1.2 verification](ecs-world.md#r12-verification).

The R1.2 memory fix advances Bevy's removal message buffers after a batch
despawns entities in `EcsWorld`. The Core regression covers successive
`WorldCommand::Remove` and `clear()` operations in long-lived worlds.

**22 November 2026 — R1.1 selected hybrid Bevy storage for R1.2.** In
matched Release runs on the CPU, the S3 tick averaged 0.5363 ms in Core,
0.1168 ms in Bevy with dynamic JSON, and 0.0154 ms in hybrid Bevy. The
hybrid achieves 34.82× speedup and passed the spike's stable-ID, JSON,
deterministic save and rejected-candidate cases.
See the [storage decision and reproduction](ecs-storage-decision.md#r11).

**22 November 2026 — R0.6 baseline recorded.** Three scored native S1–S5
passes on the RTX 4080 SUPER after R0.5 passed repeatability (maximum
declared spread 2.39 %). S1/S2 passed their frame and memory budgets.
The Core ECS tick in S3 averaged 0.756 ms for the upcoming R1.1 spike.
See the [baseline report](bench-r06-20261122.md) for per-run mean, p95 and
memory, the comparison with R0.2, and the raw evidence location.

**21 November 2026 — engine rebuild track started; 0.5 paused.** The owner made the
[engine rebuild track](product-roadmap.md#rebuild) the current priority and paused the
0.5 brief and new UI work until its phases R1, R3 and R4 are accepted.
R0.2 is done: the [native benchmarks S1–S5](bench.md) run from the source checkout,
and the [accepted series](bench-r02-20261121.md) of three runs differs by at most 4.75 %.
R0.3 adds [frame timings](bench.md#frame-timings) in Game and benchmark JSON.
R0.4 records [hardware tiers and Windows device evidence](hardware-tiers.md).
The GTX 1660 SUPER is a candidate for minimum Windows tier A hardware.
The RTX 4080 SUPER configuration is the only one verified in R0.4.
R0.5 adds [capability skips in verification](verification.md#vulkan-dxc-capabilities),
[debug Vulkan diagnostics](bench.md#vulkan-validation), and typed renderer and
shader errors. The Windows debug S1 run reached Playing at 1920×1080 with zero
validation errors, including producer retirement.
See the
[dated decision](decisions.md#engine-rebuild-20261121).

**17 November 2026 — 0.4.2 accepted and closed by the owner.** The owner checked
the delivery and asked to close the milestone, having previously personally
confirmed that the jitter had disappeared. This completes manual product
acceptance. The next planned milestone is [0.5: composition and dependencies](product-roadmap.md#compose);
implementation has not started and will be separate work after its brief is defined.

Accepted final03 delivery: [Full](../outputs/ui-density-042/final03/launch-full.cmd) and [Team](../outputs/ui-density-042/final03/launch-team.cmd).
The [dated decision and SHA-256](release-042-ui-density-verification.md#owner-acceptance-20261117) separate owner acceptance
from engineering evidence. No detailed per-scenario owner transcript was recorded.
Mixed-DPI/Linux and final automated captures are not declared completed; earlier
report limitations retain their actual status.

The [final03 compact native chrome](release-042-ui-density-verification.md)
uses shared local 1-point padding and 2-point gaps. Native two-Scene telemetry
at 1580×940/100% confirms 762×715→787×803 pixels per image. Final03 adds a current
enabled-state divider guard without changing geometry. Final automated matched
capture and automated detached/narrow/zoom visual checks were not obtained because
Computer Use repeatedly reported Escape stop even after explicit resume. This is
a pre-acceptance automation limitation, not an unclosed product milestone.

The [native resize feedback correction](release-042-resize-jitter-verification.md)
supersedes the earlier no-reproduction result: an owner capture shows 803/805px
jitter, and a separate actual Editor probe reproduces continued alternation after
one controlled stale sample. Native geometry readback now persists without being
sent back as a resize/move command. Creation, explicit moves, recreation and
monitor/scale placement still request geometry. The owner later confirmed that
jitter had disappeared; physical DPI is not certified. Previous icon, Area, SSAO
and 100% Scene behavior are preserved.


The [versioned product roadmap](product-roadmap.md#map) is the planning baseline.
The closed [0.4.2: image clarity and editor docking](release-042-verification.md)
implements the following: ordinary Player, Scene and a single Game pane request
100% physical resolution; editor image edges align with the pixel grid.
Project Settings is under Edit; free pane headers support native tear-off/docking.
The original report separates CPU/native evidence from physical held-gesture,
mixed-monitor DPI and owner image acceptance that had not been recorded then.
The later general manual acceptance is recorded above; it does not substitute
for missing detailed scenarios or measurements. FXAA remains unchanged; the
reported motion trail is not established as fixed. A downscale control is deferred.
The [SSAO follow-up](release-042-ssao-verification.md) identifies the reported
static fringe and stepped plane shading as false self-occlusion from raw camera
depth comparisons. The corrected built-in uses the receiving tangent plane and
world-distance rejection; FXAA is unchanged. This static fix is separate from
the still-unconfirmed temporal-trail report.
The [native pane follow-up](release-042-native-panes-verification.md) unifies docked
and detached client headers, retains the OS title/name/×, and changes secondary
Close to remove just that instance. That report records the earlier singleton
window snapshot; the [area-host follow-up](release-042-area-hosts-verification.md)
now gives floating hosts their own split trees and the common Area menu.
Area Close removes one leaf; OS × closes all current leaves of that host.
Split and editor choice preserve host membership; docking is explicit drag,
and the Return menu command is removed. The final-main fallback explanation
is hover help, including disabled Close. Legacy singleton layouts remain readable;
new trees are not a supported downgrade format. Closed leaves stay absent after
save/reopen.
The report distinguishes native header/close evidence from the unreproduced
held-resize jitter and the still-required physical input check.

The [Editor window icon follow-up](release-042-window-icon-verification.md)
reuses the original installed ModuleEngine ICO unchanged in root/floating windows and sets both Windows icon
sizes on their exact owned HWNDs. Cached-data and offscreen native slot checks
pass; taskbar appearance has not been visually rechecked while owner input is
active. The new candidate is delivered separately; the open previous build,
EXE resources and Player branding are preserved.

[0.4.1: anti-aliasing from Editor to Player](release-041-plan.md) corresponds
to editor clarity stage 3 (3.1–3.6).
The [0.4.1 engineering report](release-041-verification.md) now records the
implemented rendering explanations/controls, matched FXAA/off frames, native
Full/Team workflows and actual relocated Players, including export from Team.
The [owner checks](product-roadmap-checks.md) define how to exercise each delivery.
The owner now reports that 0.4.1 settings work and are clear, following the earlier
quick discovery of Project Settings and comment on excessive permanent explanatory
text. This is an owner observation; full independent human acceptance
remains open, not an unqualified release acceptance or new game-performance
certification. The [compact settings and player-menu follow-up](release-041-menu-verification.md)
now provides sections, help on request, concise statuses, a stable-ID Custom menu
and per-game local preferences. Actual separate Player processes retain Off after
exit, while a separate clean profile starts at FXAA; package and authored files
remain unchanged. The report records the scoped checks and remaining human acceptance.

The [separate-scene / Hierarchy follow-up](scene-files.md) now provides versioned
mutable scene sources, stable bindings and combined file history, with a compact
searchable object tree and explicit names/F2. Old inline projects require explicit
conversion. Windows Core (115), ProjectTools (75) and Editor (303) checks, actual Full/Team/Player
exports and native UI pass in the documented scope; Linux ProjectTools (74) and Editor
compilation pass. Physical DPI, hardware Linux, crash-atomic multi-file storage
and new FPS certification are not established by this follow-up. Its final
independent review and source/artifact bindings are separate from stage2A history.

The [Scene controls](scene-assets.md) follow-up separates orientation, snapping
and framing: World/Local is one switching globe/cube icon, alongside a per-area
Speed field, and snap increments have their own settings. Held RMB + WASD,
Q down/E up, mouse look and RMB-wheel speed adjustment navigate the personal Scene
camera, without changing the authored scene or game camera. Its input/layout
checks and native captures are scoped follow-up evidence, not a new storage,
hardware-DPI or game-performance certification.


On 13 November the user accepted the [engine composition and editor direction](engine-composition-direction.md):
clarify technical engine/gameplay ownership first, make the Engine overview the
main entry point, retain an additional architecture map and focused subsystem
graphs, and make Engine/Project identity explicit. The user then narrowed the
scope to **one configurable ModuleEngine with multiple game projects**.
Independent Engine management, switching and shared-version update workflows
are deferred, not prerequisites for UI work. The agreement itself did not
establish UI/runtime acceptance; subsequent implementation is recorded below.
Historical evidence retains its original scope.

Stage 1 (tasks 1.1–1.5) of the [editor clarity plan](editor-clarity-plan.md) now
provides [source-owned descriptions and a read-only capability inventory](engine-capabilities.md).
Engine/gameplay/project ownership, actual registry participation, optional
services/libraries and unknown evidence are separate. Windows scoped CPU/Editor
and smaller-feature checks, strict Clippy, two actual 44-record fixture reports
and WSL CPU metadata checks pass; [stage-1 evidence and limits](engine-capabilities.md#stage1-verification)
record the bounded verification and independent review. Stage 2's
[Engine Overview](engine-overview.md#stage2-verification) is implemented with
bounded Windows UI/regression checks, Linux compilation and source-bound
independent review. It identifies the current project/configuration/rights,
separates source-owned groups and opens real tools without altering authored data.
[Stage 2A — native editor UI rollout](editor-clarity-plan.md#stage-2a) is now
implemented and delivered. The [rollout record](editor-ui-rollout.md) documents
named workspaces/real OS areas, graph/Scene input, hierarchy, shared commands and
hidden-draft safety. Windows passes 295 ordinary Editor tests plus five explicit
native/artifact cases, matching Full/Team/Player delivery and a valid native
Graph/Game/Scene benchmark. F23 Linux compilation/build and actual WSLg Overview/
Scene pass on software Vulkan. The existing desktop launch target now contains
the verified Editor. Physical 150/200%/mixed-DPI, native picker selection, external
foreground input and hardware Linux remain unverified; this is not new G1/ARPG
60 FPS or cold-start certification. The 72-row matrix retains V02/V10/D11 and P22's
explicit boundaries. Stage 3 has its separate report above; stages 4–6 are not started; overlapping stage-4 work reuses
this evidence when that stage is reconciled.

On 14 November the user's queued UI requests were consolidated into an
[editor design with primary-source research](editor-ui-design.md), a
[technical geometry/icon/input baseline](editor-ui-spec.md) and
[design/review skills](editor-ui-skills.md). The design includes named layouts,
area-border actions/native windows, graph gestures, readable Assets, Scene
overlays and standard icon controls with hover/focus help. These are documented
requirements/proposals, not implemented UI changes or a new stage-3–6 acceptance.
The follow-up [complete UI catalogue](editor-ui-catalog.md) maps 72 design
surfaces to current source registrations/purposes and explicit proposed routes,
with per-surface requirements, eight state categories, primary mockups and state
sheets. It covers all 19 builtin area types, both current module panels, all
seven app-modal purposes and seven native picker purposes plus OS overwrite.
Previously open interaction details now have concrete proposed choices for
review; this remains documentation-only, not implementation acceptance.

The subsequent [focused hierarchy refinement](editor-ui-hierarchy.md) revises
the Overview, Inspector and Assets mockups, with clearer selection, grouped
fields/facts, contextual primary actions and three 480-point compact-area
variants. The catalogue stays at 72 surfaces; other layouts retain their
earlier composition. This is a design revision, not native editor integration
or proof of usability/DPI/performance acceptance.

A [catalogue-wide command audit](editor-ui-spec.md#command-ownership) removes
duplicate local Game-session controls and Project File Save in the mockups.
Legacy preview Start/Pause is also removed because it shares the shell flag;
its Reset/configuration remains scoped; component specimens omit
the shell controller and are marked as samples. Distinct Apply/draft/file
transactions and menu/shortcut routes remain. The root Blueprint mockup also
reclaims its permanent command row for the canvas, with RMB creation, eligible
node double-click and focused-canvas F/Delete defined as target bindings.
Those mockup revisions left native code unchanged; subsequent native integration
is tracked in the stage-2A rollout record.

G1 now has completed Windows Editor/Player routes and the agreed 1080p Playing
budget: mean UI-distinct interval 8.4723 ms, p95 14.0901 ms, peak working set
338,776,064 bytes. The [1 November continuation](g1-verification-20261101.md)
records the actual Source16 artifacts, rendering, input, recovery and audio
repairs; the [independent final review](../outputs/reviews/g1-game-20261101/final-review.md)
governs Goal2 closure. Goal1's reusable [foundation](g1-foundation-20261027.md)
retains its historical independent acceptance.

The common audio module's 75% feed slowdown was reproduced and corrected with a
40 ms bounded queue; native source-clock and owner-lifecycle checks pass, and
the user confirmed normal sound. Actual Editor authoring, Apply/Undo/Redo,
Save/reopen and authored/Play separation pass. Current Player warning/recovery
and the demanding movement/Undo/onscreen UI/sound route pass. Normal presentation
uses GPU-only transport with no measured render/service errors. This is fixed-PC
Windows evidence, not clean-machine, hardware-Linux, phone or physical scanout
certification. The [engine assessment](g1-engine-assessment.md) retains the
composition and portability limits; no G2 implementation starts here.

Earlier October G1 reports and broad test counts retain their original sources;
they are not a new full workspace run. The EN-01/P0 material below retains its
own historical source, artifact and acceptance bindings.

Summary as of 22 October 2026: EN-01 is implemented and verified within the [reported scope](en01-english-ui-20261022.md), before portfolio-game development. Engine-owned UI and diagnostics use English; authored content, opaque external OS/compiler diagnostics and OS-owned native dialog controls retain their original text or language. Documentation remains available in full English and Russian versions. See the [UI language contract](editor-workflow.md#ui-language).

The current normal Windows Editor `cf14c08d…`, scoped source05 Editor/Player checks and strict Clippy pass; the five CLI routes retain their separate source04 binding. Current Linux Editor compilation and five CPU Activity tests pass without new Linux GUI, timing or P0 certification. The source05 Scene capture records preparation, not ready geometry. These check results do not themselves constitute the independent final decision; the report keeps that decision separate from verification evidence.

The [P0 finalization](p0-finalization-20261018.md) records complete Windows and applicable Linux checks plus scoped real foreground observations. The [external final review](../outputs/reviews/p0-finalization-20261018/final-review.md) governs that source-bound acceptance. [17 October P0](p0-stabilization-20261017.md), [pre-async continuation](p0-continuation-20261018.md) and [async evidence](gpu-async-20261018.md) retain their own artifacts and limits; this page does not turn its date or EN-01 changes into a new P0 EXE acceptance.

<a id="цели-и-границы"></a>

## Goals and Boundaries

The PR preparation also closes a Full-source completeness gap: `crates/render-vulkan/src/retirement.rs` is now mandatory in the source inventory. Omitting it rejects Engine authoring; the source-omission regression covers this additional packaging guard separately from the historical async runtime measurements.

Workflow and personal-layout tests now use [isolated, normally validated source installations](verification.md#isolated-source-fixtures), so a stale checkout manifest cannot stop their CI scenarios before the intended assertions. This changes test setup; runtime acceptance remains bound to the reports below.

Shared modules and typed contracts must work independently of genre and specific examples. Authored data is separate from Play; rejecting a change preserves the working version and the user's work. Engine owns the architecture; Project changes what is permitted. Player is a separate native distribution with explicitly selected features, without Editor, Rust/Cargo/DXC on the player's machine.

The accepted long-term goal is a [portfolio of 5–10 distinct finished games](portfolio-roadmap.md)
covering 2D/2.5D/spatial 3D, pixel/polygonal/realistic/mobile-style graphics and
different mechanics. Mobile-style currently means a PC game; mobile ports are
future work, with specific platform targets still open. The roadmap proposes coverage, capability phases and acceptance
gates; G1 now has an approved brief, other game designs remain proposed, and no portfolio entry is
accepted by writing the plan. Two Arenas and Puzzle remain technical baselines
until they meet those gates. This adds a product direction, not new runtime features.

P0 now requires an audit and stabilization of the existing version before P1:
fix all confirmed current-scope defects and measured performance problems, repeat
required checks, and pass the unchanged ARPG and cold Custom preparation criteria.
Future missing subsystems remain separate work. This is an accepted plan change;
recording it does not itself close the evidence gates.

[Decisions](decisions.md) retain the accepted scope and assumptions; [open questions](questions.md) list future options. Historical statements such as “remains the next stage” in older reports do not reopen work already completed. Skeletal animation, full PBR, navmesh, networking, and other large subsystems require a separate task and criteria.

<a id="реализовано"></a>

## Implemented

| Area | Current contract | Evidence and description |
| --- | --- | --- |
| Shared Host and ECS | Shared GameHost outside Editor; Core is neutral to ARPG, and the mutable world belongs to ECS. | [Completed stages 2–8](modular-runtime-progress.md), [Host](shared-game-host.md), [ECS](ecs-world.md). |
| Game packages and Player | ARPG is optional; Two Arenas and the independent Puzzle are separate projects. Player has a separate executable and explicitly selected native features. | [Package boundaries](optional-gameplay.md), [Player crate](../crates/player/README.md), stages 5–7 in the [log](modular-runtime-progress.md). |
| Custom after Stop | With an unchanged supported compile context, ready code is reused; the freshness proof may invoke Cargo without a new root compilation. Instance state and worker PIDs are independent. | [Compiled-program store](custom-program-store.md), [subsequent Windows/Linux verification](remaining-authoring-fixes-20261016.md). |
| Defaults and Inspector | PropertySchema asset defaults account for actual live IDs; exact JSON keys are preserved; Apply without changes preserves prefab inheritance. | [Fixes and regressions](remaining-authoring-fixes-20261016.md), code snapshot `fb63b3ddae7cad52532761932855b755c61fb2c7`. |
| Windows dialogs and Save As | The initial folder is a validated optional hint. Save As publishes the candidate before accepting it in Editor; Cancel/error preserves the original work. A recovery file is permitted when writing the current file fails. | [Dialog and transaction verification](dialog-path-fixes-20261016.md), code snapshot `0fbfb9ca74a139253b4e9d611b2b8877cfb4166d`. |
| Linux | Native Editor, Player, and Full/Team exports were verified in the existing Ubuntu/WSL2 environment; X11/Wayland windows use WSLg/llvmpipe. This is limited functional support. | [Launch requirements](linux-support.md), [authoring acceptance](remaining-authoring-fixes-20261016.md), [latest dialog stage](dialog-path-fixes-20261016.md). |

<a id="открыто-и-не-подтверждено"></a>

## Open and Unconfirmed

- **Native Linux WAV and File/Folder dialogs are absent.** A verification project without audio and CLI operations do not confirm these services; production data must not silently lose sound. See [Linux services](linux-support.md#optional-services).
- **Hardware Linux, portability across arbitrary ABIs/drivers, and manual foreground input-to-display QA are not certified.** WSLg software Vulkan and offscreen package verification cover a different scope.
- The Custom worker manages process lifetimes, but **is not a sandbox for untrusted Rust**. Explicit diagnostic GPU readback, bounded background GPU-retirement waiting, the limited visual Rust subset, and other functional boundaries remain documented in their respective contracts.

<a id="p0-finalization-20261018"></a>

### Current P0 finalization — evidence and decision

Actual candidate09 normal Editor `a3106737…` differs from measured source08 `dcaaf0f3…` despite equal 150/163 native inputs. Its unchanged six CLI routes, fresh cold and actual ARPG Editor/relocated Player validation are PASS: [CLI 6/6](../outputs/verification/p0-finalization-20261018/windows-cli-final-09/result.json), [fresh cold](../outputs/verification/p0-finalization-20261018/cold-final-09/launch.json), [current ARPG pair](../outputs/verification/p0-finalization-20261018/arpg-final-09/result.json); no source08 timing/physical result is relabeled as a new artifact measurement. Final binding follows source10/normal10/new Full10/data-proof10 records in the report and the external decision.

Source08 passes complete default workspace runs: **Windows 712 ordinary/9 ignored; Linux 710/8**, each with all 14 actual artifacts and an explicit zero-test Player-bin harness. Required native/package cases pass separately: Windows 7; Linux 6 plus the explicit Windows-only executable-spelling skip. The fixture-emitter and historical catchup diagnostic remain excluded. Strict static/configuration checks, functional Full/Team/selected Player delivery and current GPU Windows 27/16/Linux 28/16 pass; counts and source/artifact boundaries are in the [report](p0-finalization-20261018.md).

The revalidated candidate09 normal Windows Editor `a3106737…` native ARPG window and new relocated Player `4786c95a…` pass unchanged 1080p/20-enemy Playing gates: actual UI FPS **119.0092/119.8802**, p95 **14.1886/14.0567 ms**, zero measured CPU pixel/readback/upload. Fresh target-cold passes **36.172 s wall / 35.1084651 s internal**, including final Stop readiness. The report retains source08 metrics on `dcaaf0f3…` separately. These checks imply neither causal A/B savings nor clean-machine/scanout guarantees. Linux software-Vulkan Puzzle remains functional evidence with `target_met=false`, not a timing pass.

Foreground01 records native Unicode Save As, Scene transforms/save/reopen, Cancel/Undo, rejected graph connection, Import Cancel and Play/restart/Stop with natural exit and preservation. Its exact 15 author-copy changes and six groups not newly physically certified are explicit; the broad helper checklist is not a new set of gates. Earlier failed attempts remain failed. Final documentation/tool delivery records and the [independent final decision](../outputs/reviews/p0-finalization-20261018/final-review.md) control current P0 closure; hardware Linux, native Linux audio/dialogs and physical input-to-display limits remain.

### Async GPU producer: scoped runtime gates PASS — 18 October 2026

Normal native Vulkan now submits one pending request and polls its exact ticket without ordinary producer GPU waiting or CPU pixel transfer. Four leases and actual consumer completion bound reuse; submitted camera/revision/tick/provenance and Scene owners stay attached to their pixels. Explicit diagnostics retain synchronous readback; only owned background teardown may wait up to30 s on its own fence, pinning unconfirmed resources/device on errors. Fixed 60 Hz simulation and the120 Hz presentation target are unchanged.

The [async GPU report](gpu-async-20261018.md) binds Windows08 `318ae8c06c406c959aad706ecf16efbe45e368b648d62f2ebf8176ba5316d8fe`: current27/16 GPU cases/exact images,307 ordinary/3 ignored scoped tests plus two separately executed ignored native UI tests, strict Clippy, warm whole-Editor Graph/Scene/Game+Play/Stop, selected18-file tools-free Player/relocation and AB03 all 12 processes. Editor A/B means the native ARPG window launched by the normal Editor artifact; standalone Player is separate. Accepted supplemental audits reconstruct actual UI/callback/worker series, correcting F4 helper labels without changing original reports. Median worker FPS Editor 120.0422→119.0027/Player 119.0875→120.0249; worker p95 increases0.3543/0.3731 ms. Actual UI p9514.0170→14.0375/14.0813→13.9872 ms. Unchanged scoped gates pass; no blanket no-regression or universal improvement claim.

Linux L01 `73a77fe8e06bd1a10fcc99f876c4dc7b7e612b396ab00c6c23c9942cfc142066` separately binds actual28/16 offscreen, normal Editor, strict Clippy, X11 resize1280×800→1440×900/Graph/Scene/Game/PlayStop/natural exit and ordinaryWM_DELETEempty-fixture close with all owned groups absent and no forced-cleanup success. Scoped coverage is a nonoverlapping303 ordinary/3 ignored logical union across failed v1's successful Core/Host and v2's five remaining suites, not one full-workspace run. JSON-only proof shows150 native/163 complete input maps equal Windows08; only an opaque settings entry is additionally inventoried by WSL. The initial pre-Cargo guard failure and DrvFS executable-bit fixture failure remain failed with explicit receipts. WSLg/llvmpipe is bounded functional evidence; native Linux audio/dialog, hardware/ABI/real device-loss, physical scanout and a newcold certification remain outside it.

The report links the external final source inventory and independent decision; their recorded scope controls interpretation. Runtime hashes remain Windows Editor `cebd1391…` / Linux ELF `e7ad6a75…`; the planned source-complete Full and separately retained normal EXE have their own later delivery receipt. No new whole-P0/whole-workspace acceptance is claimed. Historical17 October P0 below remains unchanged.

### Historical pre-async GPU stage — 18 October 2026

The [ME-01 receipt](../outputs/verification/p0-continuation-20261018/me01-receipt.json) maps the inherited source snapshot to the exact Editor/Player packages and recorded results. ME-01 reconciles documentation; it is not a native rerun or closure of the cold gate.

Normal native pixels remain on GPU through opaque Core leases and platform interop; four leases and actual consumer completion bound reuse. In that pre-async snapshot, producer completion waiting remained synchronous, and explicit CPU/software/Glow/PNG diagnostics retain readback. The [continuation report](p0-continuation-20261018.md) binds the restored normal Windows source `cedf173e70f531c8f1784a14ec02830a4aea905f7ff0ebd3aea5a0a07484a6cb` and its exact artifacts: first eligible fresh cold attempt **40.906 s wall / 39.9128549 s internal**, unchanged current ARPG gates (Editor 119.05/Player 119.00 FPS), Windows22/16 and separately bound current WSL Linux23/16 offscreen cases/exact comparisons, and scoped Windows authoring/export/relocated Player 120-frame evidence. This single cold attempt does not guarantee startup time; earlier failed cold attempts remain failed and their cause unknown. Diagnostic traces and admission-limit1 were removed; production default2 and original runtime remain.

In that historical run, WSLg/X11 resized Editor and clean ordinary WM_DELETE close pass. A synthetic Linux pre-submit release-failure pair verifies the expected production error and quarantine without CPU fallback; it does not simulate actual hardware loss or establish hardware Linux support. The earlier close timeout showed an Unsaved Changes migration guard in a generated fixture; failed fixture/cleanup receipts remain preserved. Native Linux audio/dialog services, physical input/scanout and arbitrary ABI/driver portability remain outside these results. The [original GPU report](gpu-presentation-20261018.md) retains its historical source15 A/B, source11 Linux and cold failures. Final source binding and independent conclusion belong to the continuation's external records; full P0 or whole-workspace acceptance is not claimed. Historical17 October P0 below is unchanged.

### P0: accepted within the documented scope — 17 October 2026

**P0 verification and independent review are complete within the documented scope.** The [final evidence report](p0-stabilization-20261017.md) binds Windows runtime `b260549403e638653b75e880598b507d7965164b11096399cb18006eadbdab2c` and Linux Scene tests `94c60e636ca77ae19609e723913a745205da950305210236a60d8033e62e8c70` to actual artifacts. Normal Editor SHA256 is `de7ae73680f0c86d1ae1b33fa7de313ecffab1589906b72551debea593e2a9e9`; own standalone Player is `27b8ca7c7486a24b7f2df8c1e0927eb61b0a52881c23b77d078f990709c47427`.

The unchanged 1080p/20-living-enemy/Playing gate passes: Editor 119.922177561 FPS, worker mean 8.338741176/p95 9.3295 ms; Player 117.082020875 FPS, worker mean 8.541021008/p95 9.2637 ms. Callback p95 is 7.5085/7.6059 ms and UI-observed unique p95 is 13.8867/14.0195 ms, separate series. Each report has 120 distinct completed presentations; origins 60/60 and 61/59 committed/interpolated, game/wall ratios 1.00535937/0.99658117, zero frame/wait errors. Focused-frame counts 145/146 with user-input frames zero do not prove held input or physical scanout latency. Simulation stays fixed at 60 Hz; eligible presentation targets 120 Hz without a guaranteed measured cadence.

The single-attempt target-cold run passes both <=45 s clocks: 37.14 s wall/36.1642576 s internal, with a new process, fresh Full03 and previously absent empty `work/pcB260-03`. Existing tools/offline registry and uncontrolled OS caches remain. Actual [native/relocation](../outputs/p0-stabilization-20261017/scene-input-fixed-arpg-relocation-01/result.json), [cold](../outputs/p0-stabilization-20261017/final-cold-editor-03/launch.json) and [FG04](../outputs/p0-stabilization-20261017/foreground-current-04/result.json) receipts preserve source/protected data.

Scene input's original 1-pass/2-fail red is retained; its corrected native set passes all 16 tests on Windows/Linux. Actual FG04 Move/one Undo/Rotate/Scale/Save changes exactly two selected transform leaves (Y rotation `0.9277253`, Y scale `1.0593712`); Editor closes with exit 0 and an explicitly author-transcribed tool receipt. Six captures have JPEG bytes despite `.png` names. Earlier scoped FG02 supports Unicode SaveAs, menus/Cancel/Assets/Play guards/history. Generic graph/DragValue drag, held keys/release, MMB/Shift+MMB, Ctrl-wheel and mid-held Escape were not exercised physically. Editor opens only for short specific UI tests and closes immediately afterward.

Applicable functional/native/package evidence, six normal CLI cases, strict Windows checks/Rustdoc, supported Full stage and scoped Linux WSLg/X11 software-Vulkan Puzzle/Team/relocated Player are recorded in the full report with each source identity. Linux functional `target_met=false` is not Linux ARPG FPS/cold/hardware/Wayland acceptance. Default coverage is a mixed-snapshot logical union: **Windows 666 ordinary/9 ignored, Linux 664/8**; the [unique-name ledger](../work/verification/p0-stabilization-20261017/composite-test-inventory-scene.json) adds only nine new Scene names/platform, not duplicate 16 passes or all-feature overlap. It is not one successful current full-workspace invocation. Failed guards/cache fixtures/race/observers remain failed reports with bounded supplements/reruns; all 33 protected files and initial user data are preserved.

The [independent final decision record](../outputs/reviews/p0-final-20261017/review.md) binds the final documentation/source snapshot separately from the unchanged native inputs. Native Linux WAV/dialog gaps, hardware/ABI support limits, physical input/scanout certification and untrusted Rust without a sandbox remain explicit limits; future portfolio systems are separate work.

<a id="как-выбирать-доказательства"></a>

## Choosing Evidence

The latest dialog-fix code snapshot is `0fbfb9ca74a139253b4e9d611b2b8877cfb4166d`: its [report](dialog-path-fixes-20261016.md) covers three changed Rust files, Editor tests, and the listed native scenarios. The full Windows/Linux workspace run from the [previous authoring stage](remaining-authoring-fixes-20261016.md) applies to `fb63b3ddae7cad52532761932855b755c61fb2c7`; its counts cannot be presented as a new full run of the current sources. The performance results in [review-fixes](review-fixes-20261016.md) remain historical; the current P0 report records later source-bound measurements.

The finished `outputs/ModuleEngine.exe`, `ModuleEngine-review-fixed.exe`, `ModuleEngine-authoring-fixed.exe`, and `ModuleEngine-dialogs-fixed.exe` correspond to different source snapshots. Choose an artifact by its report and SHA; the date of this page does not update the EXE or source inventory.

<a id="маршруты-проверок"></a>

## Verification Routes

- Windows: root [verify.ps1](../verify.ps1); the optional `-NativeBenchmarks` branch requires a desktop and an isolated measurement.
- Linux: root [verify.sh](../verify.sh); native toolchain/DXC/Vulkan requirements are in the [Linux guide](linux-support.md). The script includes offscreen checks and does not complete desktop/export acceptance.
- The [registry and stdlib launcher](verification.md) add explicit `quick`, `contracts`, `clippy`, `native`, `native-tools`, `package` profiles; `--plan`/`--list` read configuration without Cargo. Native tests run directly from the CargoJSON-selected executable with the normal loader environment. Windows-only checks are marked `skipped` on Linux. `package` checks minimal Cargo product/snapshot fixtures, not a full Full/Team/Player export.
- The scope of legacy invocations without arguments is preserved; Windows `-NativeBenchmarks` selects the actual normal Editor from CargoJSON and still does not require Python. The new launcher requires an already installed Python 3.9+ without third-party dependencies. Performance remains a separate opt-in legacy/targeted route with its previous limitations.

The launcher and its unit fixtures are implemented separately from Rust. Unit checks and planning are not a new engine/native/package acceptance; real profiles require their own reports on the current snapshot. Work and data-preservation rules: [AGENTS.md](../AGENTS.md).

Three [Inspector property checks](verification.md#inspector-property-checks) add 99 finite scenarios through real Apply/save/reopen: exact keys and null, opaque local payload, future prefab/default inheritance, and intentional local edits. This strengthens tests of existing behavior; no new dependencies or product changes were required. The sensitivity check actually detected a temporarily reintroduced bug that parsed `literal.key` as a dot path; the correct implementation was then restored byte for byte.

At this stage, `contracts` passed on Windows and Ubuntu/WSL (178 tests each; platform-specific sets differ); Windows `native`/`package` were verified separately during the routing stage. The [rollout results and boundaries](verification.md#rollout-verification-on-16-october-2026) do not replace full engine acceptance or acceptance of published EXEs.

On 17 October, `contracts` gained four existing Save As tests and three
[workflow preservation checks](verification.md#save-as-and-file-history-preservation)
with 28 finite scenarios. Windows and Ubuntu/WSL each passed 185 Rust tests on source
`editor_workflow.rs` SHA-256 `88e8f1d12cc01ce369d5e016e2107fdeb42b5706ab93078047b56b4a8a4aa1cf`.
Temporarily disabling rollback caused a real failure due to changed files;
the source was restored byte for byte, and the targeted test then passed. This strengthens tests,
without changing production behavior or Cargo dependencies; previous performance
and delivery limitations remain.

[GitHub CI](ci.md) runs separate Windows/Linux `contracts` and `clippy` checks, including
documentation PRs, and selected native/package
routes; public `myjname/ModuleEngine` requires `CI required` for `main`,
including administrators. New versions are assessed by the actual CI for their SHA,
not an old green run. A negative service PR confirmed merge blocking.

On 17 October, an explicit all-workspace/all-targets/all-features `clippy` profile
was added with warnings denied and pinned Rust release/native-target validation.
It does not add formatting, tests, or runtime checks. The profile passed on
Windows (Clippy 3.359 s; preflight 0.219 s) and Ubuntu/WSL (Clippy 27.856 s;
preflight 0.592 s), using existing warm shared caches. Reports:
[Windows](../outputs/ci-editor-builds-20261017/verification/20261017T015811Z-985d823c/result.json),
[Ubuntu/WSL](../outputs/ci-editor-builds-20261017/verification/20261017T015921Z-6ef11385/result.json).
These are not cold measurements or hardware acceptance. The
[stage-1/2 hosted run](https://github.com/myjname/ModuleEngine/actions/runs/38994338494)
passed Windows/Linux Clippy, contracts, selected native/package routes and the aggregate;
previous acceptance boundaries remain.

The [Editor diagnostics route](editor-diagnostics.md) uses a supplied normal executable,
generated fixtures and isolated Recent/layout. Its
[Windows both run](../outputs/ci-editor-builds-20261017/diagnostics/20261017T020938Z-e9e189aad466/result.json)
passed workflow and native WGPU/Dx12 capture; unsupported window-system and timeout
checks rejected their cases with preservation and owned-process cleanup evidence.
Final Windows `contracts` passed 187 Rust tests in 16 steps, Clippy passed, and the
full Python suite passed with 66 tests and two explicit platform skips; reports are
linked in the guide. The both-run native Scene pane was still preparing; separate
workflow PNGs verify the imported model, and the Puzzle capture shows its board.
Linux native capture remains unconfirmed; the stage-1/2 hosted run passed the
listed CI routes. These results do not establish Scene readiness, cold-start,
compositor responsiveness or full delivery.

[Build/verification measurements and cache policy](build-verification-performance.md)
separate target-cold compilation from repeat tests: both local `contracts` cycles
passed 187 tests with unchanged source/protected data. Whole-cycle time was
182.375/89.906 s; this does not establish hosted benefit or Editor cold-start.
The conservative CI Cargo cache preserves every verification step; actual hosted
baseline/miss/hit outcomes belong with their reports, including transfer/save costs.

[Independent review](review.md) is required for substantial changes under AGENTS.
Another agent/person examines the diff, goal, actual paths, and evidence. The stdlib
`review_snapshot.py` binds the conclusion to HEAD and tracked/untracked source bytes;
a successful comparison is not itself approval. The [template](review-template.md)
records findings, fixes, rechecks, and unverified boundaries.

## Assistant workflow

[Communication and context guidance](assistant-workflow.md), a repository debugging
skill and adaptive subagent settings were added on 17 October. New subagents default
to `gpt-6.1-sol` / `medium`; explicit launches select supported effort by task.
This does not change an active parent turn or the user's global `ultra` setting.
The [read-only Serena pilot](assistant-efficiency-20261017.md) resolved nine checked
caller references after workspace-loading configuration was corrected. Overall speed
and account-cost gains remain unmeasured; no always-on MCP registration was added.
These changes concern assistant tooling and documentation, not engine/native acceptance.
