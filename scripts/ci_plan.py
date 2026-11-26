"""Small CI path selection and fail-closed aggregate; no GitHub API or dependencies."""
import argparse
import json
import os
from pathlib import PurePosixPath
import re
import subprocess

import roadmap_map

ROADMAPS = {roadmap_map.EN_PATH: "en", roadmap_map.RU_PATH: "ru"}
ROUTES = ("native", "package", "docs_only")


def select_non_pr(paths, docs_only=False):
    # Only documentation-only changes omit the additional native/package routes.
    # Unknown/new source paths fail toward more coverage, not a missed subsystem.
    relevant = not paths or any(not (path.startswith("docs/") or
                                     PurePosixPath(path).suffix.lower() == ".md" or
                                     path in {"LICENSE-MIT", "LICENSE-APACHE", ".gitignore"}) for path in paths)
    return {"native": relevant, "package": relevant, "docs_only": docs_only, "paths": paths}


def changed_files(base, head="HEAD"):
    if not base or base == "0" * 40:
        return ["<initial-or-manual-run>"], False
    if not re.fullmatch(r"[a-fA-F0-9]{40}", base):
        raise ValueError("Base must be a full Git SHA")
    result = subprocess.run(["git", "diff", "--no-ext-diff", "--no-renames", "--raw", "-z",
                             base, head, "--"], check=True, capture_output=True)
    fields = result.stdout.split(b"\0")[:-1]
    paths = []
    regular = True
    for i in range(0, len(fields), 2):
        old_mode, new_mode = fields[i][1:].split()[:2]
        regular = regular and all(mode in {b"000000", b"100644"} for mode in (old_mode, new_mode))
        paths.append(fields[i + 1].decode("utf-8", errors="surrogateescape"))
    return sorted(paths), regular



def status_edits(before, after):
    """Return task IDs for a nonempty status-cell-only edit, or None."""
    old_lines = before.splitlines(keepends=True)
    new_lines = after.splitlines(keepends=True)
    if len(old_lines) != len(new_lines):
        return None
    rows = {i: cells for i, _, cells in roadmap_map.task_rows(before)}
    ids = [roadmap_map.plain(cells[0]) for cells in rows.values()]
    if len(set(ids)) != len(ids):
        return None
    changed = set()
    for i, (old, new) in enumerate(zip(old_lines, new_lines)):
        if old == new:
            continue
        if i not in rows or len(rows[i]) != 4:
            return None
        # Keep every character outside the last cell, including line endings.
        old_pipes = [m.start() for m in re.finditer(r"(?<!\\)\|", old)]
        new_pipes = [m.start() for m in re.finditer(r"(?<!\\)\|", new)]
        if len(old_pipes) != 5 or len(new_pipes) != 5:
            return None
        start, end = old_pipes[-2:]
        new_start, new_end = new_pipes[-2:]
        if (old[:start + 1] != new[:new_start + 1] or old[end:] != new[new_end:]
                or roadmap_map.plain(old[start + 1:end]) == roadmap_map.plain(new[new_start + 1:new_end])):
            return None
        changed.add(roadmap_map.plain(rows[i][0]))
    return changed or None


def roadmap_status_only(before, after):
    edits = [status_edits(before[path], after[path]) for path in ROADMAPS]
    if not edits[0] or edits[0] != edits[1]:
        return False
    en, ru = [roadmap_map.parse(after[path], lang) for path, lang in ROADMAPS.items()]
    return not roadmap_map.compare(en, ru)


def read_roadmaps(commit):
    result = subprocess.run(["git", "ls-tree", "-z", commit, "--", *ROADMAPS], check=True, capture_output=True)
    marked = [entry.split(b"\t", 1) for entry in result.stdout.split(b"\0") if entry]
    if len(marked) != len(ROADMAPS) or any(not meta.startswith(b"100644 blob ") for meta, _ in marked):
        return None
    return {path.decode("utf-8"): subprocess.run(["git", "cat-file", "blob", meta.split()[-1].decode("ascii")],
                                               check=True, capture_output=True).stdout.decode("utf-8")
            for meta, path in marked}


def documentation_path(path):
    return (path.startswith("docs/") and path.endswith((".md", ".markdown"))
            or path in {"AGENTS.md", "README.md", "README.ru.md"}
            or re.fullmatch(r"(?:crates|examples)/[^/]+/README(?:\.ru)?\.md", path) is not None)


def plan(base, head="HEAD", event=""):
    paths, regular = changed_files(base, head)
    if event == "pull_request":
        light = bool(paths) and regular and all(documentation_path(path) for path in paths)
        return {"native": not light, "package": not light, "docs_only": light, "paths": paths}

    if set(paths) != ROADMAPS.keys():
        return select_non_pr(paths)
    before, after = read_roadmaps(base), read_roadmaps(head)
    if not before or not after:
        return select_non_pr(paths)
    return select_non_pr(paths, docs_only=roadmap_status_only(before, after))


def check_gate(plan_result, checks_result, roadmap_result, selected):
    if plan_result != "success":
        raise ValueError("CI requires successful route selection")
    if set(selected) != set(ROUTES) or any(value not in {"true", "false"} for value in selected.values()):
        raise ValueError("Missing or invalid route selection")
    if selected["docs_only"] == "true":
        if (selected["native"], selected["package"], checks_result, roadmap_result) != ("false", "false", "skipped", "success"):
            raise ValueError("Documentation CI requires successful roadmap checks and a skipped OS matrix")
    elif (checks_result, roadmap_result) != ("success", "skipped"):
        raise ValueError("Ordinary CI requires success from both OS jobs and skips the documentation job")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="")
    parser.add_argument("--gate", action="store_true")
    args = parser.parse_args()
    if args.gate:
        selected = {key: os.environ.get("CI_" + key.upper(), "") for key in ROUTES}
        check_gate(os.environ.get("CI_PLAN_RESULT"), os.environ.get("CI_CHECKS_RESULT"),
                   os.environ.get("CI_ROADMAP_RESULT"), selected)
        print("CI required passed; route selection: " + json.dumps(selected))
        return
    selected = plan(args.base, event=os.environ.get("GITHUB_EVENT_NAME", ""))
    print(json.dumps(selected, ensure_ascii=True, indent=2))
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a", encoding="utf-8") as stream:
            for key in ROUTES:
                stream.write(key + "=" + str(selected[key]).lower() + "\n")


if __name__ == "__main__":
    main()
