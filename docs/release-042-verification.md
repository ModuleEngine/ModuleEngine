# 0.4.2 — physical image resolution and native pane movement

[English](release-042-verification.md) | [Русский](ru/release-042-verification.md) | [Scope](release-042-plan.md) | [Owner checks](product-roadmap-checks.md)

<a id="owner-acceptance-20261117"></a>

## Owner acceptance — 17 November 2026

The owner reported: “I checked it; everything works and looks fine for now. Let’s close 0.4.2.” The owner had also
personally confirmed that jitter disappeared. **Product milestone 0.4.2 is
accepted and closed.** The next planned milestone is [0.5](product-roadmap.md#compose),
with a separate brief before implementation. Implementation of 0.5 has not started.

Accepted delivery: final03 [Full](../outputs/ui-density-042/final03/launch-full.cmd) and [Team](../outputs/ui-density-042/final03/launch-team.cmd).
Editor SHA-256: `a6667310134e3d90b9562f2fe5309556d0d5bdcbc160eba034809ed2305296e4`.

This is general manual acceptance; no detailed per-scenario owner transcript was
recorded. Mixed-DPI/Linux, final automated captures and previously unperformed
automated scenarios are not declared passed. Earlier “acceptance open” statements
refer to their historical snapshots and do not reverse the owner’s decision.
The engineering evidence and its limits below are preserved.

## Engineering record before manual acceptance

Historical engineering candidate, 16 November 2026. The plan separated image and gesture acceptance; at that snapshot the candidate had not yet been accepted. The later owner decision is recorded above. The reported motion trail is not established as fixed.

## Result

Ordinary Player requests the physical image area at startup and after resize/DPI changes, instead of always rendering 1920×1080 and stretching it. The last good frame retains its aspect during asynchronous resize; input follows the displayed image. Minimized rendering is suspended. Scene and a single Game pane request 100% physical resolution without the old 1280×720 preview cap. Both image edges are snapped to physical pixels, with one rectangle for rendering, placement and input. This prevents an additional linear-filter blur caused by fractional dock boundaries. A render-scale control is deferred.

Multiple Game panes still share one Play frame; differently shaped panes are not certified as simultaneous per-pane 1:1 output. Vulkan explicitly rejects dimensions beyond 8192 per axis or 16,777,216 pixels; it does not silently reduce resolution.

The primary Project Settings command is under Edit, with the Windows → Authoring alternative retained. Dragging the free pane header crosses the threshold into a real native floating window. Root targets preview a side; release docks, release outside retains the floating area, and Escape restores the initial layout. Active gestures suppress other pane actions and layout autosave. Stable area IDs retain state and shared Undo. Capture/focus loss cancels without stealing foreground focus. The existing detach/return commands remain available.

## Image evidence and filter decision

The final native Scene capture is [`scene-100-percent-final.png`](../outputs/release-042/scene-100-percent-final.png): 900×1200 application image, 876×975 Scene request and displayed GPU frame, DPI 1, preparation complete. `work/release-042/scene-native05.*` binds the observation to Code09. Earlier Preparing captures and the wrong-Cargo-cache error capture are retained as unsuccessful attempts, not passes.

Ordinary native Player traces [`player-normal01.jsonl`](../outputs/release-042/player-normal01.jsonl) and [`player-normal03.jsonl`](../outputs/release-042/player-normal03.jsonl) establish settled GPU-resident 1280×720, ultrawide 3440×1417 and final 1920×1080 output matching the physical area. Square/circle proportions were observed in the UI. The second run logged 18 suspended samples over 5,584 ms with renderer calls fixed at 2,976, then resumed. The earlier trace reached its bounded row limit before the final 1080p size; only the later trace establishes that rendered size. Opt-in tracing now writes a final observation after its bounded intermediate rows.

Actual FXAA → Off and a full exit/restart retaining Off were observed with the earlier delivery. The final Player also saved Off successfully using explicit `--state-root`. An initial final-Player attempt omitted that argument, hit Access denied in the default profile and correctly displayed Not saved; that is a test launch failure, not persistence acceptance. Full/Team launchers select the prepared Cargo cache when unset; the default cache on this machine lacks `sha2`. Player needs no Rust compilation. Clean-machine toolchain portability is not established.

[`image-diagnostic-02/comparison.html`](../outputs/release-042/image-diagnostic-02/comparison.html) provides original-size Off/current FXAA/sRGB-control frames at 960×540, 1280×720, 1920×1080, 900×900 and 3840×2160. The matched 60-frame camera movement and 12 stopped frames are reconstructed renderer output, not a desktop Player recording. Stopped frames are byte-equal in all three modes. This does not prove absence of a trail during native presentation or physical scanout.

The shipped FXAA shader remains unchanged. The diagnostic sRGB control reduces measured positive edge lightening around bright lines but changes more pixels and has a cost; there is no perceptual reference establishing it as universally better. The analysis reports 12 warmed wall-clock samples per size/mode, including CPU/readback, on RTX 4080 SUPER. These are not GPU timestamps, cold-start, ARPG 60 FPS or native Player performance certification.

## Checks and delivery

Logs are retained under `work/release-042`; generated evidence and packages under [`outputs/release-042`](../outputs/release-042). The build target is Windows x86_64 MSVC, pinned Rust 1.99.0, release, locked/offline with the prepared workspace Cargo cache. No publication was performed; binary/SDK version pins are unchanged.

| Check | Result |
| --- | --- |
| Affected editor checks | 64 PASS in `checks12`: Scene 43, shared UI 2, drag controller 4, dock 8, native placement 2, navigation 5 |
| Fractional dock regression | Actual Scene image mesh bounds equal the input rectangle at DPI 1 / 1.5 / 2; requested pixels match both snapped edges |
| Player library, all features | 64 PASS, `player-tests02.*` |
| Formatting and strict workspace Clippy | Release, all targets/features; `clippy02.*`; no full workspace test-suite claim |
| Full / Team / Player export | True Full→Team and Full→Player, plus Team→Player using its own SDK/native delivery; no restriction bypass |
| Final Full native gesture | Header tear-off, floating move retaining HWND 32706450, second-drag docking back into root 29495076; contents retained; normal exit without saving authored changes |
| Earlier attempts | Failed threshold/batched-event tests, nested-context deadlock attempts, premature captures, invalid endpoint and launch-profile/cache errors retained separately; not counted as passes |

Launch `launch-full.cmd`, `launch-team.cmd` or `launch-player.cmd` in the output directory. The paired owner cards explain the isolated profiles, Off/FXAA comparison and remaining gesture checks. Team has Engine authoring disabled. The saved choice belongs to the game profile, not the authored package.

The task preserves the initial dirty checkout and old 0.4.1 delivery. The independent review record under [`outputs/reviews/release-042`](../outputs/reviews/release-042) binds HEAD, complete source inventory, task diff and artifact hashes. A snapshot comparison is not reviewer approval; consult the reviewer's conclusion there.

## Remaining acceptance

Physical out-and-back with one uninterrupted hold, visible held preview and Escape during hold still need a human check: the installed Windows automation exposes endpoint drags, not held multi-segment gestures. Raw ordered-event/controller regressions cover these cases but do not substitute for that check. Mixed-monitor physical DPI, hardware Linux, normal Player desktop motion recording and owner judgement of fringe/softness remain open. Neither successful compilation nor an offscreen comparison establishes those results.
