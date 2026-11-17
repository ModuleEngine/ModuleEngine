# 0.4.1 — from the anti-aliasing control to the image in Player

[English](release-041-plan.md) | [Русский](ru/release-041-plan.md) | [Product roadmap](product-roadmap.md#render) | [Owner check](product-roadmap-checks.md) | [Documentation](README.md)

**Status: implementation and scoped engineering evidence are recorded; human usability acceptance is pending.** See the [engineering report](release-041-verification.md). This remains the implementation and acceptance brief, not an unqualified release report. Its product scope is [editor stage 3, tasks 3.1–3.6](editor-clarity-plan.md#stage-3). Anti-aliasing already exists. The job is to let an author find it unaided, see its effect, restore the previous state and ship the same choice in a standalone game.

The [current 0.4.1 assignment](#player-settings-handoff) follows the original six steps and incorporates the owner's feedback.

## What each person receives

- **Game author (Team):** normal launch of a “Contrasting edges” sample, a permitted anti-aliasing choice and an explanation of what changes. No Engine Overview, sources, JSON or terminal required.
- **Configuration author (Full with compatible sources):** a route from Engine Overview to the connected renderer's actual graph, with a clear distinction between “pass present, effect disabled” and “pass absent.” Missing sources do not grant Full rights; changing layouts cannot change permissions.
- **Player:** a relocated Player package shows the saved selection. An in-memory edit is never presented as an exported change.

Out of scope: a new AA algorithm, redesigning all graphs/windows, switching GPU APIs, a general module manager, adopting updates in game B, G2 or a 1.0 release. Do not add an Apply button to an operation that applies immediately. Do not bump a binary version as proof that the milestone is complete.

## Work sequence

| Step | Work | Observable exit |
| --- | --- | --- |
| 1. Baseline | Prepare a separate project with a fixed camera, contrasting diagonals and a connected FXAA pass; produce normal Full and matching Team setups. Record resolution, view, settings and ordinary launch path. Avoid conditional rules overriding the manual selection. Walk the existing route, record the first obstacle and preserve what works. | Two comparable reference modes and a baseline tied to build identities. If even the reference images show no visible difference, fix the example or trace the render path before polishing labels. |
| 2. Discover the control (3.1–3.2) | In Full identify the connected renderer, explain AA (smoothing edges) and AO (darkening where surfaces are close), show who owns the pass and open its graph. In Team check the separate Project Settings route. Explain which prepared variants are available to the game and who can change that set. | Each role finds its action from an ordinary launch, including with a saved layout. Navigation does not alter the document or trigger a build. |
| 3. See the effect (3.3) | Trace FXAA/off from the exposed variants through authored data and the running renderer to a frame. Repair demonstrated gaps only. Inspect the same game scene and object edge under matching conditions. | The person can see the difference at the edges. Menu screenshots, a changed camera or different scene contents cannot replace comparable frames. |
| 4. Remove and restore (3.4) | In a Full copy check removal/restoration of AA and AO separately: valid bypass, connections, exposed options/conditions and selection. Distinguish undoing removal from adding a new pass: Undo restores the original edit as a whole; fresh placement is not a promise to restore all previous custom values. | Panel and graph agree after removal, Undo, Redo and fresh placement. Team cannot alter topology. One user edit does not leave partially restored history. |
| 5. Failure, readiness and persistence (3.5) | In a copy reject an incompatible connection or invalid HLSL candidate. Distinguish changed/applying/ready/failed from saved to disk; disclose Stop/build requirements where real. Check cancellation of supported operations, a hidden draft and a no-op. | A specific rejection reason, recovery route and last working version are available. Rejection preserves authored data. At export the user can distinguish the saved version from unfinished edits. |
| 6. Delivery (3.6) | Save the choice, close and reopen; export comparable Players for on/off. Launch the relocated package; repeat the relevant route from a Team delivery. Check affected Full/Team paths, Undo/Redo, keyboard focus and a narrow pane. Deliver the verified sample through the ordinary launch path. | Real packages complete the route. Supply a short owner card, two comparable frame pairs (Editor and Player), rejection/restoration results, composition/hashes and a scoped independent conclusion. |

**Starting points:** [settings](settings.md), [Overview](engine-overview.md), [Save/export rules](editor-workflow.md), [GPU passes](gpu-hlsl.md). Existing code includes [variant selection](../crates/editor/src/properties.rs), [pass removal and Undo/Redo coverage](../crates/editor/src/app.rs), an [existing GPU settings check](../crates/editor/src/settings_verification.rs) and [Host compatibility checks](../crates/game-host/src/compat_tests.rs). The low-level Runtime demonstration is useful but cannot establish the ordinary Team game → Player route.

## Accepting the result

**Owner card, without menu names:** “Make the object edges in this game less jagged. Show the difference, restore the original choice, pick a setting for delivery and check it after reopening and in the game you received.” For Full add: “Remove the capability itself, see what the game author can now choose, then undo.” Give the participant a ready copy and build; do not ask them to prepare the demonstration.

**Usability:** the owner and an independent author in an appropriate role. Supplied help is allowed; a hint from an observer means “with help.” Record time finding the control, active editing time and build waits separately. Planning targets are five minutes to find the control and ten active minutes for the edit and return; these are not measurements or substitutes for correct behavior. If a participant is unavailable, mark it unverified; automation and the implementer cannot stand in for that participant.

**Engineering:** use the existing checkout and fixtures separate from authored data. Establish the initial diff; inspect the relevant verification plans, then run native/GPU measurements sequentially. When runtime changes, retain the existing budgets of the affected scenarios. Bind reports and artifacts to HEAD and a source digest including uncommitted files. Separate author runs, independent checks, human observations and unverified conditions. Do not replace historical packages or a user's running editor to prepare checks. See [verification](verification.md) and [independent review](review.md).

<a id="handoff"></a>

## Handoff to an implementation chat

Use one new project chat with **GPT-6.1 Sol** for all of 0.4.1, including fixes from its review. Keep planning and future milestone decisions in the planning chat. Use the old chat for the history of disputed decisions. A separate person or clean-context agent reviews the change; this need not be another user-owned chat.

**Original kickoff prompt (steps 1–6):**

> Implement only ModuleEngine milestone 0.4.1 from docs/release-041-plan.md in the existing checkout. First read AGENTS.md, current status and editor clarity stage 3. Record the initial diff, reproduce the route from normal launch to Player in safe Full/Team copies and find the first actual gap. Preserve what already works. Then complete and verify steps 1–6. Do not relax Team rights or alter personal projects/layouts. Supply an owner card, comparable frames, source/package bindings and independent review. Mark checks requiring human participation separately. Do not advance to 0.5 or G2, create a worktree or publish changes.

<a id="player-settings"></a>

## 0.4.1 follow-up — Project Settings and a setting in the game menu

On November 16 the owner reported quickly finding anti-aliasing in Project Settings. The screenshot shows three controls alongside permanent explanations of permissions, graphics preparation and saving. The engineering report above covers the previous delivery. The [follow-up engineering report](release-041-menu-verification.md) records the implemented compact pane, Custom menu and per-game preferences with separate-process Player checks; human acceptance remains open. The requirements below remain the acceptance brief.

1. **Simplify Project Settings.** In a wide pane, put the section list on the left and the current section heading and label/value rows on the right. Keep sections and fields reachable by keyboard in a narrow pane. Show settings for available capabilities. Make field explanations available on request and put variant conditions in an expandable section. Use short indicators for unsaved changes and pending preparation. Keep errors visible with an action the author can take. Remove permanent idle messages.
2. **Let authors bind a setting in script.** The author creates a control in their own menu and supplies the stable ID of a permitted setting. Through Custom Rust they can enumerate its allowed options with IDs and labels, read the effective choice and request a change. The configuration defines the values and their commands. In the sample game, bind anti-aliasing to the declared FXAA and Off options. The game controls the presentation. The menu author needs neither a pass ID, a duplicate list of options nor architecture editing rights.
3. **Persist the player's choice.** Start a new profile with the project author's choice. Apply an accepted change to the current game image and persist it for this game and local user across Player launches. Preserve the authored project and distributed files. Define and verify precedence with existing conditions and commands so the displayed selection agrees with actual rendering. Preserve the working state on a rejected request. If a saved option becomes unavailable, use a valid current project choice. Hide a control whose setting was removed. Report whether a change will survive exit when writing fails.
4. **Verify the delivered game.** Supply an editable Team binding example and a standalone game. Start a clean profile at the authored FXAA value, select Off in the game's menu and compare the same edge. Fully exit the process and launch the same delivered game again. Both the menu and the image must show Off. Launch with a separate clean profile and verify FXAA. Compare project and package files before and after. Give the owner the launch path, a short check card and the location in the sample where they can edit their own menu.

<a id="player-settings-handoff"></a>

### Prompt for the existing 0.4.1 chat

> Continue ModuleEngine 0.4.1 implementation in this chat. Complete the “Project Settings and a setting in the game menu” follow-up in docs/release-041-plan.md. The owner found anti-aliasing quickly, but permanent explanations crowd the settings list. Simplify the pane with sections, setting rows, help on request and concise change or error feedback. Let game authors use Custom Rust in their own menu to enumerate the permitted options of a setting by stable ID, read the selection and apply another choice. Demonstrate the binding with anti-aliasing in the sample game. Read options and commands from the configuration. The game owns the presentation.
>
> Deliver a standalone Player. Start a clean profile at FXAA, choose Off in the game's menu, compare the image, exit completely and relaunch. Verify Off in both the menu and the frame, FXAA for a separate clean profile and unchanged authored project and package files. Check a narrow pane, a rejected selection, an unavailable option and a save failure. Work in the existing checkout under AGENTS.md. Preserve the previous delivery and verify the new one on separate copies. Provide a launch path, short script example, owner check card, actual results and independent review. The scope is this 0.4.1 follow-up.
