"""CI path and aggregate policy: no Actions service, Cargo or GPU required."""
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import ci_plan
import ci_linux_dxc
import ci_workspace_tests


class PolicyTests(unittest.TestCase):
    def test_non_pr_docs_and_gitignore_omit_native_and_package(self):
        selected = ci_plan.select_non_pr(["docs/current-status.md", "README.md", ".gitignore"])
        self.assertFalse(selected["native"])
        self.assertFalse(selected["package"])

    def test_source_deleted_renamed_and_unknown_paths_require_both_routes(self):
        for path in ["crates/editor/src/game_ui.rs", "Cargo.lock", "scripts/verify.py",
                     "assets/texture.png", "game-project-v4.json", ".cargo/config.toml",
                     ".github/workflows/ci.yml", "future/new-source"]:
            self.assertTrue(ci_plan.select_non_pr([path])["native"], path)
            self.assertTrue(ci_plan.select_non_pr([path])["package"], path)

    def test_pr_documentation_allowlist_and_mixed_changes(self):
        docs = ["docs/current-status.md", "docs/ru/current-status.md", "README.md",
                "crates/player/README.ru.md", "examples/crate-garden/README.md"]
        for paths, regular, light in [
            (docs, True, True), (list(ci_plan.ROADMAPS), True, True),
            (docs, False, False), ([], True, False),
            *((docs + [extra], True, False) for extra in [
                "docs/tool.py", "docs/editor-ui-tokens.json", "toolchain-manifest.json",
                "scripts/linux-toolchain.json", "verification.json", ".github/workflows/ci.yml",
                "crates/editor/src/main.rs", "future/README.md", "scripts/help.md", ".gitignore",
                "scripts/roadmap_map.py", "scripts/roadmap_world.html", "scripts/roadmap_moves.js"]),
        ]:
            with self.subTest(paths=paths, regular=regular), patch.object(
                    ci_plan, "changed_files", return_value=(paths, regular)):
                selected = ci_plan.plan("a" * 40, event="pull_request")
                self.assertEqual([selected[key] for key in ci_plan.ROUTES], [not light, not light, light])

    def test_non_pr_documentation_keeps_ordinary_matrix(self):
        for event in ("push", "merge_group", "workflow_dispatch"):
            with patch.object(ci_plan, "changed_files", return_value=(["docs/ci.md"], True)):
                selected = ci_plan.plan("a" * 40, event=event)
                self.assertEqual([selected[key] for key in ci_plan.ROUTES], [False, False, False])

    def test_initial_and_manual_run_require_every_route(self):
        with patch("ci_plan.subprocess.run") as process:
            self.assertTrue(ci_plan.select_non_pr(ci_plan.changed_files("0" * 40)[0])["native"])
            self.assertTrue(ci_plan.select_non_pr(ci_plan.changed_files("")[0])["package"])
            process.assert_not_called()

    def test_diff_uses_nul_separation_no_rename_and_no_shell(self):
        with patch("ci_plan.subprocess.run", return_value=subprocess.CompletedProcess([], 0, b":000000 100644 0 a A\0docs/a.md\0:100755 000000 b 0 D\0crates/space name.rs\0")) as process:
            self.assertEqual(ci_plan.changed_files("a" * 40), (["crates/space name.rs", "docs/a.md"], False))
            argv = process.call_args.args[0]
            self.assertIn("-z", argv)
            self.assertIn("--no-renames", argv)
            self.assertNotIn("shell", process.call_args.kwargs)
        with self.assertRaises(ValueError):
            ci_plan.changed_files("--all")

    def test_diff_failure_never_becomes_docs_only_success(self):
        with patch("ci_plan.subprocess.run", side_effect=subprocess.CalledProcessError(1, "git")):
            with self.assertRaises(subprocess.CalledProcessError):
                ci_plan.changed_files("a" * 40)

    def test_gate_rejects_every_non_success_prerequisite(self):
        for docs, native, package in [(True, False, False), (False, True, True),
                                      (False, False, False), (False, True, False)]:
            selected = {"docs_only": str(docs).lower(), "native": str(native).lower(),
                        "package": str(package).lower()}
            needs = {job: {"result": "skipped" if docs else "success"} for job in
                     ("workspace", "release", "player", "clippy", "contracts")}
            for job, enabled in (("documentation", docs), ("native", native), ("package", package)):
                needs[job] = {"result": "success" if enabled else "skipped"}
            needs["plan"] = {"result": "success", "outputs": selected}
            ci_plan.check_gate(needs)
            for job in needs:
                with self.assertRaises(ValueError):
                    ci_plan.check_gate({k: v for k, v in needs.items() if k != job})
                for result in {"success", "failure", "skipped", "cancelled", None} - {needs[job]["result"]}:
                    with self.subTest(job=job, result=result), self.assertRaises(ValueError):
                        ci_plan.check_gate({**needs, job: {**needs[job], "result": result}})
            for invalid in ({}, {**selected, "native": ""}, {**selected, "native": "true", "docs_only": "true"}):
                with self.assertRaises(ValueError):
                    ci_plan.check_gate({**needs, "plan": {"result": "success", "outputs": invalid}})

    def test_parallel_workflow_covers_each_route_and_requires_both_platforms(self):
        root = Path(__file__).resolve().parents[1]
        text = (root / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        setup = (root / ".github/actions/ci-setup/action.yml").read_text(encoding="utf-8")
        self.assertIn("needs: [plan, workspace, release, clippy, contracts, player, native, package, documentation]", text)
        self.assertIn("name: CI required\n    if: always()", text)
        self.assertIn("CI_NEEDS: ${{ toJSON(needs) }}", text)
        self.assertIn("os: [windows-2025, ubuntu-24.04]", text)
        self.assertIn("fail-fast: false", text)
        self.assertIn("  merge_group:", text)
        cases = {
            "workspace": ["--without-dxc", "ci_linux_dxc.py", "ci_workspace_tests.py --nextest"],
            "release": ["cargo build --offline --locked --release", "--test verification_cli"],
            "clippy": ["--profile clippy --plan", "--profile clippy --timeout"],
            "contracts": ["third_party_licenses.py --check", "unittest discover", "--profile contracts --plan",
                          "--profile contracts --timeout", "--manifest-path crates/ecs-spike/Cargo.toml"],
            "player": ["--check-minimal-player", "cargo build --locked -p module-engine-player",
                       "cargo test --locked -p module-engine-player"],
            "native": ["--profile native-tools", "--profile native --require-capabilities"],
            "package": ["--profile package --timeout"],
        }
        for job, required in cases.items():
            block = text.split(f"\n  {job}:\n", 1)[1].split("\n\n  ", 1)[0]
            self.assertIn("strategy: " + ("&os_matrix" if job == "workspace" else "*os_matrix"), block)
            route = job if job in ("native", "package") else "docs_only"
            flag = "true" if route == job else "false"
            self.assertIn(f"if: needs.plan.outputs.{route} == '{flag}'", block)
            self.assertEqual([block.index(cmd) for cmd in required], sorted(block.index(cmd) for cmd in required))
            self.assertIn("uses: ./.github/actions/ci-setup", block)
            self.assertIn("&reports" if job == "workspace" else "*reports", block)
        self.assertIn("name: Upload current-run reports even after failure\n        if: always()", text)
        self.assertIn("cache-targets: false", setup)
        self.assertIn("cache-bin: false", setup)
        self.assertLess(setup.index("ci_cache.py"), setup.index("run: cargo fetch --locked"))
        for workflow in [text, setup]:
            for forbidden in ["continue-on-error", "self-hosted", "pull_request_target", "cache-hit"]:
                self.assertNotIn(forbidden, workflow)
            for line in workflow.splitlines():
                if "uses:" in line and "uses: ./" not in line:
                    self.assertRegex(line, r"uses: [a-zA-Z0-9/-]+@[0-9a-f]{40} # v")

    def test_concurrency_does_not_share_a_group_across_main_runs(self):
        text = (Path(__file__).resolve().parent.parent / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        group = "github.event_name == 'pull_request' && format('pr-{0}', github.event.pull_request.number) || format('run-{0}', github.run_id)"
        self.assertIn("group: module-engine-ci-${{ github.workflow }}-${{ " + group + " }}", text)
        self.assertIn("cancel-in-progress: ${{ github.event_name == 'pull_request' }}", text)
        # GitHub keeps one pending run per group. Exercise both the running
        # and pending decisions for three arrivals in the same workflow.
        for event, expected in (("push", []), ("pull_request", [2, 1])):
            groups, cancelled = {}, []
            for run in (1, 2, 3):
                key = "pr-42" if event == "pull_request" else f"run-{run}"
                active, pending = groups.get(key, (None, None))
                if pending is not None:
                    cancelled.append(pending)
                if active is not None and event == "pull_request" and active not in cancelled:
                    cancelled.append(active)
                groups[key] = (active, run) if active else (run, None)
            self.assertCountEqual(cancelled, expected)


class WorkspaceReportingTests(unittest.TestCase):
    def test_sums_unit_and_doc_results_and_distinguishes_capability_skips(self):
        artifact = {"reason": "compiler-artifact", "package_id": "crate#test",
                    "executable": "/target/debug/deps/test-123",
                    "profile": {"test": True}, "target": {"kind": ["lib"], "name": "test"}}
        log = [
            json.dumps(artifact), "     Running unittests src/lib.rs (/target/debug/deps/test-123)",
            "test works ... Vulkan debug: validation layer unavailable",
            "Vulkan debug named DEVICE: fixture", "ok",
            "test needs_gpu ... ok", "test failed_again ... FAILED", "test ignored ... ignored",
            "successes:", "---- works stdout ----", "running 1 test", "test child_case ... ok",
            "test result: ok. 1 passed; 0 failed; 0 ignored;", "---- needs_gpu stdout ----",
            "MODULEENGINE_TEST_SKIP\tneeds_gpu\tVulkan: no loader",
            "test result: FAILED. 2 passed; 1 failed; 1 ignored;",
            "   Doc-tests test", "test doc_example ... ok",
            "test result: FAILED. 1 passed; 1 failed; 0 ignored;",
        ]
        counts, executed = ci_workspace_tests.report(log, {"crate#test": "crate"})
        self.assertEqual(counts["crate"], {"passed": 2, "failed": 2, "ignored": 1,
                                          "capability_skips": {"needs_gpu": "Vulkan: no loader"}})
        self.assertEqual(executed, {("crate", "works"), ("crate", "doc_example")})

    def test_missing_cargo_attribution_cannot_turn_into_a_green_report(self):
        with self.assertRaisesRegex(ci_workspace_tests.VerificationError, "No Cargo package"):
            ci_workspace_tests.report(["test result: ok. 1 passed; 0 failed; 0 ignored;"], {})

    def test_minimal_player_rejects_game_and_vulkan_dependencies(self):
        with patch("ci_workspace_tests.subprocess.check_output",
                   return_value="module-engine-player v0.4.0\nmodule-engine-host v0.4.0\n"):
            ci_workspace_tests.check_minimal_player()
        for dependency in ("module-engine-vulkan", "module-engine-project-puzzle", "module-engine-two-arenas"):
            with patch("ci_workspace_tests.subprocess.check_output", return_value=f"{dependency} v0.4.0\n"):
                with self.assertRaisesRegex(ci_workspace_tests.VerificationError, dependency):
                    ci_workspace_tests.check_minimal_player()


class RoadmapRouteTests(unittest.TestCase):
    EN, RU = ci_plan.ROADMAPS

    @classmethod
    def roadmaps(cls, en="1.5–2 w", ru="1,5–2 н"):
        row = "| R1.6 | Work \\| details | Criterion | {} |\n"
        other = "| R1.7 | More work | Another criterion | {} |\n"
        heading = "### R1 — Phase\n\n"
        return {cls.EN: heading + row.format(en) + other.format("2 d"),
                cls.RU: heading + row.format(ru) + other.format("2 д")}

    def route(self, before, after, paths=None):
        with patch.object(ci_plan, "changed_files", return_value=(list(before) if paths is None else paths, True)), \
                patch.object(ci_plan, "read_roadmaps", side_effect=(before, after)):
            return ci_plan.plan("a" * 40)

    def test_bilingual_status_only_and_estimates(self):
        for en, ru in [("Done", "Сделано"), ("In progress (1.5–2 w)", "В работе (1,5–2 н)"),
                       ("3 d", "3 д")]:
            plan = self.route(self.roadmaps(), self.roadmaps(en, ru))
            self.assertEqual((plan["docs_only"], plan["native"], plan["package"]), (True, False, False))

    def test_empty_single_path_and_mixed_diff_keep_ordinary_route(self):
        for paths in ([], [self.EN], [self.EN, self.RU, "docs/current-status.md"],
                      [self.EN, self.RU, "scripts/roadmap_world.html"], ["<initial-or-manual-run>"],
                      [".github/workflows/ci.yml"]):
            self.assertFalse(self.route(self.roadmaps(), self.roadmaps("Done", "Сделано"), paths)["docs_only"], paths)

    def test_non_status_and_ambiguous_edits_keep_ordinary_route(self):
        before, valid = self.roadmaps(), self.roadmaps("Done", "Сделано")
        for transform in (lambda text: text.replace("Criterion", "Other criterion"),
                          lambda text: text.replace("R1.6", "R1.9"),
                          lambda text: text.replace("Work", "Different task"),
                          lambda text: text.replace("### R1", "### R3"),
                          lambda text: text + "| R1.8 | Extra | Check | 2 d |\n",
                          lambda text: text.splitlines(keepends=True)[0],
                          lambda text: text.replace("\n", "\r\n")):
            self.assertFalse(self.route(before, {**valid, self.EN: transform(valid[self.EN])})["docs_only"])
        self.assertFalse(self.route(before, before)["docs_only"])
        self.assertFalse(self.route(before, {**before, self.EN: valid[self.EN]})["docs_only"])
        self.assertFalse(self.route(before, self.roadmaps("Done", "В работе"))["docs_only"])
        after = {**valid, self.RU: before[self.RU].replace("2 д", "Сделано")}
        self.assertFalse(self.route(before, after)["docs_only"])
        self.assertFalse(self.route(None, valid, list(before))["docs_only"])

    def test_invalid_values_fail_instead_of_becoming_todo(self):
        for status in ("Implemented", "Done-ish", "In progres", "In progress (soon)", "1.5 m", ""):
            with self.subTest(status=status), self.assertRaisesRegex(ValueError, "R1.6.*unsupported"):
                self.route(self.roadmaps(), self.roadmaps(status, "В работе"))


class DxcRestoreTests(unittest.TestCase):
    def setUp(self):
        owned = Path(__file__).resolve().parent.parent / "work/verification-tests"
        owned.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="ci-dxc-", dir=owned)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.assertTrue(self.root.is_relative_to(owned.resolve()))
        (self.root / "scripts").mkdir()
        self.archive = self.root / "source.tar.gz"

    def fixture(self, member="bin/dxc", payload=b"pinned compiler"):
        with tarfile.open(self.archive, "w:gz") as stream:
            info = tarfile.TarInfo(member)
            info.size = len(payload)
            stream.addfile(info, io.BytesIO(payload))
        pin = {"dxc": {"url": "https://example.invalid/fixture", "sha256": ci_linux_dxc.sha256(self.archive),
                       "files": {"tools/dxc/bin/dxc": hashlib.sha256(payload).hexdigest()}}}
        (self.root / "scripts/linux-toolchain.json").write_text(json.dumps(pin), encoding="utf-8")
        return pin

    def download(self, _url, destination):
        shutil.copyfile(self.archive, destination)

    def test_archive_and_file_pins_and_no_existing_destination_overwrite(self):
        self.fixture()
        ci_linux_dxc.restore(self.root, self.download)
        self.assertEqual((self.root / ".tools/ci-dxc/bin/dxc").read_bytes(), b"pinned compiler")
        with self.assertRaises(FileExistsError):
            ci_linux_dxc.restore(self.root, self.download)

    def test_corrupt_archive_fails_before_extracting(self):
        self.fixture()
        self.archive.write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "archive checksum"):
            ci_linux_dxc.restore(self.root, self.download)
        self.assertFalse((self.root / ".tools/ci-dxc/bin/dxc").exists())

    def test_corrupt_component_and_tar_traversal_are_rejected(self):
        pin = self.fixture()
        pin["dxc"]["files"]["tools/dxc/bin/dxc"] = "0" * 64
        (self.root / "scripts/linux-toolchain.json").write_text(json.dumps(pin), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "component checksum"):
            ci_linux_dxc.restore(self.root, self.download)
        # Separate owned root so rejection cannot be caused by the existing destination.
        nested = self.root / "traversal"
        (nested / "scripts").mkdir(parents=True)
        pin = self.fixture(member="../outside.txt")
        (nested / "scripts/linux-toolchain.json").write_text(json.dumps(pin), encoding="utf-8")
        with self.assertRaises(tarfile.FilterError):
            ci_linux_dxc.restore(nested, self.download)
        self.assertFalse((nested / ".tools/outside.txt").exists())


if __name__ == "__main__":
    unittest.main()
