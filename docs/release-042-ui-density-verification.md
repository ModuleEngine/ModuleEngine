# Compact native chrome verification

[English](release-042-ui-density-verification.md) | [Русский](ru/release-042-ui-density-verification.md) | [Documentation](README.md)

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

The candidate compacts the actual native shell, common Area chrome, Scene
overlays and collapsed operation/status rows. At the same 1580×940 physical
client size, 100% UI scale, two Scene leaves and 50/50 split, each actual GPU
Scene changes from **762×715 to 787×803 pixels**: **+25 width, +88 height**,
15.992% more image area. Native logs show requested and displayed dimensions
equal at pixels-per-point 1. This is image area, not a frame-rate measurement.

At the end of automated verification, final visual confirmation was **incomplete**;
this is the historical automation status before the manual acceptance above.
The final matched screenshot and
native detached/narrow/zoom checks were blocked by Computer Use repeatedly
reporting physical Escape stop, including a fresh read-only inventory after
explicit resume and keyboard-free confirmations. No final screenshot exists.
The repeated entry used listWindows instead of the required first-call getState;
this procedural error is recorded and the stop cause is undetermined. The tool
response does not prove that a new physical Escape occurred. No further app
input or alternative UI mechanism was used after each stop;
completion and session reset succeeded. CPU checks do not replace these gaps.

## Implemented scope

Shared local chrome metrics live in editor_ui.rs; global form/menu typography,
Inspector and Assets spacing remain unchanged. The Project row retains 32-point
Play/Save buttons with equal 1-point vertical padding. Workspace tabs and+ share
28-point height; an explicit 2-point separator owns the inter-row gap, instead
of accumulating parent spacing. The final source expects a 1-point gap from tab
underline to separator; its final native visual check is still open.

Docked and detached hosts use the same 30-point Area header, 28-point selector
and Area button, and 1-point body inset. A 2-point visible divider gutter keeps
an 8-point grab band. Thus the actual Scene-image-to-image distance is 4 points
(2 gutter plus 1 inset per pane), not 2. The raw free header grab excludes the
horizontal divider boundary. Divider widgets are registered once after pane
content; static selector/Area exclusions preserve no-hover batched clicks.
Reading the prior pass's interaction is gated by the current parent's enabled
state, so a blocked UI cannot resize for one stale frame.

Scene controls are 32 rather than 36 points; opaque groups are 34 high with 1-point
padding,2-point control/group gaps and 2-point image inset. The left rail changes
44×124→34×102; the right cluster 316×44→240×34. Speed changes 112×36→96×32, with
local horizontal field padding 2 so Speed 1000.0 remains readable. Fonts, icon
glyphs, enabled/selected states, hover help, snapping and camera semantics stay.
The collapsed operation/status rows were captured at approximately 20 points
each, versus 34/32 before (JPEG edge sampling has pixel uncertainty). Expanded
details restore ordinary theme spacing and retain progress/errors/actions.

## Evidence and limits

The [comparison](../outputs/ui-density-042/comparison.html) displays original
native JPEG bytes from the baseline and an explicitly labelled intermediate
candidate with 787×795 Scene. Both are 1582×972 including the OS frame, with the
same client/layout/cameras; no screenshot pixels are resized or recompressed
in the evidence files. CSS can fit them on screen or display 1:1. The final
803-height measurement is from native render telemetry, not that intermediate
image. [Measurements](../outputs/ui-density-042/measurements.json) preserve
dimensions, identities, original hashes, stop sequence and remaining gaps.
Initial automatic PNGs show Preparing Scene and are excluded from acceptance.
The CUA JPEGs originally had.png filenames; exact-byte.jpg copies correct the
extension while preserving the original evidence.

Native intermediate checks verified readable Speed 1000.0 and the outer edge
of the Area button opening its real menu beside the divider. The first helper
typeText calls delivered no characters; documented key events worked. This
does not establish a product text-input defect. Final native edge dragging,
detached hosts, narrow windows, UI zoom and the final menu stripe remain open.

Windows checks, normal feature set, offline locked Cargo:

- Scene 44 tests pass, including narrow overlays at 100/150/200% logical scales,
  text entry, snap submenus, camera wheel ownership and exact physical image size.
- Dock 17 tests pass, including actual ready Scene interact/paint at both±3 px
  divider edges, both axes, main/detached trees, no-hover batched selector/Area
  edge clicks, header/divider boundary and enabled→disabled held drag.
- Activity 5 and workspace 4 tests pass. Strict Editor all-target Clippy and fmt
  pass; no full workspace or native acceptance suite is claimed.

The unchanged large-Scene CPU test initially failed because egui's first sizing
pass exposed 10000×10000 with a 2048 default texture limit. An archived old-Scene
module reproduced the same failure with byte-identical configure/image_area
helpers. Only the test fixture was bounded to 1600×1000 with a 4096 simulated
native limit; the exact required 2400×1500 at 150% remains, with no resolution
reduction. Earlier failed logs and red divider/Speed probes are retained.

## Delivery and preservation

Final03 differs from the telemetry-bearing final02 only by the current-enabled
input guard and its regression test; geometry does not change. Final03 native
automated visual checks are not claimed; owner manual acceptance is recorded above.
The normal Full/Team delivery launchers are
[Full](../outputs/ui-density-042/final03/launch-full.cmd) and
[Team](../outputs/ui-density-042/final03/launch-team.cmd).
Final03 Editor SHA-256: `a6667310134e3d90b9562f2fe5309556d0d5bdcbc160eba034809ed2305296e4`.
The source snapshot, task-only patch, original dirty baseline witnesses,
package inventories and independent conclusion are under
[the review packet](../outputs/reviews/ui-density-042/review.md).

The initial dirty checkout is preserved; authored project/scene data, root SDK
metadata, source rights, original ICO, SSAO, FXAA and native geometry feedback
logic are unchanged. main.rs is byte-identical to its initial witness; the
temporary baseline-test include was removed. The running final02 fixture and
all previous deliveries stay untouched. Player is reused byte-for-byte.
The owner's report that resize jitter is no longer noticeable is an owner
observation, not physical/DPI certification. No Linux, mixed-monitor, cold-start,
or game-performance certification is established by these engineering checks.
The checks alone did not establish complete release acceptance; the later owner
milestone closure is recorded above.
