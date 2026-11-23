#!/usr/bin/env python3
"""Build the visual roadmap page from task statuses in docs/product-roadmap.md.

The Markdown roadmap stays the single source of truth. Every phase section
(`### R0 — ...`, `### AI-A — ...`) has a task table whose last column holds the
status: `Done`, `In progress`, or an effort estimate for work not started yet.
The Russian counterpart uses `Сделано`/`Выполнено`/`Готово` and `В работе`.

The script parses both language versions, checks that they agree, and injects
the statuses into scripts/roadmap_map.html and into the campaign map
scripts/roadmap_world.html. See docs/roadmap-map.md.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EN_PATH = "docs/product-roadmap.md"
RU_PATH = "docs/ru/product-roadmap.md"
TEMPLATE_PATH = "scripts/roadmap_map.html"
WORLD_TEMPLATE_PATH = "scripts/roadmap_world.html"
DEFAULT_OUT = "outputs/roadmap/index.html"
WORLD_DIR = "world"  # the campaign map is written to <dir of --out>/world/index.html
PLACEHOLDER = "/*@ROADMAP_DATA@*/null"

HEADING = re.compile(r"^###\s+(?P<id>[A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*)\s+—\s+(?P<title>.+?)\s*$")
ANY_HEADING = re.compile(r"^#{1,3}\s")
TASK_ID = re.compile(r"^(?P<phase>[A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*)\.\d+$")
STATUS_WORDS = {
    "en": {"done": ("done",), "cur": ("in progress",)},
    "ru": {"done": ("сделано", "выполнено", "готово"), "cur": ("в работе",)},
}
# An optional task is tracked and shown but is not required to finish its phase.
OPTIONAL_PREFIX = {
    "en": re.compile(r"^optional\s*[·:]\s*", re.IGNORECASE),
    "ru": re.compile(r"^необязательно\s*[·:]\s*", re.IGNORECASE),
}
DONE_PREFIX = re.compile(r"^(done|сделано|выполнено|готово)\b[^:]{0,40}:\s*", re.IGNORECASE)
ESTIMATE = {
    lang: re.compile(rf"{number}(?:\s*[–-]\s*{number})?\s+{unit}")
    for lang, number, unit in (("en", r"[0-9]+(?:\.[0-9]+)?", "[dw]"),
                               ("ru", r"[0-9]+(?:,[0-9]+)?", "[дн]"))
}


@dataclass
class Task:
    id: str
    state: str  # done | cur | todo
    effort: str
    text: str
    acceptance: str
    detail: str = ""    # full task cell, for the campaign map card
    criteria: str = ""  # full acceptance cell
    optional: bool = False  # does not gate its phase


@dataclass
class Phase:
    id: str
    title: str
    tasks: list[Task] = field(default_factory=list)


def plain(cell: str) -> str:
    """Strip the Markdown that matters in table cells."""
    cell = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", cell)
    cell = cell.replace("`", "").replace("**", "").replace("\\|", "|")
    return re.sub(r"\s+", " ", cell).strip()


def short(text: str, limit: int = 150) -> str:
    text = DONE_PREFIX.sub("", text)
    text = text[:1].upper() + text[1:]
    for stop in (". ", "; "):
        cut = text.find(stop)
        if 0 < cut < limit:
            return text[:cut]
    text = text.rstrip(".")
    return text if len(text) <= limit else text[: limit - 1].rstrip() + "…"


def classify(cell: str, lang: str) -> str:
    low = plain(cell).lower()
    if low in STATUS_WORDS[lang]["done"]:
        return "done"
    for word in STATUS_WORDS[lang]["cur"]:
        if low == word or (low.startswith(word + " (") and low.endswith(")")
                           and ESTIMATE[lang].fullmatch(low[len(word) + 2:-1])):
            return "cur"
    if ESTIMATE[lang].fullmatch(low):
        return "todo"
    raise ValueError(f"unsupported {lang} roadmap status or estimate: {cell!r}")


def split_row(line: str) -> list[str]:
    body = line.strip()
    if body.startswith("|"):
        body = body[1:]
    if body.endswith("|"):
        body = body[:-1]
    return [cell.strip() for cell in re.split(r"(?<!\\)\|", body)]


def task_rows(markdown: str):
    """Yield each recognized row with its line and the containing phase heading."""
    section = None
    for line_number, line in enumerate(markdown.splitlines(keepends=True)):
        heading = HEADING.match(line)
        if heading:
            section = (line_number, heading["id"], plain(heading["title"]))
            continue
        if ANY_HEADING.match(line):
            section = None
            continue
        if section is None or not line.lstrip().startswith("|"):
            continue
        cells = split_row(line)
        task_id = TASK_ID.match(plain(cells[0]))
        if task_id and task_id["phase"] == section[1] and len(cells) >= 3:
            yield line_number, section, cells


def parse(markdown: str, lang: str) -> list[Phase]:
    phases: list[Phase] = []
    current_section = None
    for _, section, cells in task_rows(markdown):
        if current_section != section:
            current_section = section
            phases.append(Phase(section[1], section[2]))
        status = cells[-1]
        task_id = plain(cells[0])
        optional = bool(OPTIONAL_PREFIX[lang].match(plain(status)))
        try:
            state = classify(OPTIONAL_PREFIX[lang].sub("", plain(status)), lang)
        except ValueError as exc:
            raise ValueError(f"{EN_PATH if lang == 'en' else RU_PATH}: {task_id}: {exc}") from exc
        phases[-1].tasks.append(Task(
            id=task_id,
            state=state,
            effort=plain(status),
            text=short(plain(cells[1])),
            acceptance=short(plain(cells[2])) if len(cells) > 3 else "",
            detail=plain(cells[1]),
            criteria=plain(cells[2]) if len(cells) > 3 else "",
            optional=optional,
        ))
    return phases


def compare(en: list[Phase], ru: list[Phase]) -> list[str]:
    errors = []
    en_tasks = {t.id: t for p in en for t in p.tasks}
    ru_tasks = {t.id: t for p in ru for t in p.tasks}
    for task_id in sorted(en_tasks.keys() - ru_tasks.keys()):
        errors.append(f"{task_id}: present in {EN_PATH} but missing in {RU_PATH}")
    for task_id in sorted(ru_tasks.keys() - en_tasks.keys()):
        errors.append(f"{task_id}: present in {RU_PATH} but missing in {EN_PATH}")
    names = {"done": "done", "cur": "in progress", "todo": "not started"}
    for task_id, task in en_tasks.items():
        other = ru_tasks.get(task_id)
        if other and other.optional != task.optional:
            errors.append(f"{task_id}: optional marker differs between {EN_PATH} and {RU_PATH}")
        if other and other.state != task.state:
            errors.append(
                f"{task_id}: status differs — English '{task.effort}' is {names[task.state]}, "
                f"Russian '{other.effort}' is {names[other.state]}"
            )
    return errors


def merge(en: list[Phase], ru: list[Phase]) -> list[Phase]:
    """English statuses with Russian wording (the page is in Russian)."""
    ru_tasks = {t.id: t for p in ru for t in p.tasks}
    ru_titles = {p.id: p.title for p in ru}
    merged = []
    for phase in en:
        tasks = []
        for task in phase.tasks:
            local = ru_tasks.get(task.id)
            tasks.append(Task(
                id=task.id,
                state=task.state,
                effort=local.effort if local and local.state == "todo" else task.effort,
                text=local.text if local else task.text,
                acceptance=local.acceptance if local else task.acceptance,
                detail=local.detail if local else task.detail,
                criteria=local.criteria if local else task.criteria,
                optional=task.optional,
            ))
        merged.append(Phase(phase.id, ru_titles.get(phase.id, phase.title), tasks))
    return merged


def source_commit(root: Path) -> str:
    sha = os.environ.get("GITHUB_SHA", "")
    if not sha:
        try:
            sha = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, capture_output=True,
                                 text=True, check=True).stdout.strip()
        except (OSError, subprocess.CalledProcessError):
            sha = ""
    return sha[:7]


def render(template: str, phases: list[Phase], commit: str, generated: str) -> str:
    if template.count(PLACEHOLDER) != 1:
        raise ValueError(f"{TEMPLATE_PATH} must contain {PLACEHOLDER} exactly once")
    data = {
        "source": EN_PATH,
        "commit": commit,
        "generated": generated,
        "phases": [asdict(p) for p in phases],
    }
    blob = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("</", "<\\/")
    return template.replace(PLACEHOLDER, blob)


def unplaced(template: str, phases: list[Phase]) -> list[str]:
    return [p.id for p in phases if f"id: '{p.id}'" not in template]


def build(root: Path) -> tuple[str, list[str], list[str]]:
    en = parse((root / EN_PATH).read_text(encoding="utf-8"), "en")
    ru = parse((root / RU_PATH).read_text(encoding="utf-8"), "ru")
    errors = compare(en, ru)
    if not en:
        errors.append(f"no phase task tables found in {EN_PATH}")
    template = (root / TEMPLATE_PATH).read_text(encoding="utf-8")
    warnings = [f"phase {pid} has no node on the map in {TEMPLATE_PATH}; its tasks are not shown"
                for pid in unplaced(template, en)]
    generated = dt.datetime.now(dt.timezone.utc).strftime("%d.%m.%Y %H:%M UTC")
    page = render(template, merge(en, ru), source_commit(root), generated)
    return page, errors, warnings


TASK_DEPS = re.compile(r"const TASK_DEPS = \{(?P<body>[^}]*)\};")
TASK_DEP_ENTRY = re.compile(r"'(?P<task>[^']+)'\s*:\s*\[(?P<deps>[^\]]*)\]")


def task_deps(template: str) -> dict[str, list[str]]:
    """Cross-phase task dependencies declared in the campaign map template."""
    found = TASK_DEPS.search(template)
    if not found:
        return {}
    return {m["task"]: re.findall(r"'([^']+)'", m["deps"]) for m in TASK_DEP_ENTRY.finditer(found["body"])}


def dependency_errors(template: str, en: list[Phase], ru: list[Phase]) -> list[str]:
    """A declared dependency is an error when the task is in the roadmap but the task it waits for is not."""
    errors = []
    for path, phases in ((EN_PATH, en), (RU_PATH, ru)):
        ids = {t.id for p in phases for t in p.tasks}
        for task, deps in task_deps(template).items():
            for dep in deps:
                if task in ids and dep not in ids:
                    errors.append(f"{task} waits for {dep} (TASK_DEPS in {WORLD_TEMPLATE_PATH}), "
                                  f"but {dep} is missing from {path}; update TASK_DEPS together with the roadmap")
    return errors


def build_world(root: Path) -> tuple[str, list[str], list[str]]:
    """The campaign map page, errors and warnings; statuses come from the same tables as the table page."""
    en = parse((root / EN_PATH).read_text(encoding="utf-8"), "en")
    ru = parse((root / RU_PATH).read_text(encoding="utf-8"), "ru")
    template = (root / WORLD_TEMPLATE_PATH).read_text(encoding="utf-8")
    warnings = [f"phase {pid} has no region on the campaign map in {WORLD_TEMPLATE_PATH}; its tasks are not shown"
                for pid in unplaced(template, en)]
    generated = dt.datetime.now(dt.timezone.utc).strftime("%d.%m.%Y %H:%M UTC")
    errors = dependency_errors(template, en, ru)
    return render(template, merge(en, ru), source_commit(root), generated), errors, warnings


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", default=DEFAULT_OUT, help=f"output HTML (default {DEFAULT_OUT})")
    parser.add_argument("--check", action="store_true", help="only validate the roadmap; write nothing")
    args = parser.parse_args(argv)

    try:
        page, errors, warnings = build(ROOT)
        world, world_errors, world_warnings = build_world(ROOT)
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    errors += world_errors
    warnings += world_warnings
    for warning in warnings:
        print(f"warning: {warning}", file=sys.stderr)
    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    if errors:
        return 1
    if not args.check:
        out = Path(args.out)
        out = out if out.is_absolute() else ROOT / out
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(page, encoding="utf-8")
        print(f"wrote {out}")
        world_out = out.parent / WORLD_DIR / "index.html"
        world_out.parent.mkdir(parents=True, exist_ok=True)
        world_out.write_text(world, encoding="utf-8")
        print(f"wrote {world_out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
