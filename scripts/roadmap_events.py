#!/usr/bin/env python3
"""Collect recent GitHub activity for the campaign map (scripts/roadmap_world.html).

Runs at build time only. The workflow passes the job's own GITHUB_TOKEN through the
environment; the token never reaches the generated page, and neither do author names
or comment bodies: an event keeps only its kind, time, a short title, an optional
issue/PR number, the roadmap task it mentions, and a github.com link.

    python scripts/roadmap_events.py --repo owner/name --out events.json

A failed or rate-limited API call is not an error: the file is written with fewer
events (or none) and the page is built without them. See docs/roadmap-map.md.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Callable

API = "https://api.github.com"
WINDOW_DAYS = 14
MAX_EVENTS = 30
TITLE_LIMIT = 90
TYPES = ("commit", "pr", "merge", "comment", "review")
ISO = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
Getter = Callable[[str], object]


def clip(text: object, limit: int = TITLE_LIMIT) -> str:
    """First line of the text, without control characters, cut to `limit`."""
    line = str(text or "").strip().splitlines()[0] if str(text or "").strip() else ""
    line = re.sub(r"[\x00-\x1f\x7f]", " ", line).strip()
    return line if len(line) <= limit else line[: limit - 1].rstrip() + "…"


def find_ref(text: str, tasks: set[str], phases: set[str]) -> tuple[str | None, str | None]:
    """The roadmap task (and its phase) a text mentions, only ids that exist in the roadmap."""
    text = text or ""
    for task in sorted(tasks, key=len, reverse=True):
        if re.search(rf"(?<![\w.-]){re.escape(task)}(?![\w-])(?!\.\d)", text):
            return task, task.rsplit(".", 1)[0]
    for phase in sorted(phases, key=len, reverse=True):
        if re.search(rf"(?<![\w.-]){re.escape(phase)}(?![\w-])(?!\.\d)", text):
            return None, phase
    return None, None


def parse_time(value: object) -> dt.datetime | None:
    if not isinstance(value, str) or not ISO.match(value):
        return None
    return dt.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=dt.timezone.utc)


def collect(repo: str, tasks: set[str], phases: set[str], now: dt.datetime, get: Getter,
            days: int = WINDOW_DAYS) -> tuple[list[dict], list[str]]:
    """Events from the repository's last `days` days, newest first, and warnings about failed calls."""
    warnings: list[str] = []

    def fetch(path: str):
        try:
            data = get(f"{API}/repos/{repo}/{path}")
        except (OSError, ValueError, urllib.error.URLError) as err:
            warnings.append(f"GitHub API {path.split('?')[0]} failed ({type(err).__name__}); those events are skipped")
            return []
        return data if isinstance(data, list) else []

    pulls = [p for p in fetch("pulls?state=all&sort=updated&direction=desc&per_page=30") if isinstance(p, dict)]
    pr_title = {p.get("number"): p.get("title") or "" for p in pulls}
    raw: list[dict] = []

    def add(kind: str, at: object, title: str, num: object, url: object, extra: str = "") -> None:
        task, phase = find_ref(f"{title} {extra}", tasks, phases)
        raw.append({"type": kind, "at": at, "title": clip(title), "num": num if isinstance(num, int) else None,
                    "task": task, "phase": phase, "url": url})

    for p in pulls:
        branch = ((p.get("head") or {}).get("ref")) or ""
        add("pr", p.get("created_at"), p.get("title") or "", p.get("number"), p.get("html_url"), branch)
        if p.get("merged_at"):
            add("merge", p.get("merged_at"), p.get("title") or "", p.get("number"), p.get("html_url"), branch)
    for c in fetch("commits?per_page=30"):
        if isinstance(c, dict):
            message = ((c.get("commit") or {}).get("message")) or ""
            when = (((c.get("commit") or {}).get("committer")) or {}).get("date")
            add("commit", when, message, None, c.get("html_url"))
    for c in fetch("issues/comments?sort=created&direction=desc&per_page=30"):
        if isinstance(c, dict):
            num = int(str(c.get("issue_url") or "0").rsplit("/", 1)[-1] or 0) or None
            add("comment", c.get("created_at"), pr_title.get(num) or "", num, c.get("html_url"), c.get("body") or "")
    for c in fetch("pulls/comments?sort=created&direction=desc&per_page=30"):
        if isinstance(c, dict):
            num = int(str(c.get("pull_request_url") or "0").rsplit("/", 1)[-1] or 0) or None
            add("review", c.get("created_at"), pr_title.get(num) or "", num, c.get("html_url"), c.get("body") or "")

    return sanitize({"events": raw}, repo, tasks, phases, now, days), warnings


def sanitize(data: object, repo: str, tasks: set[str], phases: set[str], now: dt.datetime | None = None,
             days: int = WINDOW_DAYS) -> list[dict]:
    """The trust boundary: only known event kinds, ISO times, known roadmap ids and github.com links survive."""
    events = data.get("events") if isinstance(data, dict) else None
    out: list[dict] = []
    for e in events if isinstance(events, list) else []:
        if not isinstance(e, dict) or e.get("type") not in TYPES:
            continue
        when = parse_time(e.get("at"))
        if when is None or (now is not None and not now - dt.timedelta(days=days) <= when <= now + dt.timedelta(hours=1)):
            continue
        task = e.get("task") if e.get("task") in tasks else None
        phase = e.get("phase") if e.get("phase") in phases else None
        if task:
            phase = task.rsplit(".", 1)[0]
        url = e.get("url") if isinstance(e.get("url"), str) and e["url"].startswith(f"https://github.com/{repo}/") else None
        num = e.get("num") if isinstance(e.get("num"), int) and not isinstance(e.get("num"), bool) else None
        out.append({"type": e["type"], "at": e["at"], "title": clip(e.get("title")), "num": num,
                    "task": task, "phase": phase, "url": url})
    out.sort(key=lambda e: e["at"], reverse=True)
    return out[:MAX_EVENTS]


def load_events(path: Path, repo: str, tasks: set[str], phases: set[str]) -> tuple[dict | None, list[str]]:
    """The page payload (repo, fetch time, events) from a file written by this script; a bad file gives None and a warning."""
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as err:
        return None, [f"events file {path} is not usable ({type(err).__name__}); the map is built without events"]
    fetched = data.get("fetched") if isinstance(data, dict) else None
    return {"repo": repo, "fetched": fetched if parse_time(fetched) else None,
            "events": sanitize(data, repo, tasks, phases)}, []


def http_getter(token: str) -> Getter:
    def get(url: str):
        request = urllib.request.Request(url, headers={
            "Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "moduleengine-roadmap-map",
            **({"Authorization": f"Bearer {token}"} if token else {})})
        with urllib.request.urlopen(request, timeout=20) as response:  # noqa: S310 - fixed https API host
            return json.load(response)
    return get


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo", default=os.environ.get("GITHUB_REPOSITORY", ""), help="owner/name")
    parser.add_argument("--out", required=True, help="events JSON file to write")
    parser.add_argument("--days", type=int, default=WINDOW_DAYS)
    args = parser.parse_args(argv)
    if not re.fullmatch(r"[\w.-]+/[\w.-]+", args.repo):
        print("error: --repo must look like owner/name (or set GITHUB_REPOSITORY)", file=sys.stderr)
        return 1
    import roadmap_map  # lazy: roadmap_map imports this module too

    en = roadmap_map.parse((roadmap_map.ROOT / roadmap_map.EN_PATH).read_text(encoding="utf-8"), "en")
    tasks = {t.id for p in en for t in p.tasks}
    phases = {p.id for p in en}
    now = dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
    events, warnings = collect(args.repo, tasks, phases, now, http_getter(os.environ.get("GITHUB_TOKEN", "")), args.days)
    for warning in warnings:
        print(f"warning: {warning}", file=sys.stderr)
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps({"repo": args.repo, "fetched": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
                                          "events": events}, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"wrote {args.out}: {len(events)} events")
    return 0


if __name__ == "__main__":
    sys.exit(main())
