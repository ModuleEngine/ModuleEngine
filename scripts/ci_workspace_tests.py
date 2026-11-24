"""Run the CI workspace tests and report libtest results by Cargo package."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

from python_runtime import require_python
from verify import VerificationError, test_skips


MIGRATION_TEST = "workspace_files::tests::migration_copy_accepts_case_distinct_filename_and_preserves_legacy_source"
ALLOCATION_TEST = "vulkan::tests::failed_partial_native_allocations_release_owned_handles_and_device_recovers"
SUMMARY = re.compile(r"^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;")
CASE = re.compile(r"^test (.+?) \.\.\. (.*)$")
FORBIDDEN_PLAYER_PACKAGES = {
    "module-engine-arpg", "module-engine-project-puzzle", "module-engine-g1",
    "module-engine-two-arenas", "module-engine-vulkan", "ash",
}


def passed_cases(suite):
    names = set()
    running = None
    for line in suite:
        if line in ("successes:", "failures:"):
            break
        if match := CASE.fullmatch(line):
            running, status = match.groups()
        else:
            status = line
        # Native Vulkan diagnostics can write to stderr between libtest's
        # test name and verdict, even with --show-output.
        if status == "ok" and running is not None:
            names.add(running)
        if status in ("ok", "FAILED", "ignored") or status.startswith("ignored, "):
            running = None
    return names


def report(lines, package_names):
    counts = {name: dict(passed=0, failed=0, ignored=0, capability_skips={})
              for name in sorted(package_names.values())}
    executables, doc_targets, executed = {}, {}, set()
    package = None
    suite = []

    def finish_suite():
        summaries = [match for line in suite if (match := SUMMARY.match(line))]
        if not summaries:
            return
        # --show-output replays captured output before the enclosing summary.
        # A test may run a child libtest and print that child's summary.
        summary = summaries[-1]
        if package is None:
            raise VerificationError(f"No Cargo package for test summary: {summary[0]}")
        passed, failed, ignored = map(int, summary.groups())
        names = passed_cases(suite)
        skipped = test_skips("\n".join(suite), names)
        row = counts[package]
        row["passed"] += passed - len(skipped)
        row["failed"] += failed
        row["ignored"] += ignored
        row["capability_skips"].update(skipped)
        executed.update((package, name) for name in names - skipped.keys())

    for raw in lines:
        line = raw.rstrip("\r\n")
        if line.startswith('{"reason":'):
            artifact = json.loads(line)
            name = package_names.get(artifact.get("package_id"))
            if name and artifact["reason"] == "compiler-artifact":
                if artifact.get("executable") and artifact["profile"]["test"]:
                    executables[Path(artifact["executable"]).name] = name
                if "lib" in artifact["target"]["kind"]:
                    doc_targets[artifact["target"]["name"]] = name
        else:
            next_package = None
            if match := re.search(r"^\s*Running .+ \((.+)\)$", line):
                next_package = executables.get(Path(match[1]).name)
            elif match := re.fullmatch(r"\s*Doc-tests (\S+)", line):
                next_package = doc_targets.get(match[1])
            if next_package is None:
                suite.append(line)
            else:
                finish_suite()
                package, suite = next_package, []
    finish_suite()
    return counts, executed


def workspace(output):
    output.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, CARGO_TERM_COLOR="never", RUST_TEST_THREADS="1",
               MODULE_ENGINE_VERIFY_SERIAL="1")
    # Isolated SDK fixtures still use the same dependency build cache. Script
    # keeps its worker products under this target's custom-workers directory.
    env.setdefault("CARGO_TARGET_DIR", str(Path("target").resolve()))
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], env=env))
    names = {item["id"]: item["name"] for item in metadata["packages"]
             if item["id"] in metadata["workspace_members"]}
    # Limit the parent Cargo's debug info without exporting a profile override
    # into Custom's worker environment, where it would invalidate reuse proofs.
    command = ["cargo", "test", "--workspace", "--locked", "--no-fail-fast",
               "--config", "profile.test.debug=0",
               "--message-format=json-render-diagnostics", "--", "--show-output"]
    print(" ".join(command), flush=True)
    log = output / "workspace.log"
    with log.open("w", encoding="utf-8") as stream, subprocess.Popen(
        command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        encoding="utf-8", errors="replace", env=env
    ) as process:
        for line in process.stdout:
            stream.write(line)
            if not line.startswith(('{"reason":', "Vulkan debug")):
                print(line, end="", flush=True)
        code = process.wait()
    with log.open(encoding="utf-8") as stream:
        counts, executed = report(stream, names)
    (output / "workspace.json").write_text(
        json.dumps({"cargo_exit": code, "packages": counts}, indent=2) + "\n", encoding="utf-8")
    rows = ["| Package | Passed | Failed | Ignored | Capability skips |", "| --- | ---: | ---: | ---: | ---: |"]
    rows.extend(f"| {name} | {row['passed']} | {row['failed']} | {row['ignored']} | {len(row['capability_skips'])} |"
                for name, row in counts.items())
    summary = "\n".join(rows) + "\n"
    print(summary, end="")
    if summary_path := os.environ.get("GITHUB_STEP_SUMMARY"):
        with Path(summary_path).open("a", encoding="utf-8") as stream:
            stream.write(summary)
    if code != 0:
        return code
    if not executed:
        raise VerificationError("The workspace run did not report any executed tests")
    with log.open(encoding="utf-8") as stream:
        if any("Vulkan validation ERROR:" in line for line in stream):
            raise VerificationError("Workspace tests reported Vulkan validation errors")
    if any(reason.startswith("Custom SDK:") for row in counts.values()
           for reason in row["capability_skips"].values()):
        raise VerificationError("CI fetched the lockfile but a Custom SDK test reported missing offline dependencies")
    if sys.platform == "linux":
        required = {("module-engine-editor", MIGRATION_TEST), ("module-engine-vulkan", ALLOCATION_TEST)}
        if missing := required - executed:
            raise VerificationError(f"Required Linux regressions did not execute: {sorted(missing)}")
    return 0


def without_dxc(output):
    env = os.environ.copy()
    env.pop("MODULE_ENGINE_DXC", None)
    if shutil.which("dxc", path=env.get("PATH")) or any(
        Path(path).exists() for path in ("tools/dxc/bin/dxc", "tools/dxc/dxc")
    ):
        raise VerificationError("The no-DXC step requires a checkout and PATH without DXC")
    command = ["cargo", "test", "--locked", "--config", "profile.test.debug=0",
               "-p", "module-engine-vulkan", "--lib",
               ALLOCATION_TEST, "--", "--exact", "--nocapture"]
    print(" ".join(command), flush=True)
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            encoding="utf-8", errors="replace", env=env)
    output.mkdir(parents=True, exist_ok=True)
    (output / "without-dxc.log").write_text(result.stdout, encoding="utf-8")
    print(result.stdout, end="")
    result.check_returncode()
    skipped = test_skips(result.stdout, [ALLOCATION_TEST])
    reason = skipped.get(ALLOCATION_TEST, "")
    if ("running 1 test" not in result.stdout.splitlines() or
            not re.search(r"^test result: ok\. 1 passed; 0 failed; 0 ignored;", result.stdout, re.MULTILINE)
            or not reason.startswith("DXC: No native DXC compiler") or " | " in reason):
        raise VerificationError("Expected one allocation test to skip specifically because DXC is absent")


def check_minimal_player():
    command = ["cargo", "tree", "--locked", "-p", "module-engine-player", "--no-default-features",
               "--edges", "normal,build", "--prefix", "none", "--format", "{p}"]
    tree = subprocess.check_output(command, encoding="utf-8")
    print(tree, end="")
    packages = {line.split()[0] for line in tree.splitlines() if line.strip()}
    if forbidden := packages & FORBIDDEN_PLAYER_PACKAGES:
        raise VerificationError("Minimal Player includes a game or Vulkan: " + ", ".join(sorted(forbidden)))


def main():
    require_python()
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--without-dxc", action="store_true")
    mode.add_argument("--check-minimal-player", action="store_true")
    parser.add_argument("--output", type=Path, default=Path("outputs/verification/ci-workspace"))
    args = parser.parse_args()
    try:
        if args.without_dxc:
            without_dxc(args.output)
        elif args.check_minimal_player:
            check_minimal_player()
        else:
            return workspace(args.output)
    except (OSError, VerificationError, subprocess.CalledProcessError) as error:
        print(f"CI workspace tests: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
