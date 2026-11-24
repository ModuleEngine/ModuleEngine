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
    def test_docs_only_omits_expensive_routes(self):
        selected = ci_plan.select(["docs/current-status.md", "README.md", ".gitignore"])
        self.assertFalse(selected["native"])
        self.assertFalse(selected["package"])

    def test_source_deleted_renamed_and_unknown_paths_require_both_routes(self):
        for path in ["crates/editor/src/game_ui.rs", "Cargo.lock", "scripts/verify.py",
                     "assets/texture.png", "game-project-v4.json", ".cargo/config.toml",
                     ".github/workflows/ci.yml", "future/new-source"]:
            self.assertTrue(ci_plan.select([path])["native"], path)
            self.assertTrue(ci_plan.select([path])["package"], path)

    def test_initial_and_manual_run_require_every_route(self):
        with patch("ci_plan.subprocess.run") as process:
            self.assertTrue(ci_plan.select(ci_plan.changed_paths("0" * 40))["native"])
            self.assertTrue(ci_plan.select(ci_plan.changed_paths(""))["package"])
            process.assert_not_called()

    def test_diff_uses_nul_separation_no_rename_and_no_shell(self):
        with patch("ci_plan.subprocess.run", return_value=subprocess.CompletedProcess([], 0, b"docs/a.md\0crates/space name.rs\0")) as process:
            self.assertEqual(ci_plan.changed_paths("a" * 40), ["crates/space name.rs", "docs/a.md"])
            argv = process.call_args.args[0]
            self.assertIn("-z", argv)
            self.assertIn("--no-renames", argv)
            self.assertNotIn("shell", process.call_args.kwargs)
        with self.assertRaises(ValueError):
            ci_plan.changed_paths("--all")

    def test_diff_failure_never_becomes_docs_only_success(self):
        with patch("ci_plan.subprocess.run", side_effect=subprocess.CalledProcessError(1, "git")):
            with self.assertRaises(subprocess.CalledProcessError):
                ci_plan.changed_paths("a" * 40)

    def test_gate_rejects_every_non_success_prerequisite(self):
        normal = {"native": "true", "package": "true", "roadmap_only": "false"}
        light = {"native": "false", "package": "false", "roadmap_only": "true"}
        ci_plan.check_gate("success", "success", "skipped", normal)
        ci_plan.check_gate("success", "skipped", "success", light)
        for result in ["failure", "cancelled", "skipped", "", None]:
            for planning, checks, roadmap, selected in [
                (result, "success", "skipped", normal), ("success", result, "skipped", normal),
                (result, "skipped", "success", light), ("success", "skipped", result, light),
                ("success", result, "success", normal),
            ]:
                with self.assertRaises(ValueError):
                    ci_plan.check_gate(planning, checks, roadmap, selected)
        for selected in [{}, {**normal, "native": "TRUE"}, {**normal, "roadmap_only": ""},
                         {**light, "native": "true"}, {**light, "package": "true"}]:
            with self.assertRaises(ValueError):
                ci_plan.check_gate("success", "skipped", "success", selected)

    def test_workflow_keeps_unconditional_gate_and_pinned_read_only_actions(self):
        # Structure check complements policy tests; Actions still needs its first real run.
        text = (Path(__file__).resolve().parent.parent / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.assertNotIn("pull_request_target", text)
        self.assertNotIn("self-hosted", text)
        self.assertNotIn("continue-on-error", text)
        self.assertIn("    if: always()\n    needs: [plan, checks, roadmap_status]", text)
        self.assertIn("    if: needs.plan.outputs.roadmap_only == 'false'", text)
        self.assertIn("    if: needs.plan.outputs.roadmap_only == 'true'", text)
        self.assertIn("  contents: read", text)
        self.assertIn("  merge_group:", text)
        self.assertNotIn("CARGO_PROFILE_TEST_DEBUG:", text)
        self.assertNotIn("CARGO_PROFILE_DEV_DEBUG:", text)
        self.assertIn("--component rustfmt --component clippy", text)
        self.assertIn("run: python scripts/verify.py --profile clippy --plan", text)
        self.assertIn("run: python scripts/verify.py --profile clippy --timeout 1800", text)
        for line in text.splitlines():
            if "uses:" in line:
                self.assertRegex(line, r"uses: actions/[a-z-]+@[0-9a-f]{40} # v")

    def test_cache_never_controls_fetch_verification_or_artifact_reporting(self):
        text = (Path(__file__).resolve().parent.parent / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        self.assertNotIn("cache-hit", text)
        self.assertNotIn("lookup-only", text)
        self.assertNotIn("save-always", text)
        self.assertIn("actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6", text)
        cache = text.split("      - name: Restore compatible Cargo", 1)[1].split("      - name:", 1)[0]
        self.assertIn("if: steps.cargo-cache-context.outputs.enabled == 'true'", cache)
        self.assertEqual(cache.count("restore-keys:"), 1)
        paths = cache.split("          path: |\n", 1)[1].splitlines()
        self.assertEqual([p.strip() for p in paths], ["${{ env.CARGO_HOME }}/registry", "${{ env.CARGO_HOME }}/git",
                         "target/debug", "target/release", "target/${{ steps.cargo-cache-context.outputs.native_host }}/debug",
                         "target/${{ steps.cargo-cache-context.outputs.native_host }}/release"])
        self.assertLess(text.index("run: python scripts/ci_cache.py"), text.index("run: cargo fetch --locked"))
        for block in text.split("      - name: ")[1:]:
            if any(command in block for command in ["run: cargo fetch --locked", "run: python scripts/verify.py --profile contracts",
                                                   "run: python scripts/verify.py --profile clippy"]):
                self.assertNotIn("        if:", block.split("      - ", 1)[0])
        self.assertIn("if: runner.os == 'Windows' && needs.plan.outputs.native == 'true'", text)
        self.assertIn("if: runner.os == 'Linux' && needs.plan.outputs.native == 'true'", text)
        self.assertIn("if: needs.plan.outputs.package == 'true'", text)

    def test_full_matrix_fetches_before_workspace_and_separate_player_checks(self):
        text = (Path(__file__).resolve().parent.parent / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        commands = [
            "cargo fetch --locked", "scripts/ci_workspace_tests.py --without-dxc",
            "scripts/ci_linux_dxc.py", "run: python scripts/ci_workspace_tests.py\n",
            "scripts/ci_workspace_tests.py --check-minimal-player",
            "cargo build --locked -p module-engine-player --no-default-features",
            "cargo test --locked -p module-engine-player --no-default-features",
        ]
        self.assertEqual(sorted(text.index(command) for command in commands),
                         [text.index(command) for command in commands])
        for block in text.split("      - name: ")[1:]:
            if any(command in block for command in commands[3:]):
                self.assertNotIn("        if:", block.split("      - ", 1)[0])

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
            "test works ... ok", "test needs_gpu ... ok", "test ignored ... ignored",
            "successes:", "---- works stdout ----", "running 1 test", "test child_case ... ok",
            "test result: ok. 1 passed; 0 failed; 0 ignored;", "---- needs_gpu stdout ----",
            "MODULEENGINE_TEST_SKIP\tneeds_gpu\tVulkan: no loader",
            "test result: ok. 2 passed; 0 failed; 1 ignored;",
            "   Doc-tests test", "test doc_example ... ok",
            "test result: FAILED. 1 passed; 1 failed; 0 ignored;",
        ]
        counts, executed = ci_workspace_tests.report(log, {"crate#test": "crate"})
        self.assertEqual(counts["crate"], {"passed": 2, "failed": 1, "ignored": 1,
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
        with patch.object(ci_plan, "changed_paths", return_value=list(before) if paths is None else paths), \
                patch.object(ci_plan, "read_roadmaps", side_effect=(before, after)):
            return ci_plan.plan("a" * 40)

    def test_bilingual_status_only_and_estimates(self):
        for en, ru in [("Done", "Сделано"), ("In progress (1.5–2 w)", "В работе (1,5–2 н)"),
                       ("3 d", "3 д")]:
            plan = self.route(self.roadmaps(), self.roadmaps(en, ru))
            self.assertEqual((plan["roadmap_only"], plan["native"], plan["package"]), (True, False, False))

    def test_empty_single_path_and_mixed_diff_keep_ordinary_route(self):
        for paths in ([], [self.EN], [self.EN, self.RU, "docs/current-status.md"],
                      [self.EN, self.RU, "scripts/roadmap_world.html"], ["<initial-or-manual-run>"],
                      [".github/workflows/ci.yml"]):
            self.assertFalse(self.route(self.roadmaps(), self.roadmaps("Done", "Сделано"), paths)["roadmap_only"], paths)

    def test_non_status_and_ambiguous_edits_keep_ordinary_route(self):
        before, valid = self.roadmaps(), self.roadmaps("Done", "Сделано")
        for transform in (lambda text: text.replace("Criterion", "Other criterion"),
                          lambda text: text.replace("R1.6", "R1.9"),
                          lambda text: text.replace("Work", "Different task"),
                          lambda text: text.replace("### R1", "### R3"),
                          lambda text: text + "| R1.8 | Extra | Check | 2 d |\n",
                          lambda text: text.splitlines(keepends=True)[0],
                          lambda text: text.replace("\n", "\r\n")):
            self.assertFalse(self.route(before, {**valid, self.EN: transform(valid[self.EN])})["roadmap_only"])
        self.assertFalse(self.route(before, before)["roadmap_only"])
        self.assertFalse(self.route(before, {**before, self.EN: valid[self.EN]})["roadmap_only"])
        self.assertFalse(self.route(before, self.roadmaps("Done", "В работе"))["roadmap_only"])
        after = {**valid, self.RU: before[self.RU].replace("2 д", "Сделано")}
        self.assertFalse(self.route(before, after)["roadmap_only"])
        self.assertFalse(self.route(None, valid, list(before))["roadmap_only"])

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
