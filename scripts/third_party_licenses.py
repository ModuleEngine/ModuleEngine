#!/usr/bin/env python3
"""Third-party Rust license policy and notice file; standard library only.

Generates THIRD-PARTY-LICENSES.txt from the locked dependency graph of the
shipped targets (normal and build dependencies for Windows and Linux) and
checks every package's SPDX expression against the allowed license list.
`--check` fails when the committed file is stale or a license is not allowed.
Run after `cargo fetch --locked` so package sources are present locally.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = "THIRD-PARTY-LICENSES.txt"
PLATFORMS = ("x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu")

# Permissive licenses compatible with distributing ModuleEngine under
# MIT OR Apache-2.0, including in closed games built with it.
ALLOWED = {
    "MIT", "MIT-0", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "Zlib",
    "BSD-2-Clause", "BSD-3-Clause", "ISC", "BSL-1.0", "0BSD", "Unlicense",
    "CC0-1.0", "Unicode-3.0", "Unicode-DFS-2016", "OFL-1.1", "Ubuntu-font-1.0",
}
# Preferred license when a package without its own license file offers a choice.
STANDARD_TEXTS = ("MIT", "Apache-2.0", "BSL-1.0")
NOTICE_FILE = re.compile(r"^(licen[cs]e|copying|copyright|notice|unlicense)", re.IGNORECASE)
# Notices that live outside the package root and are compiled into binaries.
EXTRA_NOTICES = {
    "epaint_default_fonts": ("fonts/OFL.txt", "fonts/UFL.txt", "fonts/Hack-Regular.txt",
                             "fonts/emoji-icon-font-mit-license.txt"),
}


class PolicyError(Exception):
    pass


def tokenize(expression):
    return re.findall(r"\(|\)|[^\s()]+", expression.replace("/", " OR "))


def license_options(expression):
    """Return the alternatives of an SPDX expression as sets of required licenses."""
    tokens = tokenize(expression)
    position = 0

    def peek():
        return tokens[position] if position < len(tokens) else None

    def take():
        nonlocal position
        position += 1
        return tokens[position - 1]

    def atom():
        token = take() if peek() is not None else None
        if token is None or token in {")", "AND", "OR", "WITH"}:
            raise PolicyError(f"Invalid SPDX expression: {expression}")
        if token == "(":
            options = either()
            if peek() != ")":
                raise PolicyError(f"Unbalanced SPDX expression: {expression}")
            take()
            return options
        if peek() == "WITH":
            take()
            if peek() in {None, "(", ")", "AND", "OR", "WITH"}:
                raise PolicyError(f"Invalid SPDX expression: {expression}")
            token = f"{token} WITH {take()}"
        return [frozenset([token.removesuffix("+")])]

    def both():
        options = atom()
        while peek() == "AND":
            take()
            right = atom()
            options = [left | other for left in options for other in right]
        return options

    def either():
        options = both()
        while peek() == "OR":
            take()
            options += both()
        return options

    options = either()
    if peek() is not None:
        raise PolicyError(f"Invalid SPDX expression: {expression}")
    return options


def allowed_option(expression):
    for option in license_options(expression):
        if option <= ALLOWED:
            return option
    return None


def cargo_metadata(root, platform):
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", platform],
        cwd=root, check=True, capture_output=True, text=True, encoding="utf-8")
    return json.loads(result.stdout)


def shipped_packages(metadata):
    """Packages reachable from workspace members through normal or build edges."""
    members = set(metadata["workspace_members"])
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    seen, pending = set(), list(members)
    while pending:
        current = pending.pop()
        if current in seen:
            continue
        seen.add(current)
        for dep in nodes[current]["deps"]:
            if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]):
                pending.append(dep["pkg"])
    packages = {package["id"]: package for package in metadata["packages"]}
    return [packages[i] for i in seen - members]


def normalize(text):
    lines = text.replace("\r\n", "\n").replace("\r", "\n").split("\n")
    return "\n".join(line.rstrip() for line in lines).strip("\n") + "\n"


def read_text(path):
    return normalize(path.read_bytes().decode("utf-8", errors="replace"))


def standard_text(root, name, package):
    if name == "MIT":
        holders = ", ".join(package.get("authors") or []) or f"The {package['name']} authors"
        body = read_text(root / "LICENSE-MIT").split("\n", 3)[3]
        return f"MIT License\n\nCopyright (c) {holders}\n{body}"
    if name == "Apache-2.0":
        return read_text(root / "LICENSE-APACHE")
    return read_text(root / "scripts" / "license-texts" / f"{name}.txt")


def package_notices(root, package):
    directory = Path(package["manifest_path"]).parent
    own = sorted(p.name for p in directory.iterdir() if p.is_file() and NOTICE_FILE.match(p.name))
    notices = []
    for name in own + list(EXTRA_NOTICES.get(package["name"], ())):
        path = directory / name
        if not path.is_file():
            raise PolicyError(f"{package['name']} {package['version']}: missing {name}")
        notices.append(read_text(path))
    if not own:
        # No license file in the package root: add the standard text of the
        # preferred allowed license; extra notices cover any other components.
        choice = None
        for option in license_options(package["license"] or ""):
            if option <= ALLOWED:
                choice = next((name for name in STANDARD_TEXTS if name in option), None)
                if choice:
                    break
        if choice is None:
            raise PolicyError(f"{package['name']} {package['version']} ships no license file and "
                              f"'{package['license']}' has no bundled standard text")
        notices.append(standard_text(root, choice, package))
    return notices


def collect(root, metadata_by_platform):
    packages, errors = {}, []
    for metadata in metadata_by_platform:
        for package in shipped_packages(metadata):
            packages[(package["name"], package["version"])] = package
    for key in sorted(packages):
        package = packages[key]
        source = package.get("source") or ""
        if not source.startswith("registry+"):
            errors.append(f"{key[0]} {key[1]}: unsupported source {source or 'path'}")
        expression = package.get("license")
        if not expression:
            errors.append(f"{key[0]} {key[1]}: no SPDX license expression")
            continue
        try:
            if allowed_option(expression) is None:
                errors.append(f"{key[0]} {key[1]}: license '{expression}' is not allowed")
        except PolicyError as error:
            errors.append(f"{key[0]} {key[1]}: {error}")
    if errors:
        raise PolicyError("\n".join(errors))
    return [packages[key] for key in sorted(packages)]


def render(root, packages):
    texts, uses, rows = {}, {}, []
    for package in packages:
        label = f"{package['name']} {package['version']}"
        ids = []
        for notice in package_notices(root, package):
            digest = hashlib.sha256(notice.encode("utf-8")).hexdigest()
            texts.setdefault(digest, notice)
            uses.setdefault(digest, []).append(label)
            ids.append(digest)
        rows.append((label, package["license"], ids))
    order = {digest: f"T{index:03}" for index, digest in
             enumerate(sorted(texts, key=lambda d: (uses[d][0], d)), start=1)}
    out = [
        "ModuleEngine third-party notices",
        "",
        "ModuleEngine is licensed under MIT OR Apache-2.0 (see LICENSE-MIT and",
        "LICENSE-APACHE). Its Windows and Linux binaries include the Rust packages",
        "listed below. Each package is used under the license stated for it; the",
        "notice texts the packages ship follow the list.",
        "",
        "Generated by scripts/third_party_licenses.py from Cargo.lock. Do not edit.",
        "",
        f"Packages ({len(rows)}):",
        "",
    ]
    for label, expression, ids in rows:
        out.append(f"{label} | {expression} | {', '.join(sorted(order[d] for d in ids))}")
    for digest in sorted(texts, key=order.get):
        out += ["", "=" * 78, f"{order[digest]}: {', '.join(uses[digest])}", "=" * 78, "",
                texts[digest].rstrip("\n")]
    return "\n".join(out) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="fail if the committed file differs or a license is not allowed")
    args = parser.parse_args()
    try:
        packages = collect(ROOT, [cargo_metadata(ROOT, p) for p in PLATFORMS])
        rendered = render(ROOT, packages)
    except PolicyError as error:
        print(f"Third-party license policy failed:\n{error}", file=sys.stderr)
        return 1
    target = ROOT / OUTPUT
    if args.check:
        current = target.read_bytes().decode("utf-8") if target.is_file() else ""
        if current != rendered:
            print(f"{OUTPUT} is stale; run: python scripts/third_party_licenses.py", file=sys.stderr)
            return 1
        print(f"{OUTPUT}: {len(packages)} packages, licenses allowed, file current")
        return 0
    target.write_bytes(rendered.encode("utf-8"))
    print(f"Wrote {OUTPUT}: {len(packages)} packages")
    return 0


if __name__ == "__main__":
    sys.exit(main())
