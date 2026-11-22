import datetime as dt
import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
import urllib.error
from pathlib import Path

import roadmap_events
import roadmap_map

REPO = "owner/name"
NOW = dt.datetime(2026, 11, 22, 12, 0, 0, tzinfo=dt.timezone.utc)
TASKS = {"R1.6", "R2.3", "R10.1"}
PHASES = {"R1", "R2", "R10"}
WORLD_TEMPLATE = Path(__file__).resolve().parent / "roadmap_world.html"


def stamp(hours_ago: float) -> str:
    return (NOW - dt.timedelta(hours=hours_ago)).strftime("%Y-%m-%dT%H:%M:%SZ")


def fake_api(**routes):
    """A getter that answers by the first path fragment that matches; anything else raises like a dead network."""
    def get(url: str):
        for fragment, answer in routes.items():
            if f"/{fragment.replace('_', '/')}?" in url:
                if isinstance(answer, Exception):
                    raise answer
                return answer
        return []
    return get


class FindRefTest(unittest.TestCase):
    def ref(self, title, extra="", tasks=TASKS, phases=PHASES):
        return roadmap_events.find_ref(title, extra, tasks, phases)

    def test_finds_only_known_task_ids(self):
        self.assertEqual(self.ref("Finish R2.3 (draft)"), ("R2.3", "R2"))
        self.assertEqual(self.ref("v1.6 and R9.9 are not tasks"), (None, None))

    def test_task_id_is_not_a_prefix_match(self):
        self.assertEqual(self.ref("R10.1 landed"), ("R10.1", "R10"))
        self.assertEqual(self.ref("R1.65 is something else"), (None, None))
        self.assertEqual(self.ref("see R1.6."), ("R1.6", "R1"))

    def test_phase_mention_falls_back_to_the_phase(self):
        self.assertEqual(self.ref("R2: render groundwork"), (None, "R2"))
        self.assertEqual(self.ref("R20 is unrelated"), (None, None))

    def test_the_first_task_in_the_title_wins(self):
        self.assertEqual(self.ref("Record R2.3 and then R1.6"), ("R2.3", "R2"))
        self.assertEqual(self.ref("Record R1.6 and then R2.3"), ("R1.6", "R1"))

    def test_the_title_beats_the_branch_and_the_branch_is_the_fallback(self):
        self.assertEqual(self.ref("Finish R1.6", "claude/r2.3-work R2.3"), ("R1.6", "R1"))
        self.assertEqual(self.ref("Finish things", "claude/R2.3-work"), ("R2.3", "R2"))
        self.assertEqual(self.ref("Work on R2", "R1.6"), ("R1.6", "R1"))

    def test_pr_35_title_is_stable_whatever_the_set_order(self):
        """Same-length ids once came out in set order: R1.6, R1.7 or R2.3 depending on PYTHONHASHSEED."""
        title = "Record R1.6 completion and the starts of R1.7 and R2.3"
        roadmap = {f"R{p}.{n}" for p in range(0, 11) for n in range(1, 9)}
        for seed in range(25):
            ids = set(sorted(roadmap, key=lambda i: hash((seed, i))))
            self.assertEqual(self.ref(title, "TechArt/roadmap-r16-r17-r23", ids, {i.split(".")[0] for i in ids}), ("R1.6", "R1"))

    def test_same_answer_under_different_hash_seeds(self):
        code = ("import sys; sys.path.insert(0, %r); import roadmap_events, roadmap_map; "
                "en = roadmap_map.parse(open(%r, encoding='utf-8').read(), 'en'); "
                "t = {x.id for p in en for x in p.tasks}; print(roadmap_events.find_ref("
                "'Record R1.6 completion and the starts of R1.7 and R2.3', '', t, {p.id for p in en}))"
                % (str(Path(__file__).resolve().parent), str(roadmap_map.ROOT / roadmap_map.EN_PATH)))
        answers = {subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, check=True,
                                  env={"PYTHONHASHSEED": str(seed), "PATH": ""}).stdout for seed in range(6)}
        self.assertEqual(answers, {"('R1.6', 'R1')\n"})


class CollectTest(unittest.TestCase):
    def collect(self, **routes):
        return roadmap_events.collect(REPO, TASKS, PHASES, NOW, fake_api(**routes))

    def test_every_event_kind_is_collected_without_authors_or_bodies(self):
        events, warnings = self.collect(
            pulls=[{"number": 7, "title": "R2.3 bindless textures", "created_at": stamp(30), "merged_at": stamp(2),
                    "html_url": f"https://github.com/{REPO}/pull/7", "head": {"ref": "claude/x"},
                    "user": {"login": "someone-private"}}],
            commits=[{"html_url": f"https://github.com/{REPO}/commit/abc", "author": {"login": "someone-private"},
                      "commit": {"message": "Finish R1.6 contracts\n\nlong secret body", "committer": {"date": stamp(5)}}}],
            issues_comments=[{"issue_url": f"https://api.github.com/repos/{REPO}/issues/7", "created_at": stamp(1),
                              "html_url": f"https://github.com/{REPO}/pull/7#c1", "body": "top secret comment text"}],
            pulls_comments=[{"pull_request_url": f"https://api.github.com/repos/{REPO}/pulls/7", "created_at": stamp(3),
                             "html_url": f"https://github.com/{REPO}/pull/7#r1", "body": "inline remark"}])
        self.assertEqual(warnings, [])
        self.assertEqual({e["type"] for e in events}, {"pr", "merge", "commit", "comment", "review"})
        text = json.dumps(events)
        for private in ("someone-private", "top secret", "long secret body", "inline remark"):
            self.assertNotIn(private, text)
        by_type = {e["type"]: e for e in events}
        self.assertEqual(by_type["commit"]["task"], "R1.6")
        self.assertEqual(by_type["commit"]["title"], "Finish R1.6 contracts")
        self.assertEqual((by_type["merge"]["task"], by_type["merge"]["num"]), ("R2.3", 7))
        self.assertEqual(by_type["comment"]["task"], "R2.3")  # the PR title tells which task it is about
        self.assertEqual([e["at"] for e in events], sorted((e["at"] for e in events), reverse=True))

    def test_comments_fall_back_to_the_task_their_pr_resolves_to_from_its_branch(self):
        """PR "Graphics work" on branch TechArt/R2.3-gpu-scene: "Looks good" on it belongs to R2.3, both comment kinds."""
        pull = {"number": 9, "title": "Graphics work", "created_at": stamp(40), "html_url": f"https://github.com/{REPO}/pull/9",
                "head": {"ref": "TechArt/R2.3-gpu-scene"}}
        events, _ = self.collect(
            pulls=[pull],
            issues_comments=[{"issue_url": f"https://api.github.com/repos/{REPO}/issues/9", "created_at": stamp(2),
                              "html_url": f"https://github.com/{REPO}/pull/9#c", "body": "Looks good"}],
            pulls_comments=[{"pull_request_url": f"https://api.github.com/repos/{REPO}/pulls/9", "created_at": stamp(3),
                             "html_url": f"https://github.com/{REPO}/pull/9#r", "body": "Looks good"}])
        by_type = {e["type"]: e for e in events}
        self.assertEqual({k: (v["task"], v["phase"]) for k, v in by_type.items()},
                         {"pr": ("R2.3", "R2"), "comment": ("R2.3", "R2"), "review": ("R2.3", "R2")})

    def test_a_comment_that_names_its_own_task_keeps_it(self):
        pull = {"number": 9, "title": "Graphics work", "created_at": stamp(40), "head": {"ref": "TechArt/R2.3-gpu-scene"}}
        events, _ = self.collect(pulls=[pull], issues_comments=[
            {"issue_url": f"https://api.github.com/repos/{REPO}/issues/9", "created_at": stamp(2), "body": "this is really R1.6"}])
        self.assertEqual([e["task"] for e in events if e["type"] == "comment"], ["R1.6"])

    def test_a_comment_on_an_unknown_pr_has_no_task(self):
        events, _ = self.collect(issues_comments=[
            {"issue_url": f"https://api.github.com/repos/{REPO}/issues/99", "created_at": stamp(2), "body": "Looks good"}])
        self.assertEqual([(e["task"], e["phase"]) for e in events], [(None, None)])

    def test_old_events_are_dropped(self):
        events, _ = self.collect(commits=[{"html_url": f"https://github.com/{REPO}/commit/a",
                                           "commit": {"message": "old", "committer": {"date": stamp(24 * 30)}}}])
        self.assertEqual(events, [])

    def test_a_failed_call_costs_only_its_events(self):
        events, warnings = self.collect(
            pulls=urllib.error.URLError("offline"),
            commits=[{"html_url": f"https://github.com/{REPO}/commit/a",
                      "commit": {"message": "R1.6 done", "committer": {"date": stamp(1)}}}])
        self.assertEqual([e["type"] for e in events], ["commit"])
        self.assertEqual(len(warnings), 1)
        self.assertIn("pulls", warnings[0])

    def test_rate_limit_answer_is_not_a_crash(self):
        events, _ = self.collect(commits={"message": "API rate limit exceeded"})
        self.assertEqual(events, [])


class SanitizeTest(unittest.TestCase):
    def clean(self, *events):
        return roadmap_events.sanitize({"events": list(events)}, REPO, TASKS, PHASES, NOW)

    def event(self, **over):
        return {"type": "commit", "at": stamp(1), "title": "t", "num": None, "task": "R1.6", "phase": "R1",
                "url": f"https://github.com/{REPO}/commit/a", **over}

    def test_unknown_kinds_ids_and_bad_times_are_rejected_or_blanked(self):
        out = self.clean(self.event(type="deploy"), self.event(at="yesterday"), self.event(task="Z9.9", phase="Z9"),
                         self.event(at=stamp(24 * 40)), "not a dict")
        self.assertEqual(len(out), 1)
        self.assertEqual((out[0]["task"], out[0]["phase"]), (None, None))

    def test_links_must_stay_inside_the_repository(self):
        for bad in ("javascript:alert(1)", "https://evil.example/x", "https://github.com/other/repo/pull/1",
                    f"http://github.com/{REPO}/x"):
            self.assertIsNone(self.clean(self.event(url=bad))[0]["url"], bad)
        self.assertIsNotNone(self.clean(self.event())[0]["url"])

    def test_titles_are_one_clipped_line_without_control_characters(self):
        out = self.clean(self.event(title="first\nsecond\x00" + "x" * 300))
        self.assertEqual(out[0]["title"], "first")
        out = self.clean(self.event(title="a\x07b" + "y" * 300))
        self.assertLessEqual(len(out[0]["title"]), roadmap_events.TITLE_LIMIT)
        self.assertNotRegex(out[0]["title"], r"[\x00-\x1f]")

    def test_the_list_is_capped(self):
        out = self.clean(*[self.event(at=stamp(i / 100)) for i in range(100)])
        self.assertEqual(len(out), roadmap_events.MAX_EVENTS)

    def test_a_task_overrides_a_mismatched_phase(self):
        self.assertEqual(self.clean(self.event(task="R2.3", phase="R1"))[0]["phase"], "R2")


class LoadEventsTest(unittest.TestCase):
    def test_missing_or_broken_file_gives_no_events_and_a_warning(self):
        with tempfile.TemporaryDirectory() as tmp:
            bad = Path(tmp) / "events.json"
            payload, warnings = roadmap_events.load_events(bad, REPO, TASKS, PHASES)
            self.assertIsNone(payload)
            self.assertEqual(len(warnings), 1)
            bad.write_text("{not json", encoding="utf-8")
            self.assertIsNone(roadmap_events.load_events(bad, REPO, TASKS, PHASES)[0])


class WorldEventsBuildTest(unittest.TestCase):
    def build(self, events):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "events.json"
            path.write_text(json.dumps(events), encoding="utf-8")
            return roadmap_map.build_world(roadmap_map.ROOT, path, REPO)

    def test_without_events_the_page_still_builds(self):
        page, errors, _ = roadmap_map.build_world(roadmap_map.ROOT)
        self.assertEqual(errors, [])
        self.assertNotIn(roadmap_map.EVENTS_PLACEHOLDER, page)
        self.assertIn("const EVD = null;", page)

    def test_a_broken_events_file_is_a_warning_not_an_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "events.json"
            path.write_text("[[[", encoding="utf-8")
            page, errors, warnings = roadmap_map.build_world(roadmap_map.ROOT, path, REPO)
        self.assertEqual(errors, [])
        self.assertTrue(any("events file" in w for w in warnings))
        self.assertIn("const EVD = null;", page)

    def test_event_text_cannot_close_the_script_block(self):
        evil = "</script><script>alert(1)</script><!--"
        fetched = stamp(0)
        page, errors, _ = self.build({"fetched": fetched, "events": [
            {"type": "commit", "at": stamp(1), "title": evil, "num": None, "task": None, "phase": None, "url": None}]})
        self.assertEqual(errors, [])
        self.assertNotIn(evil, page)
        self.assertEqual(page.count("</script>"), 1)  # only the template's own closing tag
        self.assertIn("\\u003c/script>", page)


@unittest.skipUnless(shutil.which("node"), "Node.js is needed to run the page model")
class WorldEventsModelTest(unittest.TestCase):
    """Runs the model part of the campaign map with events in Node."""

    def model(self, events, spec_tasks=None):
        template = WORLD_TEMPLATE.read_text(encoding="utf-8")
        en = roadmap_map.parse((roadmap_map.ROOT / roadmap_map.EN_PATH).read_text(encoding="utf-8"), "en")
        ru = roadmap_map.parse((roadmap_map.ROOT / roadmap_map.RU_PATH).read_text(encoding="utf-8"), "ru")
        page = roadmap_map.render(template, roadmap_map.merge(en, ru), "test", "now")
        page = roadmap_map.inject_events(page, {"repo": REPO, "fetched": stamp(0), "events": events})
        code = page[page.index("// @model-begin"):page.index("// @model-end")]
        script = code + "process.stdout.write(JSON.stringify(EVENTS.map(e => [e.type, e.num, e.p.id, e.p.reg.key])));"
        out = subprocess.run(["node", "-"], input=script, capture_output=True, text=True, encoding="utf-8", check=True)
        return json.loads(out.stdout)

    def ev(self, kind, **over):
        return {"type": kind, "at": stamp(1), "title": "t", "num": None, "task": None, "phase": None, "url": None, **over}

    def test_an_event_lands_on_the_province_of_its_task(self):
        self.assertEqual(self.model([self.ev("commit", task="R1.6", phase="R1")])[0][2:], ["R1.6", "R1"])

    def test_a_phase_only_event_lands_in_that_region(self):
        self.assertEqual(self.model([self.ev("comment", phase="R2")])[0][3], "R2")

    def test_an_event_without_a_reference_goes_to_the_front(self):
        out = self.model([self.ev("commit")])
        self.assertEqual(len(out), 1)
        self.assertRegex(out[0][2], r"^R\d+\.\d+$")

    def test_a_merged_pr_replaces_its_opened_event(self):
        out = self.model([self.ev("pr", num=7, task="R1.6"), self.ev("merge", num=7, task="R1.6"),
                          self.ev("pr", num=8, task="R1.6")])
        self.assertEqual(sorted((e[0], e[1]) for e in out), [("merge", 7), ("pr", 8)])

    def test_the_page_escapes_event_text_and_links(self):
        template = WORLD_TEMPLATE.read_text(encoding="utf-8")
        tail = template[template.index("/* chronicle"):]
        self.assertIn("esc(e.title", tail)
        self.assertIn('rel="noopener noreferrer"', tail)
        self.assertNotRegex(tail, r"innerHTML\s*=[^;]*\be\.(title|url)\b(?![^;]*esc\()")
        self.assertIsNone(re.search(r"\bfetch\(|XMLHttpRequest|WebSocket", template))  # the page makes no requests


if __name__ == "__main__":
    unittest.main()
