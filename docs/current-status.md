<a id="текущее-состояние-moduleengine"></a>

# Current ModuleEngine Status

[English](current-status.md) | [Русский](ru/current-status.md) | [Documentation](README.md)

**25 November 2026.** The [Cargo workspace version](../Cargo.toml) is `0.4.0`. The owner closed milestone 0.4.2 on 17 November. The current priority is the [R engine rebuild](product-roadmap.md#rebuild). Work on 0.5 and new UI features remains paused until the core R1, R3 and R4 phases are accepted. The [roadmap](product-roadmap.md#map) tracks stage decisions separately from the version printed by Editor and Player.

<a id="цели-и-границы"></a>

## Goals and boundaries

- **World.** R1.8 completed the Bevy-backed ECS storage change within its accepted performance bounds. Core owns stable IDs, the fixed-tick schedule and rollback. Object remains a compatibility provider. GameHost extracts the retained render scene from the accepted world. See the [R1 record](product-roadmap.md#rebuild-r1) and [ECS contract](ecs-world.md).
- **Renderer.** R2.4 is accepted on the recorded reference. The stock path uses retained GPU scene data, frustum culling and indirect draws on qualifying Vulkan devices. With a sampled D32 attachment, a current-frame depth seed also enables Hi-Z rejection. The measurements and their limits are in the [R2.4 receipt](evidence/r24/README.md). R2.5 remains paused, with the [S4 reference](evidence/r25-s4/README.md) saved for it.
- **Review follow-up.** RV-A, RV-B and RV-C are complete in the roadmap. Scene and Play follow the connected provider. History retains opaque authored data while checking required files. Linked-template drafts survive selection changes. The [ARPG tick trial](evidence/rv-c-arpg/README.md) with 20 live enemies supports the recorded [R6 decision](decisions.md#rv-c-arpg-tick). [RV-D scope](product-roadmap.md#rebuild-rv-d) records separate maintenance criteria.

<a id="реализовано"></a>

## Implemented

- **Editor.** Full with verified sources can change Engine and Project. Team freezes the architecture and allows only the exposed Project changes. Scene, Hierarchy, Inspector and Assets edit the authored project. Game runs a separate Play copy. See [editor workflow](editor-workflow.md) and [distribution](distribution.md).
- **Delivery.** Player uses built game logic and SPIR-V. Crate Garden has ten levels and a Windows acceptance record on the reference machine. Two Arenas is the technical ARPG example. See the [Crate Garden receipt](g1-verification-20261101.md), [game authoring](editor-game.md) and [Player](player.md).

<a id="открыто-и-не-подтверждено"></a>

## Open and unconfirmed

The rebuild roadmap records planned renderer, editor, physics/audio and animation work. A clean-machine G1 delivery and independent author handoff remain open. Linux CI uses software Vulkan. Hardware Linux, physical input/scanout, new cold-start measurements and the ARPG frame-rate target each need their own evidence. See [Linux support](linux-support.md) and the [owner check cards](product-roadmap-checks.md).

<a id="как-выбирать-доказательства"></a>

## Choosing evidence

Use the source SHA, artifact hashes, target, features and limits in each receipt. The [status archive](status-archive.md) preserves the dated historical reports and original paths of local evidence.

<a id="маршруты-проверок"></a>

## Verification routes

[CI](ci.md) runs the full workspace and a separate minimal Player build and test on Windows and Linux. Strict Clippy also covers the default Release Editor, where CI checks that `--self-check` reports the feature requirement. Changes to executable sources add native and package checks. For local work, start with the [verification profiles](verification.md#profiles) and inspect the plan for the affected contract.
