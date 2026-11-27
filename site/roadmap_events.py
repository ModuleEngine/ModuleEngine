#!/usr/bin/env python3
"""Collect recent GitHub activity for the campaign map (site/roadmap_world.html).

Runs at build time only. The workflow passes the job's own GITHUB_TOKEN through the
environment; the token never reaches the generated page, and neither do author names
or comment bodies: an event keeps only its kind, time, a short title, an optional
issue/PR number, the roadmap task it mentions, and a github.com link.

    python site/roadmap_events.py --repo owner/name --out events.json

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


def _first_mention(text: str, ids: set[str]) -> str | None:
    """The id that appears first in the text; ids are tried longest first so R10.1 is not read as R1."""
    if not ids:
        return None
    alternatives = "|".join(re.escape(i) for i in sorted(ids, key=lambda i: (-len(i), i)))
    tail = r"(?!\w)(?!\.\d)(?!-\d)"
    found = re.search(rf"(?<![\w.])({alternatives}){tail}", text or "")
    return found.group(1) if found else None


def find_ref(title: str, extra: str, tasks: set[str], phases: set[str]) -> tuple[str | None, str | None]:
    """The roadmap task (and its phase) an event belongs to, from known ids only and in a fixed order:
    the first task in the title, else the first task in `extra` (branch name, comment body, rest of a commit
    message), else the first phase named in the title, else in `extra`. The same text always gives the same
    answer, whatever the iteration order of the sets."""
    for text in (title, extra):
        task = _first_mention(text, tasks)
        if task:
            return task, task.rsplit(".", 1)[0]
    for text in (title, extra):
        phase = _first_mention(text, phases)
        if phase:
            return None, phase
    return None, None


def parse_time(value: object) -> dt.datetime | None:
    """A UTC ISO time such as 2026-11-22T12:00:00Z; None for anything else, including dates that do not exist."""
    if not isinstance(value, str) or not ISO.match(value):
        return None
    try:
        return dt.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=dt.timezone.utc)
    except ValueError:
        return None


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
    # what each PR itself resolves to (title, then branch): a comment on it falls back to this
    pr_ref = {p.get("number"): find_ref(p.get("title") or "", ((p.get("head") or {}).get("ref")) or "", tasks, phases)
              for p in pulls}
    raw: list[dict] = []

    def add(kind: str, at: object, title: str, num: object, url: object, extra: str = "") -> None:
        task, phase = find_ref(title, extra, tasks, phases)
        if task is None and phase is None and kind in ("comment", "review"):
            task, phase = pr_ref.get(num, (None, None))  # nothing in the comment or the PR title: use the PR's own task
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
            head, _, rest = message.partition("\n")
            when = (((c.get("commit") or {}).get("committer")) or {}).get("date")
            add("commit", when, head, None, c.get("html_url"), rest)
    for c in fetch("issues/comments?sort=created&direction=desc&per_page=30"):
        if isinstance(c, dict):
            num = int(str(c.get("issue_url") or "0").rsplit("/", 1)[-1] or 0) or None
            add("comment", c.get("created_at"), pr_title.get(num) or "", num, c.get("html_url"), c.get("body") or "")
    for c in fetch("pulls/comments?sort=created&direction=desc&per_page=30"):
        if isinstance(c, dict):
            num = int(str(c.get("pull_request_url") or "0").rsplit("/", 1)[-1] or 0) or None
            add("review", c.get("created_at"), pr_title.get(num) or "", num, c.get("html_url"), c.get("body") or "")

    return sanitize({"events": raw}, repo, tasks, phases, now, days), warnings


def _text_or_none(value: object) -> str | None:
    return value if isinstance(value, str) else None


def sanitize_report(data: object, repo: str, tasks: set[str], phases: set[str], now: dt.datetime | None = None,
                    days: int = WINDOW_DAYS) -> tuple[list[dict], int]:
    """The trust boundary: only known event kinds, ISO times, known roadmap ids and github.com links survive.

    Returns the events and how many were malformed (wrong types, unknown kind, impossible time); those are
    omitted. Events outside the time window, unknown ids and foreign links are not malformed: the event or
    the field is dropped quietly."""
    events = data.get("events") if isinstance(data, dict) else None
    out: list[dict] = []
    malformed = 0
    for e in events if isinstance(events, list) else []:
        if not isinstance(e, dict) or e.get("type") not in TYPES or not isinstance(e.get("type"), str):
            malformed += 1
            continue
        when = parse_time(e.get("at"))
        if when is None:
            malformed += 1
            continue
        if now is not None and not now - dt.timedelta(days=days) <= when <= now + dt.timedelta(hours=1):
            continue
        fields = {k: e.get(k) for k in ("task", "phase", "url", "title")}
        num = e.get("num")
        if any(v is not None and not isinstance(v, str) for v in fields.values()) or \
                (num is not None and (not isinstance(num, int) or isinstance(num, bool))):
            malformed += 1
            continue
        task = fields["task"] if fields["task"] in tasks else None
        phase = fields["phase"] if fields["phase"] in phases else None
        if task:
            phase = task.rsplit(".", 1)[0]
        url = fields["url"] if fields["url"] and fields["url"].startswith(f"https://github.com/{repo}/") else None
        out.append({"type": e["type"], "at": e["at"], "title": clip(fields["title"]), "num": num,
                    "task": task, "phase": phase, "url": url})
    out.sort(key=lambda e: e["at"], reverse=True)
    return out[:MAX_EVENTS], malformed


def sanitize(data: object, repo: str, tasks: set[str], phases: set[str], now: dt.datetime | None = None,
             days: int = WINDOW_DAYS) -> list[dict]:
    return sanitize_report(data, repo, tasks, phases, now, days)[0]


def load_events(path: Path, repo: str, tasks: set[str], phases: set[str]) -> tuple[dict | None, list[str]]:
    """The page payload (repo, fetch time, events) from a file written by this script.

    An unreadable file gives None and a warning; malformed events are omitted with a warning. Neither stops the build."""
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as err:
        return None, [f"events file {path} is not usable ({type(err).__name__}); the map is built without events"]
    events, malformed = sanitize_report(data, repo, tasks, phases)
    warnings = [f"{malformed} malformed event(s) in {path} were left out of the map"] if malformed else []
    fetched = data.get("fetched") if isinstance(data, dict) else None
    return {"repo": repo, "fetched": fetched if parse_time(fetched) else None, "events": events}, warnings


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
