# 0.4.2 — native pane headers and instance close

[English](release-042-native-panes-verification.md) | [Русский](ru/release-042-native-panes-verification.md) | [Documentation](README.md)

The follow-up keeps one document, history and Play session while unifying docked
and detached area headers. It does not change rendering, FXAA, SSAO, project
formats or Full/Team permissions. The existing SSAO shader is preserved with
SHA-256 `65c9b086813cd25da93ae36408f23d3e0e084867665aa17b468e7773a1395a92`.

## Behavior

The client header shares the editor selector, **Area** and free drag space with
docked areas. The **Area** menu is a vertical three-dot vector button at the right
edge, with Area as its hover/accessibility name. Per the owner's final preference, detached windows retain the OS
title/name, system **×** and native edge/corner resize frame. The client header
has no second **×** or File/Edit row. OS decorations remain stable during moves,
preserving the native capture; client-header moves use the outer origin while
retaining the whole client size. The area menu owns Close; the client header has no separate close button.

**×**, **Area → Close area**, and system Close remove only that stable-ID instance.
Same-type siblings keep their state; closing never closes the project. The saved
layout omits the closed instance and its pane memory. Reopening through Windows
uses the ordinary existing find/open behavior. Closing during a move terminates
that move without rolling the closed instance back into the layout.

**Area → Return to main window** and drag-to-dock remain explicit operations.
They preserve the instance ID/view state; Return uses a safe available slot when
its previous neighbor changed. Main-layout last-area protection remains intact.

## Resize investigation

The original decorated native window was tested on an isolated Full copy with a
ready Scene at 760×510 logical points. System Size changed width 760 → 749 → 738;
changed-geometry samples showed no reversal or oscillation. The first fixture
launch lacked the configured offline Cargo cache and failed to compile Custom;
that environment failure was corrected before observing the ready Scene.

This does **not** reproduce or establish a fix for the reported slow held-resize
jitter. Computer Use cannot inject a slow path, hold at a boundary, then release;
its screenshots also omit the decorated outer resize border. Fast automated
edge drags did not establish that gesture. No speculative geometry-feedback
patch is credited as a jitter fix. The opt-in `MODULE_ENGINE_AREA_GEOMETRY_TRACE`
logs at most 2048 changed samples per native area, including time, actual saved
geometry, requested position/size and scale, for a future distinguishing probe.

## Evidence and remaining checks

Evidence lives under `outputs/native-panes-042` and `work/native-panes-042`.
The baseline/candidate copies and isolated personal roots do not replace the
published SSAO delivery or the user's personal layouts. The source-bound review
packet is under `outputs/reviews/native-panes-042`.

The earlier borderless candidate (preserved as intermediate evidence) confirmed
the shared client header, explicit Return to two
same-type docked Scene instances, re-detach, and × removing just the detached
instance. Saved JSON retained Scene ID 2 and its memory, with no detached area.
Final checks, artifacts and independent conclusion are recorded in the review
packet; intermediate native observations are not full release acceptance.

Physical slow edge/corner motion, holding at the boundary and release, mixed-DPI
monitor moves, hardware Linux and owner image acceptance remain separate checks.
To check manually, open Scene in a new window, resize each edge and corner slowly,
hold, release, and watch OS geometry, header/content layout and Scene image
separately. Repeat the same sizes after any change. Move the floating header
twice, dock or explicitly Return, create two Scene instances, close one by × and
one by system Close, restart, then reopen through Windows. Same-type siblings
must retain their state; a closed instance must not reappear.
