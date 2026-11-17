# Native resize feedback verification

[English](release-042-resize-jitter-verification.md) | [Русский](ru/release-042-resize-jitter-verification.md) | [Documentation](README.md)

The owner captured the current original-icon Full window alternating between
803 and 805 physical pixels at the same (797,156) origin and 803px height. Twelve
continuous frames span 2.59 seconds with eight width changes. This supersedes
previous no-reproduction evidence; those older reports remain historical.

The cause is a delayed geometry feedback loop. Each immediate viewport builder
previously received the last stored native size/position. Its callback samples
native geometry before egui-winit applies the changed builder request; that sample
is then stored and sent back next frame. Pinned egui 0.36.2 compares requested
builder values, not the current OS rectangle. A single out-of-phase pair can
therefore keep requesting alternating sizes without further external input.

A separate hidden, own-process actual Editor/Scene fixture uses the production
Editor.ui/draw_detached_areas path and exact bound HWND. At 803/805 widths,
100ms motion converged after stopping; asynchronous 6ms motion produced eight
changed stale requests but also converged. These are controls, not spontaneous
owner-stop reproduction. A single controlled stale stored sample after motion
stopped then reproduced continued 803/805 alternation. The matched final fixture
has explicit terminal 803/805 commands, no-input intervals at both widths, and
one phase injection from client789 to stored787 at about9.2s. Baseline retained
both outer widths through the final no-input interval; the correction honors
the explicit target once and remains stable. Actual issued commands are recorded
in external-commands.json. Early root sample hints were not authoritative and
are removed from the final matched harness. A controlled injection establishes
the causal boundary; it does not claim spontaneous physical-owner stop reproduction.

The correction keeps native observations for persistence without turning them
into OS commands. A transient tracker is keyed by the full workspace key and
stable host ID, compares only geometry/scale/work areas, and records a permitted
readback after it is stored. Creation/recreation receives builder geometry;
existing-host explicit intent dispatches InnerSize/OuterPosition once in the
already-created child context. Pending commands are consumed, so an immediate
callback rerun after request_discard cannot append them a second time. This also handles restoring a target equal to the
builder's older cached request after the user resized elsewhere. Observed frames
have neither builder geometry nor geometry commands. Drag/pending-readback guards,
monitor/scale placement and Wayland's lack of explicit position remain supported.
No debounce, resize dead zone, rounding rule or forced render resolution is added.

Targeted regressions cover observation without commands, exact original760×510
and(64,64) command dispatch after800×600/(797,156), no stale-sample echo, explicit
move/recreation/monitor/scale routes and existing decorated drag placement. The
matched actual Editor fixture verifies no-input behavior and logs request presence.
Source, complete logs, fixture/project hashes and final artifacts are under
work/resize-jitter-042 and outputs/resize-jitter-042. Temporary diagnostic entrypoints,
hidden-window override and work-module include are removed before the delivery;
only their bounded opt-in geometry trace remains. Full is normally source-sealed,
Team normally exported; no package pin bypass.

Previous packages and the owner's active window remain untouched. The original
ICO, Area parity, SSAO and100% Scene are preserved. Computer Use is not resumed
after the owner's earlier input invalidation. Physical owner visual/held-pointer
and mixed-DPI acceptance remain pending; the scoped native regression is not a
claim about all possible resize behavior, hardware platforms or performance.
