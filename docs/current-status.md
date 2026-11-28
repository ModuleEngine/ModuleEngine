<a id="текущее-состояние-moduleengine"></a>

# Current ModuleEngine Status

[English](current-status.md) | [Русский](ru/current-status.md) | [Documentation](README.md)

**27 November 2026.** The [Cargo workspace version](../Cargo.toml) is `0.4.0`. The owner closed milestone 0.4.2 on 17 November. The current priority is the [R engine rebuild](product-roadmap.md#rebuild). Work on 0.5 and new UI features remains paused until the core R1, R3 and R4 phases are accepted. The [roadmap](product-roadmap.md#map) tracks stage decisions separately from the version printed by Editor and Player.

Editor and Player use the shared Vulkan shell. The root and detached windows present Scene and Game graph images on the same device. The [Player contract](player.md) describes image lifetime and diagnostic checks. The [paired S1/S2 result](evidence/r26-player/README.md) records the effect of native window pacing against R0.6.

<a id="цели-и-границы"></a>

## Goals and boundaries

- **Editor builds.** The default Editor links ARPG, Puzzle and G1. The same shell builds without games or with a selected game. The [composition contract](architecture.md#r41-editor-extensions) describes build flags and package registration.
- **World.** R1.8 completed the Bevy-backed ECS storage change within its accepted performance bounds. Core owns stable IDs, the fixed-tick schedule and rollback. Object remains a compatibility provider. GameHost extracts the retained render scene from the accepted world. R1.9 supports [batch edits of transforms and one named JSON component](ecs-world.md#r19-batch-motion). See the [R1 record](product-roadmap.md#rebuild-r1) and [ECS contract](ecs-world.md).
- **Renderer.** R2.4 is accepted on the recorded reference. The stock path uses retained GPU scene data, frustum culling and indirect draws on qualifying Vulkan devices. With a sampled D32 attachment, a current-frame depth seed also enables Hi-Z rejection. The measurements and their limits are in the [R2.4 receipt](evidence/r24/README.md). R2.5 derives image barriers and shares memory across disjoint transient lifetimes. In the [matched S4 check](evidence/r25-s4/receipt.md), Forward and Deferred match the reference pixels and use 43.48% and 51.85% less graph image memory per slot.
- **Review follow-up.** RV-A, RV-B and RV-C are complete in the roadmap. Scene and Play follow the connected provider. History retains opaque authored data while checking required files. Linked-template drafts survive selection changes. The [ARPG tick trial](evidence/rv-c-arpg/README.md) with 20 live enemies supports the recorded [R6 decision](decisions.md#rv-c-arpg-tick). [RV-D scope](product-roadmap.md#rebuild-rv-d) records separate maintenance criteria.

<a id="реализовано"></a>

## Implemented

- **Editor.** Full with verified sources can change Engine and Project. Team freezes the architecture and allows only the exposed Project changes. Scene, Hierarchy, Inspector and Assets edit the authored project. Game runs a separate Play copy. See [editor workflow](editor-workflow.md) and [distribution](distribution.md).
- **Mesh shader experiment.** On the fixed S5 view, the original Tier B prototype reduced the median run mean GPU graph time from 4.552 to 2.461 ms. Meshlet preparation took 15.93 seconds. R2.4 is the production path. See the [source-bound comparison](r81-s5-meshlets.md#r81-result) and [decision](decisions.md#r81-meshlet-prototype).
- **Vulkan editor prototype.** Both painters drew the Editor root on an R2 device and met the timing and text criteria at native 100% and 150% DPI on the Windows reference machine. The R3.2 shell starts with the library painter. See the [measurements and captures](editor-vulkan-spike.md#r31-results).
- **Physics.** An optional [Rapier step](physics-rapier.md) adds bodies, colliders, joints, collision events and queries at 60 Hz. Scene displays physics geometry and Player selects the native feature from the authored schedule.
- **glTF animation.** The importer loads joint hierarchies, inverse binds, skin weights and TRS tracks. GameHost loops the first applicable clip and deforms vertex positions on the CPU. The Khronos playback check covers CesiumMan and Fox alongside the static crystal. See [content and verification](content.md#gltf-animation).
- **Delivery.** Player uses built game logic and SPIR-V. Crate Garden has ten levels and a Windows acceptance record on the reference machine. Two Arenas is the technical ARPG example. See the [Crate Garden receipt](g1-verification-20261101.md), [game authoring](editor-game.md) and [Player](player.md).

<a id="открыто-и-не-подтверждено"></a>

## Open and unconfirmed

The rebuild roadmap records planned renderer, editor, physics/audio and animation work. A clean-machine G1 delivery and independent author handoff remain open. Linux CI uses software Vulkan. Hardware Linux, physical input/scanout, new cold-start measurements and the ARPG frame-rate target each need their own evidence. See [Linux support](linux-support.md) and the [owner check cards](product-roadmap-checks.md).

<a id="как-выбирать-доказательства"></a>

## Choosing evidence

Use the source SHA, artifact hashes, target, features and limits in each receipt. The [status archive](status-archive.md) preserves the dated historical reports and original paths of local evidence.

<a id="маршруты-проверок"></a>

## Verification routes

Documentation-only PRs run link and bilingual roadmap checks and build both map pages. On Windows and Linux, seven verification jobs can run concurrently. For other PRs, [CI](ci.md) runs the full workspace and a separate minimal Player build and test on Windows and Linux. Strict Clippy also covers the default Release Editor, where CI checks that `--self-check` reports the feature requirement. Changes to executable sources add native and package checks. For local work, start with the [verification profiles](verification.md#profiles) and inspect the plan for the affected contract.
