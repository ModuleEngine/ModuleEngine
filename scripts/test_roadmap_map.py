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


class RepositoryRoadmapTest(unittest.TestCase):
    def test_repository_roadmap_builds_without_errors(self):
        root = Path(roadmap_map.__file__).resolve().parent.parent
        page, errors, warnings = roadmap_map.build(root)
        self.assertEqual(errors, [])
        self.assertEqual(warnings, [])
        self.assertNotIn(roadmap_map.PLACEHOLDER, page)


ROOT = Path(roadmap_map.__file__).resolve().parent.parent


def without_phase(markdown, phase_id):
    """Remove one phase section (heading, table and anchor-free text up to the next section)."""
    start = markdown.index(f"### {phase_id} — ")
    rest = re.search(r"\n(?:<a id=|#{1,3} )", markdown[start + 1:])
    end = start + 1 + rest.start() if rest else len(markdown)
    return markdown[:start] + markdown[end:]


@unittest.skipUnless(shutil.which("node"), "Node.js is needed to run the page model")
class PageModelTest(unittest.TestCase):
    """Runs the data part of scripts/roadmap_map.html (between @model markers) in Node."""

    def model(self, en_text, ru_text):
        merged = roadmap_map.merge(roadmap_map.parse(en_text, "en"), roadmap_map.parse(ru_text, "ru"))
        template = (ROOT / roadmap_map.TEMPLATE_PATH).read_text(encoding="utf-8")
        page = roadmap_map.render(template, merged, "test", "now")
        code = page[page.index("// @model-begin"):page.index("// @model-end")]
        script = code + """
process.stdout.write(JSON.stringify(Object.fromEntries(Object.entries(phases).map(([k, p]) =>
  [k, { cls: p.cls, deps: p.deps, tasks: p.tasks.map(t => [t[0], t[4]]) }]))));
"""
        result = subprocess.run(["node", "-"], input=script, capture_output=True, text=True,
                                encoding="utf-8", check=True)
        return json.loads(result.stdout)

    def read(self):
        return ((ROOT / roadmap_map.EN_PATH).read_text(encoding="utf-8"),
                (ROOT / roadmap_map.RU_PATH).read_text(encoding="utf-8"))

    def test_statuses_reach_the_map(self):
        model = self.model(*self.read())
        self.assertEqual(len(model["R0"]["tasks"]), 6)
        self.assertIn(model["R0"]["cls"], {"cur", "done"})

    def test_removed_phase_tasks_do_not_return_from_template(self):
        en, ru = self.read()
        en, ru = without_phase(en, "R0"), without_phase(ru, "R0")
        self.assertNotIn("| R0.1 |", en)
        self.assertNotIn("| R0.1 |", ru)
        self.assertEqual(roadmap_map.compare(roadmap_map.parse(en, "en"), roadmap_map.parse(ru, "ru")), [])
        model = self.model(en, ru)
        self.assertEqual(model["R0"]["tasks"], [])
        self.assertEqual(model["R0"]["cls"], "later")
        leaked = [t[0] for p in model.values() for t in p["tasks"] if t[0].startswith("R0.")]
        self.assertEqual(leaked, [])
        self.assertEqual(len(model["R1"]["tasks"]), 8)

    def test_every_listed_dependency_has_an_arrow(self):
        model = self.model(*self.read())
        template = (ROOT / roadmap_map.TEMPLATE_PATH).read_text(encoding="utf-8")
        links = set(re.findall(r"add\('([^'>]+>[^']+)'", template))
        missing = [f"{dep}>{key}" for key, phase in model.items() for dep in phase["deps"]
                   if f"{dep}>{key}" not in links]
        self.assertEqual(missing, [])
        for link in ("R2>R7", "R1>R7", "R1>aiA", "R4>aiA", "R6>aiA"):
            self.assertIn(link, links)


if __name__ == "__main__":
    unittest.main()
