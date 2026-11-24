import contextlib
import io
import json
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import roadmap_map

EN = """
## Engine rebuild track

### R0 — Freeze and baseline · 1–2 weeks

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| R0.1 | Done 21 November 2026: the owner paused [0.5](#compose). More text. | Dated decision. | Done |
| R0.2 | Second `task`. | Runs agree. | In progress (2–3 d) |
| R0.3 | Third task | Report | 1 d |

### Cross-cutting hygiene, done inside the phases

| R0.9 | Not a task row of a phase section. | x | Done |

### AI-A — External-agent prototype · 1 week

| # | Task | Acceptance | Effort |
| --- | --- | --- | --- |
| AI-A.0 | Re-check the table. | Results recorded. | 0.5–1 d |
"""

RU = """
### R0 — Заморозка · 1–2 недели

| # | Задача | Критерий приёмки | Трудоёмкость |
| --- | --- | --- | --- |
| R0.1 | Выполнено 21 ноября 2026: пауза. | Решение. | Выполнено |
| R0.2 | Вторая задача. | Прогоны. | В работе |
| R0.3 | Третья задача | Отчёт | 1 д |

### AI-A — Прототип · 1 неделя

| # | Задача | Критерий приёмки | Трудоёмкость |
| --- | --- | --- | --- |
| AI-A.0 | Перепроверить таблицу. | Записано. | 0,5–1 д |
"""


class ParseTest(unittest.TestCase):
    def test_optional_prefix_is_parsed_and_must_match_between_languages(self):
        en = "### R1 — P · 1 week\n\n| R1.1 | T | C | Optional · 2–3 d |\n"
        ru = "### R1 — Ф · 1 неделя\n\n| R1.1 | З | К | Необязательно · 2–3 д |\n"
        task = roadmap_map.parse(en, "en")[0].tasks[0]
        self.assertEqual((task.state, task.optional), ("todo", True))
        self.assertEqual(roadmap_map.compare(roadmap_map.parse(en, "en"), roadmap_map.parse(ru, "ru")), [])
        plain_ru = ru.replace("Необязательно · ", "")
        errors = roadmap_map.compare(roadmap_map.parse(en, "en"), roadmap_map.parse(plain_ru, "ru"))
        self.assertTrue(any("optional" in e for e in errors), errors)

    def test_reads_statuses_from_last_column(self):
        phases = roadmap_map.parse(EN, "en")
        self.assertEqual([p.id for p in phases], ["R0", "AI-A"])
        self.assertEqual([(t.id, t.state) for t in phases[0].tasks],
                         [("R0.1", "done"), ("R0.2", "cur"), ("R0.3", "todo")])
        self.assertEqual(phases[0].tasks[2].effort, "1 d")
        self.assertEqual(phases[0].tasks[0].text, "The owner paused 0.5")
        self.assertEqual(phases[1].tasks[0].id, "AI-A.0")

    def test_languages_agree(self):
        self.assertEqual(roadmap_map.compare(roadmap_map.parse(EN, "en"), roadmap_map.parse(RU, "ru")), [])

    def test_reports_status_and_task_mismatch(self):
        ru = RU.replace("| В работе |", "| 2–3 д |").replace("| AI-A.0 |", "| AI-A.1 |")
        errors = roadmap_map.compare(roadmap_map.parse(EN, "en"), roadmap_map.parse(ru, "ru"))
        self.assertTrue(any("R0.2: status differs" in e for e in errors), errors)
        self.assertTrue(any(e.startswith("AI-A.0: present in docs/product-roadmap.md") for e in errors), errors)
        self.assertTrue(any(e.startswith("AI-A.1: present in docs/ru/") for e in errors), errors)

    def test_render_uses_russian_wording_and_escapes_script_end(self):
        merged = roadmap_map.merge(roadmap_map.parse(EN, "en"), roadmap_map.parse(RU, "ru"))
        merged[0].tasks[2].text = "</script>"
        page = roadmap_map.render("<script>const SRC = /*@ROADMAP_DATA@*/null;</script>", merged, "abc1234", "now")
        self.assertIn('"text":"Вторая задача"', page)
        self.assertIn('"state":"cur"', page)
        self.assertNotIn("</script>\"", page)
        with self.assertRaises(ValueError):
            roadmap_map.render("no placeholder", merged, "", "")


ROOT = Path(roadmap_map.__file__).resolve().parent.parent
WORDS = {"en": {"done": "Done", "cur": "In progress", "todo": "1 d", "opt": "Optional · 1 d"},
         "ru": {"done": "Сделано", "cur": "В работе", "todo": "1 д", "opt": "Необязательно · 1 д"}}
STATE_NAMES = {"done": "Готово", "cur": "В работе", "todo": "Впереди"}


def roadmap(spec, lang):
    """Small roadmap fixture: {"R0": ["done", "cur"], ...} -> Markdown with one table per phase."""
    lines = []
    for phase, states in spec.items():
        lines += [f"### {phase} — Phase {phase} · 1 week", "", "| # | Task | Acceptance | Effort |", "| --- | --- | --- | --- |"]
        lines += [f"| {phase}.{i} | Task {phase}.{i}. | Check {phase}.{i}. | {WORDS[lang][state]} |"
                  for i, state in enumerate(states, 1)]
        lines.append("")
    return "\n".join(lines)


def review_track(**overrides):
    """R0-R2 with the review fixes: R2.5 waits for RV-A, RV-B.1-RV-B.3 and RV-C.3 (TASK_DEPS of the campaign map).
    R2 comes first so that the next move cannot come from the order of the sections alone."""
    spec = {"R0": ["done"], "R2": ["done"] * 4 + ["todo"], "R1": ["done", "opt"],
            "RV-A": ["todo"] * 6, "RV-B": ["todo"] * 4, "RV-C": ["todo"] * 4, "RV-D": ["opt"] * 2}
    spec.update(overrides)
    return spec


def review_done_except(*open_tasks):
    """RV-A, RV-B and RV-C done except the named tasks, which stay open."""
    spec = {}
    for phase, n in (("RV-A", 6), ("RV-B", 4), ("RV-C", 4)):
        spec[phase] = ["todo" if f"{phase}.{i}" in open_tasks else "done" for i in range(1, n + 1)]
    return review_track(**spec)


def without_phase(markdown, phase_id):
    """Remove one phase section: its heading and everything up to the next anchor or heading."""
    heading = re.search(rf"^### {re.escape(phase_id)} — .*$", markdown, re.M)
    rest = re.search(r"\n(?:<a id=|#{1,3} )", markdown[heading.end():])
    end = heading.end() + rest.start() if rest else len(markdown)
    return markdown[:heading.start()] + markdown[end:]


def live():
    return ((ROOT / roadmap_map.EN_PATH).read_text(encoding="utf-8"),
            (ROOT / roadmap_map.RU_PATH).read_text(encoding="utf-8"))


class LiveRoadmapTest(unittest.TestCase):
    def test_repository_roadmap_builds_without_errors(self):
        page, errors, _warnings = roadmap_map.build(ROOT)
        self.assertEqual(errors, [])
        self.assertNotIn(roadmap_map.PLACEHOLDER, page)


@unittest.skipUnless(shutil.which("node"), "Node.js is needed to run the page model")
class PageModelTest(unittest.TestCase):
    """Runs the data part of scripts/roadmap_map.html (between @model markers) in Node."""

    def model(self, en_text, ru_text):
        merged = roadmap_map.merge(roadmap_map.parse(en_text, "en"), roadmap_map.parse(ru_text, "ru"))
        template = (ROOT / roadmap_map.TEMPLATE_PATH).read_text(encoding="utf-8")
        deps = roadmap_map.task_deps(WORLD_TEMPLATE.read_text(encoding="utf-8"))
        page = roadmap_map.render(template, merged, "test", "now", deps)
        code = page[page.index("// @model-begin"):page.index("// @model-end")]
        script = code + """
process.stdout.write(JSON.stringify({
  phases: Object.fromEntries(Object.entries(phases).map(([k, p]) =>
    [k, { id: p.id, cls: p.cls, deps: p.deps, shownVia: p.shownVia || {}, tasks: p.tasks.map(t => [t[0], t[4]]) }])),
  summary: { kind: summary.kind, task: summary.item ? summary.item.t[0] : null, phase: summary.phase || null,
             waits: summary.waits || [], remaining: summary.remaining }
}));
"""
        result = subprocess.run(["node", "-"], input=script, capture_output=True, text=True,
                                encoding="utf-8", check=True)
        return json.loads(result.stdout)

    def fixture(self, spec):
        return self.model(roadmap(spec, "en"), roadmap(spec, "ru"))

    # Fixture regressions

    def test_task_in_progress_is_the_headline(self):
        m = self.fixture({"R0": ["done", "cur"], "R1": ["todo"]})
        self.assertEqual(m["phases"]["R0"]["cls"], "cur")
        self.assertEqual(m["phases"]["R1"]["cls"], "next")
        self.assertEqual(m["summary"]["kind"], "cur")
        self.assertEqual(m["summary"]["task"], "R0.2")

    def test_next_available_task_when_nothing_in_progress(self):
        m = self.fixture({"R0": ["done", "done"], "R1": ["todo", "todo"]})
        self.assertEqual((m["summary"]["kind"], m["summary"]["task"]), ("next", "R1.1"))

    def test_all_tasks_done(self):
        m = self.fixture({"R0": ["done"], "R1": ["done"], "R4": ["done"]})
        self.assertEqual(m["summary"], {"kind": "done", "task": None, "phase": None, "waits": [], "remaining": 0})

    def test_removed_phase_tasks_do_not_return_and_work_is_blocked(self):
        full = {"R0": ["done", "cur"], "R1": ["todo", "todo", "todo"], "R4": ["todo"]}
        en, ru = roadmap(full, "en"), roadmap(full, "ru")
        en, ru = without_phase(en, "R0"), without_phase(ru, "R0")
        self.assertNotIn("| R0.", en + ru)
        m = self.model(en, ru)
        self.assertEqual(m["phases"]["R0"]["tasks"], [])
        self.assertEqual(m["phases"]["R0"]["cls"], "later")
        self.assertEqual([t for p in m["phases"].values() for t in p["tasks"] if t[0].startswith("R0.")], [])
        self.assertEqual(len(m["phases"]["R1"]["tasks"]), 3)
        self.assertEqual(m["summary"]["kind"], "blocked")
        self.assertEqual((m["summary"]["phase"], m["summary"]["waits"], m["summary"]["remaining"]), ("R1", ["R0"], 4))

    def test_checkpoint_needs_all_of_r1_r3_r4(self):
        base = {"R0": ["done"], "R2": ["done"]}
        for open_phase in ("R1", "R3", "R4"):
            spec = dict(base, R1=["done", "done"], R3=["done"], R4=["done"])
            spec[open_phase] = ["done", "todo"]
            with self.subTest(open_phase=open_phase):
                self.assertEqual(self.fixture(spec)["phases"]["g05"]["cls"], "gate")
        passed = self.fixture(dict(base, R1=["done", "done"], R3=["done"], R4=["done"]))
        self.assertEqual(passed["phases"]["g05"]["cls"], "done")

    def test_open_optional_task_does_not_block_phase_or_checkpoint(self):
        spec = {"R0": ["done"], "R2": ["done"], "R1": ["done", "done", "opt"], "R3": ["done"], "R4": ["done"]}
        m = self.fixture(spec)
        self.assertEqual(m["phases"]["R1"]["cls"], "done")
        self.assertEqual(m["phases"]["g05"]["cls"], "done")
        self.assertIn(["R1.3", "Впереди"], [list(t) for t in m["phases"]["R1"]["tasks"]])

    def test_review_fixes_come_before_r25_and_r25_before_optional_work(self):
        self.assertEqual(self.fixture(review_track())["summary"]["task"], "RV-A.1")
        self.assertEqual(self.fixture(review_done_except())["summary"]["task"], "R2.5")

    def test_open_rv_c3_keeps_r25_out_of_the_headline(self):
        m = self.fixture(review_done_except("RV-C.3"))
        self.assertEqual(m["summary"]["task"], "RV-C.3")

    def test_phase_of_only_optional_tasks_is_done_only_when_they_are(self):
        open_ = self.fixture({"R0": ["done"], "RV-A": ["done"], "RV-D": ["opt", "opt"]})
        self.assertEqual(open_["phases"]["RV-D"]["cls"], "next")
        done = self.fixture({"R0": ["done"], "RV-A": ["done"], "RV-D": ["done", "done"]})
        self.assertEqual(done["phases"]["RV-D"]["cls"], "done")

    def test_every_listed_dependency_has_an_arrow(self):
        m = self.fixture({"R0": ["cur"]})["phases"]
        template = (ROOT / roadmap_map.TEMPLATE_PATH).read_text(encoding="utf-8")
        links = set(re.findall(r"add\('([^'>]+>[^']+)'", template))
        missing = []
        for key, phase in m.items():
            for dep in phase["deps"]:
                via = phase["shownVia"].get(dep)
                ok = f"{dep}>{key}" in links or (via and f"{via}>{key}" in links and dep in m[via]["deps"])
                if not ok:
                    missing.append(f"{dep}>{key}")
        self.assertEqual(missing, [])
        for link in ("R2>R7", "R1>R7", "R1>aiA", "R4>aiA", "R6>aiA"):
            self.assertIn(link, links)

    # Live documents: expectations come from the tables themselves

    def test_live_statuses_reach_the_map(self):
        en, ru = live()
        parsed = roadmap_map.parse(en, "en")
        m = self.model(en, ru)["phases"]
        by_id = {p["id"]: p for p in m.values()}
        for phase in parsed:
            if phase.id in by_id:
                self.assertEqual(by_id[phase.id]["tasks"], [[t.id, STATE_NAMES[t.state]] for t in phase.tasks], phase.id)

    def test_live_removed_phase_disappears(self):
        en, ru = live()
        parsed = roadmap_map.parse(en, "en")
        by_id = {p["id"]: k for k, p in self.model(en, ru)["phases"].items()}
        placed = [p for p in parsed if p.id in by_id]
        if not placed:
            self.skipTest("no phase of the live roadmap is on the map")
        target = placed[0]
        en2, ru2 = without_phase(en, target.id), without_phase(ru, target.id)
        self.assertEqual(roadmap_map.compare(roadmap_map.parse(en2, "en"), roadmap_map.parse(ru2, "ru")), [])
        m = self.model(en2, ru2)
        self.assertEqual(m["phases"][by_id[target.id]]["tasks"], [])
        prefix = target.id + "."
        self.assertEqual([t for p in m["phases"].values() for t in p["tasks"] if t[0].startswith(prefix)], [])
        remaining = sum(t.state != "done" for p in parsed if p.id != target.id and p.id in by_id for t in p.tasks)
        self.assertEqual(m["summary"]["remaining"], remaining)
        if remaining:
            self.assertNotEqual(m["summary"]["kind"], "done")


WORLD_TEMPLATE = ROOT / roadmap_map.WORLD_TEMPLATE_PATH


def drop_row(markdown, task_id):
    """Remove one task row from a roadmap table."""
    kept = [line for line in markdown.splitlines() if not line.lstrip().startswith(f"| {task_id} |")]
    assert len(kept) == len(markdown.splitlines()) - 1, task_id
    return "\n".join(kept) + "\n"


class WorldDependencyTest(unittest.TestCase):
    """R2.3 waits for R1.6: removing only R1.6 must fail the build instead of unlocking R2.3."""

    def tree(self, tmp, en, ru):
        for rel, text in ((roadmap_map.EN_PATH, en), (roadmap_map.RU_PATH, ru),
                          (roadmap_map.TEMPLATE_PATH, (ROOT / roadmap_map.TEMPLATE_PATH).read_text(encoding="utf-8")),
                          (roadmap_map.WORLD_TEMPLATE_PATH, WORLD_TEMPLATE.read_text(encoding="utf-8"))):
            path = Path(tmp) / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        return Path(tmp)

    def test_declared_dependencies_are_read_from_the_template(self):
        deps = roadmap_map.task_deps(WORLD_TEMPLATE.read_text(encoding="utf-8"))
        self.assertEqual(deps.get("R2.3"), ["R1.6"])

    def test_removing_only_the_required_task_is_an_error_naming_both(self):
        en, ru = live()
        with tempfile.TemporaryDirectory() as tmp:
            root = self.tree(tmp, drop_row(en, "R1.6"), drop_row(ru, "R1.6"))
            _page, errors, _warnings = roadmap_map.build_world(root)
            self.assertEqual(len(errors), 2, errors)
            for error in errors:
                self.assertIn("R2.3", error)
                self.assertIn("R1.6", error)
            self.assertTrue(any(roadmap_map.EN_PATH in e for e in errors))
            self.assertTrue(any(roadmap_map.RU_PATH in e for e in errors))
            with mock.patch.object(roadmap_map, "ROOT", root), contextlib.redirect_stderr(io.StringIO()) as err:
                self.assertEqual(roadmap_map.main(["--check"]), 1)
                self.assertNotEqual(roadmap_map.main(["--out", str(Path(tmp) / "out" / "index.html")]), 0)
            self.assertIn("R2.3 waits for R1.6", err.getvalue())

    def test_removing_the_waiting_task_or_both_is_fine(self):
        en, ru = live()
        with tempfile.TemporaryDirectory() as tmp:
            for tasks in (["R2.3"], ["R1.6", "R2.3"]):
                e, r = en, ru
                for task in tasks:
                    e, r = drop_row(e, task), drop_row(r, task)
                _page, errors, _warnings = roadmap_map.build_world(self.tree(tmp, e, r))
                self.assertEqual(errors, [], tasks)

    def test_present_required_task_is_not_an_error(self):
        en, ru = live()
        with tempfile.TemporaryDirectory() as tmp:
            _page, errors, _warnings = roadmap_map.build_world(self.tree(tmp, en, ru))
            self.assertEqual(errors, [])


class WorldBuildTest(unittest.TestCase):
    def test_pages_link_to_each_other_by_file_name(self):
        """A link to a directory does not open its index.html from file://."""
        table = (ROOT / roadmap_map.TEMPLATE_PATH).read_text(encoding="utf-8")
        world = WORLD_TEMPLATE.read_text(encoding="utf-8")
        self.assertIn('href="world/index.html"', table)
        self.assertIn('href="../index.html"', world)

    def test_reduced_motion_stops_css_animation(self):
        world = WORLD_TEMPLATE.read_text(encoding="utf-8")
        block = re.search(r"@media \(prefers-reduced-motion: reduce\)\{([^}]*)\}", world)
        self.assertIsNotNone(block)
        self.assertIn(".chip.cur", block.group(1))
        self.assertIn(".ribbon.hot", block.group(1))
        self.assertIn("animation:none", block.group(1))
        # an equal-specificity rule written later would override it
        for selector in (r"\.ribbon\.hot", r"\.chip\.cur"):
            for rule in re.finditer(rf"^\s*{selector}\{{", world, re.M):
                self.assertLess(rule.start(), block.start(), selector)

    def test_tooltip_escapes_markdown_text(self):
        world = WORLD_TEMPLATE.read_text(encoding="utf-8")
        line = next(l for l in world.splitlines() if "function showTip" in l)
        self.assertIn("esc(p.title)", line)
        self.assertNotIn("${p.title}", line)

    def test_live_roadmap_builds_the_campaign_map(self):
        page, errors, warnings = roadmap_map.build_world(ROOT)
        self.assertEqual((errors, warnings), ([], []))
        self.assertNotIn(roadmap_map.PLACEHOLDER, page)
        self.assertIn('"id":"R0"', page)

    def test_template_has_one_placeholder(self):
        self.assertEqual(WORLD_TEMPLATE.read_text(encoding="utf-8").count(roadmap_map.PLACEHOLDER), 1)

    def test_phase_without_region_is_reported(self):
        template = WORLD_TEMPLATE.read_text(encoding="utf-8")
        phases = roadmap_map.parse(roadmap({"R0": ["todo"], "R42": ["todo"]}, "en"), "en")
        self.assertEqual(roadmap_map.unplaced(template, phases), ["R42"])


@unittest.skipUnless(shutil.which("node"), "Node.js is needed to run the page model")
class WorldModelTest(unittest.TestCase):
    """Runs the data part of scripts/roadmap_world.html (between @model markers) in Node."""

    def model(self, en_text, ru_text):
        merged = roadmap_map.merge(roadmap_map.parse(en_text, "en"), roadmap_map.parse(ru_text, "ru"))
        template = WORLD_TEMPLATE.read_text(encoding="utf-8")
        page = roadmap_map.render(template, merged, "test", "now")
        code = page[page.index("// @model-begin"):page.index("// @model-end")]
        script = code + """
process.stdout.write(JSON.stringify({
  provs: provs.map(p => [p.id, p.st]), crit: Object.fromEntries(provs.map(p => [p.id, [p.crit, p.detail || '']])), taskDeps: TASK_DEPS, regions: regs.filter(r => !r.landmark).map(r => r.key),
  focus: focusIdx >= 0 ? provs[focusIdx].id : null, arrows: arrows.map(a => a.a.key + '>' + a.b.key),
  landmarks: Object.fromEntries(provs.filter(p => p.kind !== 'task').map(p => [p.id, p.st]))
}));
"""
        result = subprocess.run(["node", "-"], input=script, capture_output=True, text=True,
                                encoding="utf-8", check=True)
        return json.loads(result.stdout)

    def fixture(self, spec):
        return self.model(roadmap(spec, "en"), roadmap(spec, "ru"))

    def test_every_task_becomes_a_province_with_its_status(self):
        m = self.fixture({"R0": ["done", "cur"], "R1": ["todo", "todo"]})
        tasks = [p for p in m["provs"] if re.fullmatch(r"R\d+\.\d+", p[0])]
        self.assertEqual(tasks, [["R0.1", "done"], ["R0.2", "cur"], ["R1.1", "next"], ["R1.2", "next"]])
        self.assertEqual(m["focus"], "R0.2")

    def test_next_move_is_the_first_open_task_of_a_started_region(self):
        m = self.fixture({"R0": ["done", "done"], "R1": ["done", "todo", "todo"], "R2": ["todo"]})
        states = dict(m["provs"])
        self.assertEqual((states["R1.2"], states["R1.3"], states["R2.1"]), ("front", "next", "next"))
        self.assertEqual(m["focus"], "R1.2")

    def test_first_open_task_is_the_next_move_at_the_very_start(self):
        m = self.fixture({"R0": ["todo", "todo"]})
        self.assertEqual(dict(m["provs"])["R0.1"], "front")

    def test_task_waiting_for_another_phase_is_not_the_next_move(self):
        """R2.3 also needs R1.6: the mark moves to R2.3 only after R1.6 is done."""
        before = self.fixture({"R0": ["done"], "R1": ["done"] * 5 + ["todo", "todo"], "R2": ["done", "done", "todo", "todo"]})
        states = dict(before["provs"])
        self.assertEqual((states["R1.6"], states["R2.3"]), ("front", "next"))
        after = self.fixture({"R0": ["done"], "R1": ["done"] * 6 + ["todo"], "R2": ["done", "done", "todo", "todo"]})
        states = dict(after["provs"])
        self.assertEqual((states["R1.7"], states["R2.3"]), ("front", "front"))

    def test_review_fixes_are_the_next_move_not_an_optional_task(self):
        m = self.fixture(review_track())
        states = dict(m["provs"])
        self.assertEqual((m["focus"], states["R1.2"], states["R2.5"]), ("RV-A.1", "next", "next"))
        m = self.fixture(review_done_except())
        self.assertEqual((m["focus"], dict(m["provs"])["RV-D.1"]), ("R2.5", "next"))

    def test_r25_waits_for_rv_c3_but_not_for_rv_b4(self):
        self.assertEqual(dict(self.fixture(review_done_except("RV-C.3"))["provs"])["R2.5"], "next")
        self.assertEqual(dict(self.fixture(review_done_except("RV-B.4"))["provs"])["R2.5"], "front")

    def test_missing_required_task_never_unlocks_the_waiting_one(self):
        """Safety net behind the build error: the model keeps R2.3 blocked while R1.6 is absent."""
        m = self.fixture({"R0": ["done"], "R1": ["done"] * 5, "R2": ["done", "done", "todo", "todo"]})
        self.assertEqual(dict(m["provs"])["R2.3"], "next")

    def test_task_dependencies_come_from_the_roadmap(self):
        en, _ru = live()
        model = self.fixture({"R0": ["todo"]})
        self.assertTrue(model["taskDeps"])
        for task, deps in model["taskDeps"].items():
            phase = task.split(".")[0]
            # "R2.3 also on R1.6" names the task; "R2.5 also on RV-A and RV-B" names whole phases
            # "all of R6 also on RV-C.4" covers every task of the phase
            found = re.search(rf"^### {re.escape(phase)} — .*(?:{re.escape(task)}|{re.escape(phase)}) also on ([^;]+)", en, re.M)
            self.assertTrue(found, f"{task} has no 'also on' note in its phase heading")
            for dep in deps:
                self.assertTrue(re.search(rf"(?<![\w.-]){re.escape(dep)}(?![\w-])|(?<![\w.-]){re.escape(dep.rsplit('.', 1)[0])}(?![\w.-])", found[1]),
                                f"{task} needs {dep}")

    def test_card_gets_the_full_acceptance_text(self):
        criteria = "Textures: 1 000 unique ones; no floor flicker on S4. Custom passes keep working."
        md = {"en": "### R0 — Phase · 1 week\n\n| # | Task | Acceptance | Effort |\n| --- | --- | --- | --- |\n"
                    f"| R0.1 | Path `outputs/bench/<date>/` first. Then more. | {criteria} | 1 d |\n",
              "ru": "### R0 — Фаза · 1 неделя\n\n| # | Задача | Критерий | Трудоёмкость |\n| --- | --- | --- | --- |\n"
                    f"| R0.1 | Путь `outputs/bench/<date>/` сначала. Затем ещё. | {criteria} | 1 д |\n"}
        m = self.model(md["en"], md["ru"])
        self.assertEqual(m["crit"]["R0.1"][0], criteria)
        self.assertEqual(m["crit"]["R0.1"][1], "Путь outputs/bench/<date>/ сначала. Затем ещё.")

    def test_removed_phase_leaves_the_map(self):
        spec = {"R0": ["done"], "R1": ["todo"], "R4": ["todo"]}
        en, ru = without_phase(roadmap(spec, "en"), "R1"), without_phase(roadmap(spec, "ru"), "R1")
        m = self.model(en, ru)
        self.assertNotIn("R1", m["regions"])
        self.assertEqual([p for p in m["provs"] if p[0].startswith("R1.")], [])
        self.assertNotIn("R1>R4", m["arrows"])

    def test_checkpoint_is_passed_when_its_phases_are_done(self):
        base = {"R0": ["done"], "R2": ["done"], "R1": ["done"], "R3": ["done"]}
        self.assertEqual(self.fixture(dict(base, R4=["todo"]))["landmarks"]["R1+R3+R4"], "next")
        self.assertEqual(self.fixture(dict(base, R4=["done"]))["landmarks"]["R1+R3+R4"], "done")

    def test_live_statuses_reach_the_campaign_map(self):
        en, ru = live()
        m = self.model(en, ru)
        shown = dict(m["provs"])
        for phase in roadmap_map.parse(en, "en"):
            for task in phase.tasks:
                if task.id in shown and task.state == "done":
                    self.assertEqual(shown[task.id], "done", task.id)
                elif task.id in shown:
                    self.assertNotEqual(shown[task.id], "done", task.id)

    def test_dependencies_agree_with_the_table_page(self):
        """Region metadata of the campaign map mirrors `phases` of the table page."""
        table = (ROOT / roadmap_map.TEMPLATE_PATH).read_text(encoding="utf-8")
        world = WORLD_TEMPLATE.read_text(encoding="utf-8")
        script = ("const D = 'Готово', C = 'В работе', N = 'Впереди';\n"
                  + table[table.index("const phases = {"):table.index("// ---- Statuses")]
                  + "\n" + world[world.index("const STATIC = {"):world.index("const ST_OF")]
                  + "\nprocess.stdout.write(JSON.stringify({ table: Object.fromEntries(Object.entries(phases).map(([k, p]) => [k, [p.id, p.deps, p.unlocks]])),"
                  + " world: Object.fromEntries(Object.entries(STATIC).map(([k, p]) => [k, [p.id, p.deps, p.unlocks]])) }));")
        out = json.loads(subprocess.run(["node", "-"], input=script, capture_output=True, text=True,
                                        encoding="utf-8", check=True).stdout)
        self.assertEqual(out["world"], out["table"])


if __name__ == "__main__":
    unittest.main()
