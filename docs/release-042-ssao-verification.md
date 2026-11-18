# 0.4.2 follow-up — SSAO self-occlusion, fringe and stepped shading

[English](release-042-ssao-verification.md) | [Русский](ru/release-042-ssao-verification.md) | [GPU contract](gpu-hlsl.md) | [Previous candidate](release-042-verification.md)

The owner's close-up showed a fringe along the white bars and broad horizontal brightness steps on the ground. These static artifacts reproduce with FXAA disabled. Their demonstrated cause is the built-in SSAO pass; this is separate from the unconfirmed temporal-trail report.

## Cause and change

The old shader counted a neighbor as an occluder when its camera-Z was at least 0.025 nearer. Points on an unobstructed tilted plane naturally have different camera-Z values. As the slope crosses the threshold for the eight discrete offsets, the number of contributing samples changes in steps across the plane. The same incorrect test changes shading near silhouettes, leaving a misleading fringe.

The corrected pass reconstructs camera-relative world positions using the existing linear-depth/camera-basis ABI. It tests displacement against the receiving surface normal and applies a smooth world-distance falloff. Coplanar neighbors cannot occlude the receiver; unrelated foreground geometry outside the radius is rejected. Real nearby contacts still contribute. The existing eight offsets, strength, resource bindings, push constants, graph and authored JSON are retained. FXAA, tone mapping, color encoding, render scale and GUI presentation are unchanged. This does not disable SSAO or edit the example to hide the defect.

## Regression and evidence

The new [`ssao_check` example](../crates/render-vulkan/examples/ssao_check.rs) compares actual GPU frames with AO enabled versus bypass, on identical scenes/cameras, with FXAA disabled. An isolated flat plane must be pixel-identical to bypass; a distant floating cube must not cast screen-space contact shading on it; a touching cube must still darken the contact. These expectations do not duplicate the AO algorithm.

| Case, each in Forward and Deferred | Original shader | Corrected shader |
| --- | --- | --- |
| Oblique plane, 640×480 | 204,380 falsely darkened pixels, max channel delta 18 | 0 changed pixels |
| Close plane, 960×720 | 363,314 falsely darkened pixels, max delta 17 | 0 changed pixels |
| Rotated portrait plane, 480×720 | 298,560 falsely darkened pixels, max delta 17 | 0 changed pixels |
| Distant silhouette, 640×480 | 210,026 changed pixels | 0 changed pixels |
| Real cube/plane contact | Positive, but includes plane self-shading | 2,268 / 2,213 darkened pixels, max delta 10 |

The original shader fails eight regression cases. The corrected shader passes all twelve cases, including the visual reference scene. Existing actual GPU bypass/copy, cache-renaming, compiled HLSL ABI, editable-Custom equivalence and textured Forward/Deferred checks also pass (five tests). Original failures, build output and complete logs remain in `work/ssao-042`; original and corrected images/reports are under [`outputs/release-042-ssao`](../outputs/release-042-ssao). Tests ran sequentially on Windows x86_64 MSVC, pinned Rust 1.99.0, locked/offline, release, RTX 4080 SUPER. No new dependency or data/API version was introduced.

[`comparison.html`](../outputs/release-042-ssao/comparison.html) links matched original/corrected/bypass frames at their original sizes. The native Scene capture uses the production editor and GPU presentation at full physical viewport resolution, with the normal FXAA selection retained. The separate Player export carries newly compiled SPIR-V; export/package checks verify the actual delivered files. The paired owner cards and launchers sit beside the comparison. Existing 0.4.1 and earlier 0.4.2 deliveries are preserved.

Final source digest, task diff, artifact hashes, actual native/export/Clippy results and independent conclusion are recorded under [`outputs/reviews/ssao-042`](../outputs/reviews/ssao-042). Read that record for the exact accepted snapshot; image generation or a source hash alone is not review approval.

## Limits

This fixes the reproduced false self-occlusion and broad shading steps. The pass remains an eight-tap screen-space approximation with resolution-dependent sampling and no offscreen occluders; it is not a general shadow solution. The remaining subtle, local shading near genuine contacts is intentional. This static case does not establish a fix for a temporal trail or physical scanout behavior, mixed-monitor DPI, hardware Linux, cold-start or game performance. No new acceptance of the previous docking work is implied.
