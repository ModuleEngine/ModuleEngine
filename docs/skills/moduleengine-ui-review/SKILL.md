---
name: moduleengine-ui-review
description: "Review planned or implemented ModuleEngine native editor UI for task clarity, scope/rights, icons/help, geometry, input conflicts, responsive layout and evidence. Use for UI design reviews and acceptance checks, not arbitrary code audits, game HUD/art or a claim of independence when reviewing your own work."
---

# Review ModuleEngine editor UI

[English](SKILL.md) | [Русский](references/SKILL.ru.md)

Perform a bounded, evidence-based review. This skill supplies a method; it does
not make an implementation author an independent reviewer. Do not mutate code,
authored data, layouts or files during a read-only review. If the user also
authorizes fixes, keep findings and fixes explicit; an assigned independent
reviewer reports findings to the author rather than writing the implementation.

## Load the task and evidence

Locate the current ModuleEngine repository, not an unrelated historical copy.
Paths below are repository-relative. Read AGENTS.md, docs/current-status.md,
the actual user goal and affected contract. Read docs/editor-ui-design.md,
relevant docs/editor-ui-spec.md sections and docs/editor-ui-tokens.json. Check
docs/editor-clarity-plan.md before treating a proposal as shipped behavior.
If sources or an artifact are missing, state the resulting evidence limit.

Establish initial versus task diff, new files, target/features, source snapshot,
artifact identity and checks actually performed. For independent review follow
docs/review.md, including a clean reviewer context and source-digest binding.
Existing dirty work stays visible and must not be attributed to the new change.

## Inspect task completion, not just appearance

Trace the intended route from its entry point to the real command and result.
Check project/workspace/graph scope identity, technical/gameplay/project owner,
Full/Team/source rights and unknown state evidence. Workspace names cannot grant
permissions. An absent optional capability cannot initialize implicitly or
leave misleading relevant-looking settings.

Inspect standard icon meaning, persistent selected/checked states, actual
shortcuts, semantic accessibility names and hover/keyboard/disabled help.
Critical information cannot depend only on hovering. Check opaque overlay
surfaces and all actual text/icon/focus contrasts against the token benchmarks,
not just a screenshot's sampled colour or one successful hex pair.

Check narrow/short panes, long names, 100/150/200% and OS work-area clamping.
Inspect graph click-versus-drag, MMB/RMB panning, wheel zoom, Alt+LMB wire delete,
text focus, divider/overlay priority, mixed selection/Delete/Undo and wrong-scope
actions. Inspect Scene overlay capture/Cancel, one gesture/one Undo, Assets
folder/address identity and existing reference/file-transaction safeguards.
Do not infer these behaviors from a static mockup.

Check dirty/saved/applied/working/error distinctions, real progress/cancellation,
last-good retention, authored/Play separation, Save/reopen and personal-layout
preservation where affected. New native windows need real OS/focus/close/return/
shared-history evidence; an egui floating panel is not equivalent. Pure selection,
layout navigation, graph pan/zoom must not compile or rebuild. Compare matched
timing workloads for performance claims; a screenshot or passing CPU test is
not Player/render/native acceptance.

Choose only relevant checks. Use moduleengine-debug for observed regressions
and a minimal reproduction. Coordinate native/GPU runs, use owned fixtures,
and apply Computer Use/session-hygiene skills if a brief real UI check is needed.
Do not install tools, touch real user data or keep a Computer Use session open
merely to finish a checklist.

## Report findings and scope honestly

Each confirmed finding names file/line or visible control, trigger, consequence,
violated requirement, priority and evidence. Separate product defects, fixture/
environment failures, design recommendations and missing evidence. Mark a
proposal disagreement as such; do not invent a failure from personal taste.
If none are found, say no confirmed findings within the reviewed scope, not
that all errors are absent.

List inspected artifacts, own checks versus author logs, unverified conditions
and whether behavior or only design can be accepted. EN/RU meaning, links and
tokens must agree. After source/criteria fixes, recheck the affected scope and
rebind the conclusion to HEAD and the new source digest. Never lower existing
acceptance budgets or relabel historical evidence to make review pass.
