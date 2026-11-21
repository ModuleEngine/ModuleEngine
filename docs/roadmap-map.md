# Visual roadmap

[English](roadmap-map.md) | [Русский](ru/roadmap-map.md) | [Documentation](README.md)

The visual roadmap is an HTML page with the phase map of the [engine rebuild track](product-roadmap.md#rebuild), the current task, the next steps and rough timing. It is built automatically from [docs/product-roadmap.md](product-roadmap.md); nobody edits task statuses in the page by hand.

## Where to see it

After every merge to `main` that touches the roadmap, the [Roadmap map workflow](../.github/workflows/roadmap.yml) publishes the page to GitHub Pages: `https://myjname.github.io/ModuleEngine/`.

One-time setup: in the repository **Settings → Pages → Build and deployment**, set **Source** to **GitHub Actions**. Until then the deploy job fails while the build and check still run.

## How to change a status

The last column of every phase task table in the roadmap is the status:

| Value in the English table | Value in the Russian table | Meaning on the map |
| --- | --- | --- |
| `Done` | `Сделано` (also `Выполнено` or `Готово`) | Task done |
| `In progress`, optionally `In progress (2–3 d)` | `В работе`, optionally `В работе (2–3 д)` | Task in progress |
| Anything else, usually the estimate (`2–3 d`) | Anything else (`2–3 д`) | Not started |

Change the cell in both language versions in the same pull request that starts or completes the task. The phase colour, the “Done / Now / Next” cards, the progress bar, the header label and the column headers are derived from these statuses:

- a phase is done when all its tasks are done, and current when at least one is done or in progress;
- a phase is next when every phase it depends on is done or current;
- a checkpoint (R1+R3+R4, R2+R5) is passed when all its phases are done, including a phase whose arrow is drawn through another phase (R1 through R4);
- the header shows the task in progress, otherwise the next available task; when all tasks are done it says the track is complete, and when open tasks remain but no phase can start (for example, a dependency was removed from the plan) it says work is blocked and names what the phase waits for.

Only sections with a heading of the form `### <ID> — <title>` (for example `### R1 — ...` or `### AI-A — ...`) and task rows whose ID starts with that phase ID (`R1.3`, `AI-A.0`) are read.

## What stays hand-written

The page template [scripts/roadmap_map.html](../scripts/roadmap_map.html) holds the Russian wording: short task titles, phase descriptions, dependencies, the map layout, the timing chart and the version route. For a task that is not in the template yet, the page uses the first sentence of the task from the Russian roadmap. A new phase needs a node in the template (`phases` and `pos`) and an arrow for each of its dependencies (`add('from>to', ...)`); until the node exists the build prints a warning and the phase is not shown. A phase removed from the roadmap keeps its node but loses its tasks and is marked “not in plan”; the template's old tasks never come back.

`scripts/test_roadmap_map.py` runs the page's data model in Node.js and checks, on small EN/RU fixtures, that a phase removed from both Markdown versions disappears from the map and the header reports blocked work, that the 0.5 checkpoint needs R1, R3 and R4, and that every dependency listed in a phase card has an arrow. Checks against the live roadmap derive their expectations from its tables, so ordinary plan edits do not break them.

## Checks and local build

```sh
python scripts/roadmap_map.py            # writes outputs/roadmap/index.html
python scripts/roadmap_map.py --check    # validation only
```

The build fails when the English and Russian tables list different tasks or different statuses. Pull requests that touch the roadmap run the same check, and `scripts/test_roadmap_map.py` runs with the other script tests in CI.
