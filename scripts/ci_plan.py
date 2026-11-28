"""Small CI path selection and fail-closed aggregate; no GitHub API or dependencies."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROUTES_FILE = Path(__file__).resolve().parent.parent / ".github" / "ci-routes.json"
ROUTES = ("native", "package", "light")
LIGHT_ROUTES = ("docs", "site", "policy")
LIGHT_REASONS = {
    "docs": "only documentation changed",
    "site": "site files changed with optional documentation",
    "policy": "CI policy changed with optional documentation or site files",
}
PACKAGE_MANIFEST = ".module-engine-package.json"
POLICY_PATHS = {"scripts/ci_plan.py", "scripts/test_ci.py"}
SHA256 = re.compile("[0-9a-f]{64}")


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


def select(paths, regular=True, rules=None, policy=False):
    """Route a set of changed paths. Anything unknown, irregular or empty runs the full matrix."""
    rules = load_rules() if rules is None else rules
    routes = {path: "policy" if policy and path in POLICY_PATHS else classify(path, rules) for path in paths}
    triggers = sorted(path for path, route in routes.items() if route not in LIGHT_ROUTES)
    if not paths or not regular:
        route = "full"
        reason = "no changed files in the diff" if not paths else "a changed file is not a regular, non-executable file"
    elif triggers:
        route = "full"
        reason = "files outside the light routes"
    else:
        route = next(name for name in reversed(LIGHT_ROUTES) if name in routes.values())
        reason = LIGHT_REASONS[route]
    light = route in LIGHT_ROUTES
    return {"native": not light, "package": not light, "light": light, "route": route,
            "reason": reason, "paths": sorted(paths), "triggers": triggers,
            "unclassified": sorted(path for path, found in routes.items() if found is None)}


def changed_files(base, head="HEAD"):
    if not base or base == "0" * 40:
        return {"<initial-or-manual-run>": None}, False
    if not re.fullmatch(r"[a-fA-F0-9]{40}", base):
        raise ValueError("Base must be a full Git SHA")
    result = subprocess.run(["git", "diff", "--no-ext-diff", "--no-renames", "--raw", "--no-abbrev", "-z",
                             base, head, "--"], check=True, capture_output=True)
    fields = result.stdout.split(b"\0")[:-1]
    changes = {}
    regular = True
    for i in range(0, len(fields), 2):
        old_mode, new_mode, old_blob, new_blob, status = fields[i][1:].split()
        regular = regular and all(mode in {b"000000", b"100644"} for mode in (old_mode, new_mode))
        path = fields[i + 1].decode("utf-8", errors="surrogateescape")
        changes[path] = ((old_blob.decode("ascii"), new_blob.decode("ascii"))
                         if old_mode == new_mode == b"100644" and status == b"M" and old_blob != new_blob else None)
    return changes, regular


def git_blob(oid):
    return subprocess.run(["git", "cat-file", "blob", oid], check=True, capture_output=True).stdout


def unique_object(pairs):
    data = dict(pairs)
    if len(data) != len(pairs):
        raise ValueError("Duplicate package manifest key")
    return data


def strings(value):
    return isinstance(value, list) and all(isinstance(item, str) for item in value)


def read_package_manifest(oid):
    manifest = json.loads(git_blob(oid).decode("utf-8"), object_pairs_hook=unique_object)
    if not isinstance(manifest, dict) or set(manifest) != {
        "schema_version", "engine_version", "profile", "files", "module_sources",
        "gameplay_source_ids", "blueprint_hash", "sdk_files", "backend_files",
    }:
        raise ValueError("Unknown package manifest fields")
    schema = manifest["schema_version"]
    blueprint = manifest["blueprint_hash"]
    modules = manifest["module_sources"]
    if (type(schema) is not int or schema != 1 or not isinstance(manifest["engine_version"], str)
            or manifest["profile"] not in ("full", "team")
            or (blueprint is not None and not isinstance(blueprint, str))
            or not strings(manifest["gameplay_source_ids"])
            or not isinstance(modules, dict) or not all(strings(paths) for paths in modules.values())):
        raise ValueError("Unknown package manifest shape")
    for field in ("files", "sdk_files", "backend_files"):
        pins = manifest[field]
        if not isinstance(pins, dict) or not all(isinstance(pin, str) and SHA256.fullmatch(pin)
                                                  for pin in pins.values()):
            raise ValueError("Invalid package manifest hashes")
    return manifest


def derived_pins(changes, policy_paths):
    """Check the narrow package exception with raw Git blobs from the event diff."""
    manifest_blobs = changes[PACKAGE_MANIFEST]
    if not manifest_blobs:
        return False
    paths = changes.keys() - {PACKAGE_MANIFEST}
    try:
        before, after = map(read_package_manifest, manifest_blobs)
        pin_fields = ("sdk_files", "backend_files") if before["profile"] == "full" else ("sdk_files",)
        if any(before[field] != after[field] for field in before.keys() - set(pin_fields)):
            return False
        if paths.intersection(before["files"]) or (
                "backend_files" not in pin_fields and paths.intersection(before["backend_files"])):
            return False
        updated = set()
        for field in pin_fields:
            old_pins, new_pins = before[field], after[field]
            if old_pins.keys() != new_pins.keys():
                return False
            changed_pins = {path for path in old_pins if old_pins[path] != new_pins[path]}
            if changed_pins != paths.intersection(old_pins):
                return False
            for path in changed_pins:
                blobs = changes[path]
                # A changed regular blob is required on both sides. The new pin must
                # match HEAD even if the base pin was stale and needed repair.
                if not blobs or hashlib.sha256(git_blob(blobs[1])).hexdigest() != new_pins[path]:
                    return False
            updated.update(changed_pins)
        return bool(updated) and policy_paths <= updated
    except (ValueError, RecursionError, subprocess.CalledProcessError):
        return False


def plan(base, head="HEAD"):
    """One routing rule for every event. On pull_request, merge_group and push the diff runs from the event's
    base SHA, so a merge group or a push is the union of every change it carries: any code means the full matrix."""
    changes, regular = changed_files(base, head)
    rules = load_rules()
    if regular and changes.get(PACKAGE_MANIFEST):
        other_paths = changes.keys() - {PACKAGE_MANIFEST}
        policy_paths = other_paths & POLICY_PATHS
        selected = select(other_paths, regular, rules, policy=bool(policy_paths))
        if selected["light"] and derived_pins(changes, policy_paths):
            selected["paths"] = sorted(changes)
            selected["reason"] += " with verified package hash updates"
            return selected
    return select(changes, regular, rules)


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
    expected_flag = "false" if light else "true"
    if any(flags[key] != expected_flag for key in ("native", "package")):
        raise ValueError("Native and package must both be selected exactly for the full route")
    expected = {key: not light for key in CORE_JOBS}
    expected.update(documentation=True, native=not light, package=not light)
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
    if selected["route"] == "policy":
        subprocess.run([sys.executable, "-B", str(Path(__file__).with_name("test_ci.py"))], check=True)
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
