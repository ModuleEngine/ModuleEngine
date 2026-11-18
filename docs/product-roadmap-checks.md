# Product roadmap: owner checks

[English](product-roadmap-checks.md) | [Русский](ru/product-roadmap-checks.md) | [Documentation](README.md)

[Full roadmap](product-roadmap.md#map) · [Next planned milestone: 0.5](product-roadmap.md#compose)

These cards define owner checks, not evidence of completed releases. **The owner reports that 0.4.1 settings work and are clear; full independent human acceptance remains open. The owner accepted and closed 0.4.2 on 17 November 2026; 0.5 is next planned, with implementation not started.** Before a check, the team supplies a ready learning project, the correct build, launch instructions and the expected result. Use copies. Ordinary supplied help is allowed. Record any human hint. This sheet is self-contained and can be printed; the roadmap explains the work behind each card.

Participant: __________________  Build / PC: __________________  Role / rights: __________________

Where I stopped, what I expected and how long it took: __________________

For every card mark: **□ independently · □ with a hint · □ unsuccessful · □ not checked**. Record active time separately from installation/build waiting, wrong turns, help used, hints and expected behavior. A verbal hint makes the attempt “with a hint”; it must not be reported as independent success. Missing access to the required environment means “not checked”, not a pass.

## 0.4.1 — game author · Team

[Work steps: 0.4.1](product-roadmap.md#render) · [Planning detail](release-041-plan.md#player-settings)

**Do:** Find antialiasing in compact Project Settings, turn it off, save and reopen the project. Use help on request and check essential statuses. Bind your game menu through a script to the permitted setting by stable ID, with choices from the current configuration. Export Player with FXAA as the project default.

**Success:** The same diagonal edges visibly differ; the author's choice survives reopening and reaches Player. The menu binding requires no rendering-pass ID.

**Boundary:** In Full, separately delete the pass itself, undo and compare the connections and image.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 0.4.1 — player · delivered game

[Player settings plan](release-041-plan.md#player-settings)

**Do:** In the author's game menu, change FXAA → Off, fully exit Player and launch it again. Separately launch with a clean player profile.

**Success:** Off survives a full restart for this game; the clean profile starts with FXAA. The authored project remains unchanged. Neither the player nor the menu binding needs a rendering-pass ID.

**Boundary:** Check player preferences separately from the author's saved project values. The owner's quick discovery of Project Settings and comment on excessive permanent text do not establish full independent acceptance; that remains open.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 0.4.2 — owner · Player and editor

**Owner acceptance — 17 November 2026:** “I checked it; everything works and looks fine for now. Let’s close 0.4.2.” The owner had also personally confirmed that jitter disappeared. This decision closes 0.4.2 and permits planning the 0.5 brief. See the [accepted final03, SHA-256 and limits](release-042-ui-density-verification.md#owner-acceptance-20261117).

No detailed per-scenario owner transcript was recorded. The original card below is retained without filling its individual lines retrospectively. Mixed-DPI/Linux, final automated captures and other unperformed automated scenarios are not marked passed.

[Work steps: image and docking](product-roadmap.md#image-and-docking) · [Planning detail](release-042-plan.md)

**Do:** In the delivered Player, widen the window and check that the test circle stays round. Use the supplied frame and game-area dimensions to check one-to-one presentation. Switch between Off and FXAA and watch one diagonal when stationary, moving and just after stopping. Fully exit and restart the game. In the editor, open `Edit > Project Settings`. Drag its header outside, move back into the editor with the same button held and release over a highlighted destination. Repeat with Escape, then reopen the editor.

**Success:** The frame and the physical image area have matching dimensions in 1:1 mode. The circle keeps its proportions. Record the appearance of jagged edges and the bright fringe, and whether a motion trail is visible. The game keeps the choice across restart. On release the pane docks exactly where the preview appeared. Escape restores the previous layout. After reopening, the pane is where you placed it.

**Record separately:** Player physical 1:1: __________; resize/aspect: __________; stationary FXAA Off/on: __________; moving FXAA Off/on and trail: __________; choice after restart: __________; `Edit > Project Settings`: __________; continuous drag out/back and preview: __________; release: __________; Escape: __________; area state/reopen: __________. Use the four result labels above for each outcome.

**Boundary:** The general manual acceptance above closes the product milestone, but does not establish a separate diagnosis or proof of a temporal-trail correction or replace missing measurements. The original card and historical engineering reports retain the actual status of individual checks.

## 0.5 — configuration author · Full

[Work steps: 0.5](product-roadmap.md#compose)

**Status:** next planned milestone; implementation will be separate work after its brief.

**Do:** Remove audio while an event still refers to it. Then remove the request and try again.

**Success:** First it is clear what blocks removal; after the change, the game starts without the service. Returning restores the previous value.

**Boundary:** The team shows the package composition. A missing button alone is not proof.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 0.6 — owner of projects A and B · Full

[Work steps: 0.6](product-roadmap.md#pin)

**Do:** Change B's appearance. Build a new game from the previous project A. Return B to its original option.

**Success:** A retains its old appearance and does not request an update. B was restored, not A.

**Boundary:** Confirm that A is a fresh build, not a previously retained EXE.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 0.7 — extension developer + game author

[Work steps: 0.7](product-roadmap.md#extend)

**Do:** Ask a colleague to add their own pass and a small Rust module. Check the parameter they expose in Team yourself.

**Success:** The result is visible in the game. The colleague could replace lighting in the selected chain; another game does not need the module.

**Boundary:** An incompatible connection has an understandable rejection reason and preserves the working option.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 0.8 — new developer · Full, clean PC

[Work steps: 0.8](product-roadmap.md#restore)

**Do:** Hand over the description and pinned materials. Ask for a corrected Player back, without helping set up the editor.

**Success:** The colleague restores the workspace and builds the game independently. If something is missing, a specific file or tool is identified.

**Boundary:** A separate attempt gives only B native v2. A is freshly built on v1; B returns to v1 with compatible progress.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## G1 — player · clean Windows

[Work steps: G1 delivery](product-roadmap.md#start)

**Do:** Receive a build with a changed wall. Launch it, complete the level with sound, exit and continue.

**Success:** The new wall, expected sound and progress are present; no compiler or sources are needed.

**Boundary:** If it only ran on the development machine, mark the clean-machine check as unverified.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 0.9a — G2 author + player

[Work steps: 0.9a](product-roadmap.md#combat)

**Do:** Play one fight. While paused, change one animation transition and one enemy condition. Undo and launch the second composition.

**Success:** Combat responds to the change and its undo. Combat without experience/loot works independently.

**Boundary:** This does not accept the complete campaign.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 0.9b — G2 player

[Work steps: 0.9b](product-roadmap.md#campaign)

**Do:** Complete three locations. Change an item, die, continue after exiting and defeat the boss.

**Success:** Experience and items persist, the current fight restarts, and the playthrough has an ending.

**Boundary:** Measurement with 20 active enemies and clean delivery have separate evidence entries.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 1.0 — owner + new developers

[Work steps: 1.0](product-roadmap.md#author) · [Understandability criteria](product-roadmap.md#usability)

**Do:** Give each person their role's task and ordinary help, without naming buttons. After an error, ask them to show the way back.

**Success:** Participants work in the correct game and can explain the result; there is no data loss, rights bypass or silent update.

**Boundary:** Apply the shared threshold, role split and hint accounting below. This is the proposed tool acceptance, not its current status or the later five-game portfolio acceptance.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 1.1 — 2D level author

[Work steps: 1.1](product-roadmap.md#pixel)

**Do:** Insert a platform and replace a sprite. Launch the level and respawn at a checkpoint.

**Success:** The intended section changes in Player and jumping works; no extra combat or 3D settings are required.

**Boundary:** Check pixel scaling at the resolutions selected in advance.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 1.2 — spatial level author

[Work steps: 1.2](product-roadmap.md#flight)

**Do:** Raise a gate; try flying through at the old height, then the new height. Finish the course.

**Success:** Only the correct passage counts; there is a result and a replay.

**Boundary:** Check the exported game.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 1.3 — material author + player

[Work steps: 1.3](product-roadmap.md#real)

**Do:** Change roughness under the specified lighting. Complete the playable scene.

**Success:** The material looks as expected in motion and fits the agreed budget.

**Boundary:** The old 2D project does not require new materials.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## 1.3.x — portfolio owner

[Work steps: 1.3.x](product-roadmap.md#portfolio)

**Do:** Open the shared table of five games and check any two manually. Find the full confirmation for each of the five.

**Success:** They share one foundation version, with different compositions and games. All five have their own verified delivery and complete loop.

**Boundary:** Old pinned working projects are not updated automatically.

Result: □ independently · □ with a hint · □ unsuccessful · □ not checked

## Roles, shared tasks and recording help

Three roles are checked: a game author configures existing features, an extension developer writes new code, and a player runs a delivered game. A finite number of attempts cannot prove “any developer”. The working promise is that a developer in the declared role can use the delivery and ordinary supplied help without internal knowledge or verbal instructions from the engine's creator. Configuration does not require Rust; a new implementation requires its language and SDK.

| Participant task, without menu names | Success criterion |
| --- | --- |
| U1 · Orientation | From ordinary launch, identify the game, rights and what renders the frame; distinguish installed, used and ready features. Team has a different route; unavailable architecture is not promised. |
| U2 · Visible change | Change permitted antialiasing, explain the change, save, reopen and see the selected option in Player. |
| U3 · Error | Try an incompatible option in a learning copy; explain the reason and next action in your own words, then return to work without data loss. |
| U4 · Two games | Keep A accessible and rebuildable while changing B; reverse the configuration first, then separately a native-package update on an appropriate version. |
| U5 · New developer | Restore Full from the description, add an extension using documentation, deliver a limited setting and game; show the composition with and without the feature. |
| U6 · Recipient | Receive only the game, launch in the declared clean environment, play, exit and continue. A clean author workspace has a different procedure. |

**Proposed first-version threshold:** the owner and one independent author of the relevant role. For a prepared example, initial targets are up to five minutes to find the setting and up to ten minutes of active work for the change and return; record installation/build waiting separately. These are proposed UX targets: refine them publicly after the first observation, before the next acceptance, rather than adjusting them retrospectively. No check requires knowing the technical term FXAA in advance.

**For 1.0:** the owner plus four new developers, two game authors and two extension authors previously uninvolved in implementation. On U1–U3 in the applicable deliveries, the owner and at least three of four must succeed without observer help. Both unfamiliar extension authors and both game authors must complete their mandatory role operations. U4 uses Full, U5 the technical role, and U6 a separate recipient. Data safety, A/B isolation and Team restrictions must pass in every attempt. Investigate the same repeated obstacle even if the numerical threshold passes.

Each supplied card must identify the version/task, folder or project, two to four actions, expected change, what must stay unchanged and how to return, with links to the actual build and working example. The developer owns implementation and data; an independent checker observes the scenario, preservation and artifact identity. The owner judges the action and interface meaning. Team evidence binds HEAD, the digest including uncommitted files, delivery hashes, selected features and module versions. Technical checks and independent review for substantial changes do not replace human completion; this owner sheet does not replace those checks. See [delivery decisions](product-roadmap.md#decision).
