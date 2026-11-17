# Maintaining the editor UI with skills

[English](editor-ui-skills.md) | [Русский](ru/editor-ui-skills.md) | [Documentation](README.md)

These two skills support the [editor design](editor-ui-design.md) and
[technical baseline](editor-ui-spec.md). They apply to ModuleEngine's native
editor, not game HUDs or unrelated applications. They load the current repository
contracts rather than embedding a second copy of the entire design system.

For a specific window or dialog, follow the [complete surface catalogue](editor-ui-catalog.md)
linked by the shared design/spec. Its per-surface requirements and state sheets
are the review input; one anatomy sketch does not establish complete coverage.
The installed skill instructions are unchanged by this catalogue delivery.

## Skills and invocation

| Skill | Purpose | Maintained source |
| --- | --- | --- |
| `$moduleengine-ui-design` | Design a task route or implement its explicitly authorized UI slice, using the shared measurements and preservation rules. | [SKILL.md](skills/moduleengine-ui-design/SKILL.md), [Russian instructions](skills/moduleengine-ui-design/references/SKILL.ru.md) |
| `$moduleengine-ui-review` | Inspect task completion, appearance, input, permissions, preservation and the limits of actual evidence. | [SKILL.md](skills/moduleengine-ui-review/SKILL.md), [Russian instructions](skills/moduleengine-ui-review/references/SKILL.ru.md) |

Example design request:

```text
$moduleengine-ui-design Propose the Scene icon toolbar and hover/focus help.
Update the design only; do not implement the editor yet.
```

Example implementation request, only when that work is intended:

```text
$moduleengine-ui-design Implement only the agreed Play/Stop/Save icon controls.
Preserve the existing commands/shortcuts and verify behavior and help.
```

Example review request:

```text
$moduleengine-ui-review Review this UI change against its task and the baseline.
Stay read-only; distinguish confirmed bugs from missing native evidence.
```

Normal automatic skill discovery remains enabled; explicit invocation is useful
when a particular method is desired. Neither skill authorizes implementation
from a discussion or supersedes the user's current direction. A review performed
by the implementation author is a self-check; substantial changes still need
the separate reviewer required by [the review contract](review.md).

## Installation and maintenance

Repository folders under `docs/skills/` are the canonical distribution sources,
not a second automatically discovered project installation. The installed user
copies belong in `$CODEX_HOME/skills/<skill-name>/`, normally
`~/.codex/skills/<skill-name>/`. Each contains `SKILL.md`,
`agents/openai.yaml` and `references/SKILL.ru.md`. Repository paths inside the
instructions resolve from the verified ModuleEngine root, not the skill folder.
The instructions work without hard-coding this machine's drive letter.

When changing a skill, update its complete Russian counterpart and UI metadata
as needed; validate the canonical folder with skill-creator's
`scripts/quick_validate.py`. That checks structure/frontmatter, not behavior.
Forward-test meaningful changes against realistic raw requests before declaring
the method useful. Copy the reviewed files into the matching user skill folder
and compare hashes; preserve any unrelated user skill changes instead of
overwriting an unexpected installation. No runtime dependency is introduced.

Delivery evidence, including installed-file hashes and the independent review,
is kept in `outputs/reviews/editor-ui-design-20261114/`. The existing app's skill
picker refresh/discovery is not established by a filesystem check. No native
UI behavior or implementation-stage acceptance follows from installing skills.
