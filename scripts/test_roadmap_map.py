import json
import re
import shutil
import subprocess
import unittest
from pathlib import Path

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
WORDS = {"en": {"done": "Done", "cur": "In progress", "todo": "1 d"},
         "ru": {"done": "Сделано", "cur": "В работе", "todo": "1 д"}}
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
        page = roadmap_map.render(template, merged, "test", "now")
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


if __name__ == "__main__":
    unittest.main()
