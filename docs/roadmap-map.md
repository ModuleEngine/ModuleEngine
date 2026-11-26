# Visual roadmap

[English](roadmap-map.md) | [Русский](ru/roadmap-map.md) | [Documentation](README.md)

The visual roadmap is an HTML page with the phase map of the [engine rebuild track](product-roadmap.md#rebuild), the current task, the next steps and rough timing. It is built automatically from [docs/product-roadmap.md](product-roadmap.md); nobody edits task statuses in the page by hand.

## Where to see it

After every merge to `main` that touches the roadmap, the [Roadmap map workflow](../.github/workflows/roadmap.yml) publishes the page to GitHub Pages: `https://myjname.github.io/ModuleEngine/`. The same workflow publishes the [campaign map](#campaign-map) next to it at `https://myjname.github.io/ModuleEngine/world/`.

One-time setup: in the repository **Settings → Pages → Build and deployment**, set **Source** to **GitHub Actions**. Until then the deploy job fails while the build and check still run.

## How to change a status

The last column of every phase task table in the roadmap is the status:

| Value in the English table | Value in the Russian table | Meaning on the map |
| --- | --- | --- |
| `Done` | `Сделано` (also `Выполнено` or `Готово`) | Task done |
| `In progress`, optionally `In progress (2–3 d)` | `В работе`, optionally `В работе (2–3 д)` | Task in progress |
| An estimate such as `2–3 d` or `1.5 w` | An estimate such as `2–3 д` or `1,5 н` | Not started |
| `Optional · 2–3 d` (before the estimate) | `Необязательно · 2–3 д` | Tracked like any task, but its phase and the checkpoints that depend on it finish without it. A phase made only of optional tasks, such as RV-D, finishes when all of them are done. Both tables must carry the prefix |
| `Paused · 2–3 w` | `На паузе · 2–3 н` | Keeps the estimate. The maps omit this task and later tasks in its phase when choosing the next move. When work starts, replace the pause marker with `In progress` |

The dedicated roadmap chat records status transitions. Implementation chats pass along accepted results and verification evidence and update technical contracts in their own pull requests. When a transition is ready, the planning chat compares `main` with the working chats and proposes one status pull request for the confirmed completions and work that has actually started. Apply the proposal from the dedicated roadmap chat and edit both language tables together. Keep the original estimate in parentheses when marking a task in progress if the estimate still applies.

The phase colour, the “Done / Now / Next” cards, the progress bar, the header label and the column headers are derived from these statuses:

- a phase is done when all its required tasks are done. Otherwise, it is current when at least one required task is done or any task is in progress;
- a phase is next when every phase it depends on is done or current;
- a checkpoint (R1+R3+R4, R2+R5) is passed when all its phases are done, including a phase whose arrow is drawn through another phase (R1 through R4);
- the header shows the task in progress, otherwise the next available task; when all tasks are done it says the track is complete, and when open tasks remain but no phase can start (for example, a dependency was removed from the plan) it says work is blocked and names what the phase waits for.

Only sections with a heading of the form `### <ID> — <title>` (for example `### R1 — ...` or `### AI-A — ...`) and task rows whose ID starts with that phase ID (`R1.3`, `AI-A.0`) are read.

## What stays hand-written

The page template [scripts/roadmap_map.html](../scripts/roadmap_map.html) holds the Russian wording: short task titles, phase descriptions, dependencies, the map layout, the timing chart and the version route. For a task that is not in the template yet, the page uses the first sentence of the task from the Russian roadmap. A new phase needs a node in the template (`phases` and `pos`) and an arrow for each of its dependencies (`add('from>to', ...)`); until the node exists the build prints a warning and the phase is not shown. A phase removed from the roadmap keeps its node but loses its tasks and is marked “not in plan”; the template's old tasks never come back.

`scripts/test_roadmap_map.py` runs the data model of both pages in Node.js and checks, on small EN/RU fixtures, that a phase removed from both Markdown versions disappears from the map and the header reports blocked work, that the 0.5 checkpoint needs R1, R3 and R4, and that every dependency listed in a phase card has an arrow. Checks against the live roadmap derive their expectations from its tables, so ordinary plan edits do not break them.

<a id="campaign-map"></a>

## Campaign map

The campaign map is the same roadmap as a game-style pixel-art map, built from the same tables. Every phase (R0–R10 and the review fixes RV-A–RV-D) is a region with its own theme, every task is a province inside it, and arrows show which phase opens which. Checkpoints and version milestones are lighthouse islands, the AI branch is a set of wizard towers.

| Task status | On the map |
| --- | --- |
| `Done` | Captured: bright colours, golden border, blue flag. A phase with all required tasks done is marked with a crown |
| `In progress` | Under siege: animated flame frame, camp and campfire |
| Not started, the first one of a phase that has been started | The next move: yellow pennant and tent. Shown only while nothing in that phase is in progress |
| Not started, anything else | Fog of war: the farther from the front, the thicker the fog |

Hover a province for its title and status; click it for the acceptance criterion and the estimate, click a region ribbon for the phase goal and the list of its provinces. Task titles and criteria come from the Russian roadmap, the card shows the full text of the task and acceptance cells; the English table only supplies statuses. A task that waits for a task of another phase (R2.3 needs R1.6; R2.5 needs RV-A, RV-B.1–RV-B.3 and RV-C.3; every R6 task needs RV-C.4) is not offered as the next move until that task is done. These dependencies are declared in `TASK_DEPS` of the template; if the waiting task is in the roadmap but the task it waits for is not, the build (including `--check`) fails and names both tasks.

Both maps put work in progress first. Within a phase, only the first unfinished required task can be the next move. If it is blocked, later tasks in the same phase wait too. When a cross-phase prerequisite is met, the waiting task of a started phase takes priority over other ready work. The campaign map can flag several started phases. Its first flag and the table headline follow the same roadmap order within each priority. A new phase comes next when its prerequisites are met. Optional tasks are offered once there is no eligible required work.

The page is a static file with no network access: a Content-Security-Policy blocks every request. All art is drawn in code on a canvas, with no external assets. Animation stops with `prefers-reduced-motion`.

What stays hand-written lives in [scripts/roadmap_world.html](../scripts/roadmap_world.html): region names and themes, the position of each region and landmark, and `STATIC` with the dependencies and goals of each phase. `STATIC` mirrors `phases` of the table page, and `scripts/test_roadmap_map.py` fails if they diverge. Provinces are laid out automatically in the order of the tasks, so a new task needs no edit in the template. A new phase needs an entry in `STATIC` and `REGS`; until it exists the build prints a warning and the phase is not shown. A phase removed from the roadmap disappears from the map together with its arrows.

### Live GitHub activity

At build time `scripts/roadmap_events.py` reads the repository's last 14 days of activity through the GitHub API and the workflow's own `GITHUB_TOKEN`, and the map shows it as objects at the place an event references. A task such as `R2.3` in a title, commit message or branch name points to its province. An AI task such as `AI-A.2` points to its phase tower. A phase such as `R2` puts the event in that region, and an event with no reference goes to the current front:

| Event | On the map |
| --- | --- |
| Commit | A small flag next to the province |
| Open pull request | A ship sailing towards the province from the nearest sea |
| Merged pull request | A ship with a golden flag in port, with sparks |
| Comment | A bird circling above the province |
| Review comment | A floating scroll |

Events older than three days are drawn dimmer. Under the map a chronicle lists the latest twelve; each entry selects its province or tower and links to GitHub. The token stays in the build: the page contains no token, no author names and no comment bodies, only the kind, time, a one-line title, the PR number and a link inside this repository, and the page makes no requests. The events file is sanitized again when the page is built, and a failed or rate-limited API call only means fewer events: the map is built without them and the build prints a warning. The workflow also runs every hour so the activity stays fresh.

## Checks and local build

```sh
python scripts/roadmap_map.py            # writes outputs/roadmap/index.html and outputs/roadmap/world/index.html
python scripts/roadmap_map.py --check    # validation only
python scripts/roadmap_events.py --repo owner/name --out events.json   # optional: GitHub activity (set GITHUB_TOKEN for a higher rate limit)
python scripts/roadmap_map.py --events events.json --repo owner/name   # build with that activity
```

The build fails when the English and Russian tables list different tasks or states, or a task has an unsupported status or estimate. Estimates use days (`d`/`д`) or weeks (`w`/`н`), with an optional range. Use a decimal point in English and a comma in Russian.

`ModuleEngine CI` uses the `Roadmap status checks` job for a nonempty change limited to existing task status cells in both exact roadmap files. It validates both tables, runs the generator tests and their Node model, checks without writing, and builds both pages. Other changes use the ordinary OS matrix and its existing route selection. `CI required` requires successful planning and all jobs required by the selected route. The separate Roadmap map workflow retains the Chromium regression and GitHub Pages publication.
