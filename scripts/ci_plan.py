"""Small CI path selection and fail-closed aggregate; no GitHub API or dependencies."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

ROUTES_FILE = Path(__file__).resolve().parent.parent / ".github" / "ci-routes.json"
ROUTES = ("native", "package", "light")
LIGHT_ROUTES = ("docs", "site")


def glob_regex(pattern):
    """Translate a route glob: '*' stays in one directory, '**' crosses directories."""
    out, i = [], 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
        elif pattern.startswith("**", i):
            out.append(".*")
            i += 2
        elif pattern[i] == "*":
            out.append("[^/]*")
            i += 1
        else:
            out.append(re.escape(pattern[i]))
            i += 1
    return re.compile("".join(out))


def load_rules(path=ROUTES_FILE):
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    rules = []
    for rule in data["rules"]:
        if rule["route"] not in data["routes"]:
            raise ValueError("Unknown route in " + str(path) + ": " + rule["route"])
        rules.extend((rule["route"], glob_regex(pattern)) for pattern in rule["paths"])
    return rules


def classify(path, rules=None):
    """Return the route of the first matching rule, or None when no rule matches."""
    for route, regex in rules if rules is not None else load_rules():
        if regex.fullmatch(path):
            return route
    return None


def select(paths, regular=True, rules=None):
    """Route a set of changed paths. Anything unknown, irregular or empty runs the full matrix."""
    rules = load_rules() if rules is None else rules
    routes = {path: classify(path, rules) for path in paths}
    triggers = sorted(path for path, route in routes.items() if route not in LIGHT_ROUTES)
    if not paths or not regular:
        route = "full"
        reason = "no changed files in the diff" if not paths else "a changed file is not a regular, non-executable file"
    elif triggers:
        route = "full"
        reason = "files outside the light routes"
    else:
        route = "site" if "site" in routes.values() else "docs"
        reason = "only " + route + " files changed"
    light = route in LIGHT_ROUTES
    return {"native": not light, "package": not light, "light": light, "route": route,
            "reason": reason, "paths": sorted(paths), "triggers": triggers,
            "unclassified": sorted(path for path, found in routes.items() if found is None)}


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


def plan(base, head="HEAD"):
    """One routing rule for every event. On pull_request, merge_group and push the diff runs from the event's
    base SHA, so a merge group or a push is the union of every change it carries: any code means the full matrix."""
    paths, regular = changed_files(base, head)
    return select(paths, regular)


def summary(selected, limit=40):
    lines = ["### CI route: `" + selected["route"] + "`", "",
             "Reason: " + selected["reason"] + ". Changed files: " + str(len(selected["paths"])) + ".", ""]
    shown = selected["triggers"] if selected["route"] == "full" and selected["triggers"] else selected["paths"]
    title = "Files that selected the full matrix" if shown is selected["triggers"] else "Changed files"
    if shown:
        lines += ["**" + title + "**", ""]
        lines += ["- `" + path + "`" for path in shown[:limit]]
        if len(shown) > limit:
            lines.append("- ... and " + str(len(shown) - limit) + " more")
    if selected["unclassified"]:
        lines += ["", "No rule in `.github/ci-routes.json` for: " + ", ".join("`" + p + "`" for p in selected["unclassified"][:limit])]
    return "\n".join(lines) + "\n"


CORE_JOBS = ("workspace", "release", "clippy", "contracts", "player")


def check_gate(needs):
    if needs.get("plan", {}).get("result") != "success":
        raise ValueError("CI requires successful route selection")
    selected = needs["plan"].get("outputs", {})
    flags = {key: value for key, value in selected.items() if key != "route"}
    if set(flags) != set(ROUTES) or any(value not in {"true", "false"} for value in flags.values()):
        raise ValueError("Missing or invalid route selection")
    if selected.get("route") not in {*LIGHT_ROUTES, "full"} or (selected["route"] in LIGHT_ROUTES) != (flags["light"] == "true"):
        raise ValueError("Route name does not match the light/full selection")
    light = flags["light"] == "true"
    if light and any(flags[key] == "true" for key in ("native", "package")):
        raise ValueError("Light plan selected a native route")
    expected = {key: not light for key in CORE_JOBS}
    expected.update(documentation=True, native=flags["native"] == "true", package=flags["package"] == "true")
    if set(needs) != {"plan", *expected}:
        raise ValueError("Missing or unexpected prerequisite job")
    for job, required in expected.items():
        result = needs[job].get("result")
        if result != ("success" if required else "skipped"):
            raise ValueError(f"Unexpected result for {job}: {result}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="")
    parser.add_argument("--gate", action="store_true")
    args = parser.parse_args()
    if args.gate:
        check_gate(json.loads(os.environ["CI_NEEDS"]))
        print("CI required passed")
        return
    selected = plan(args.base)
    print(json.dumps(selected, ensure_ascii=True, indent=2))
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a", encoding="utf-8") as stream:
            for key in ROUTES:
                stream.write(key + "=" + str(selected[key]).lower() + "\n")
            stream.write("route=" + selected["route"] + "\n")
    report = os.environ.get("GITHUB_STEP_SUMMARY")
    if report:
        with open(report, "a", encoding="utf-8") as stream:
            stream.write(summary(selected))


if __name__ == "__main__":
    main()
