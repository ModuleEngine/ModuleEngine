"""CI path and aggregate policy: no Actions service, Cargo or GPU required."""
from contextlib import ExitStack
import hashlib
import io
import json
import os
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
    @staticmethod
    def tracked():
        root = Path(__file__).resolve().parent.parent
        return subprocess.run(["git", "ls-files", "-z"], cwd=root, check=True, capture_output=True
                              ).stdout.decode("utf-8").split("\0")[:-1]

    @staticmethod
    def route(paths, regular=True):
        with patch.object(ci_plan, "changed_files", return_value=(dict.fromkeys(paths), regular)):
            return ci_plan.plan("a" * 40)

    def test_every_tracked_file_has_an_explicit_route(self):
        rules = ci_plan.load_rules()
        missing = [path for path in self.tracked() if ci_plan.classify(path, rules) is None]
        self.assertEqual(missing, [], "Add a rule to .github/ci-routes.json for these paths")

    def test_one_folder_is_one_route_except_documented_readmes(self):
        rules, folders = ci_plan.load_rules(), {}
        for path in self.tracked():
            top = path.split("/", 1)[0] if "/" in path else ""
            folders.setdefault(top, set()).add(ci_plan.classify(path, rules))
        self.assertEqual(folders["site"], {"site"})
        for folder in ("scripts", ".github", ".agents", ".codex", ".config", "tools", "assets"):
            self.assertEqual(folders.get(folder, {"full"}), {"full"}, folder)
        # docs/ holds Markdown plus data files; crates/ and examples/ hold only README text beside code.
        for folder in ("docs", "crates", "examples"):
            self.assertLessEqual(folders[folder], {"docs", "full"}, folder)
        self.assertNotIn("outputs", folders)
        self.assertNotIn("work", folders)

    def test_light_routes_skip_native_and_package(self):
        docs = ["docs/current-status.md", "docs/ru/current-status.md", "README.md", "AGENTS.md",
                "crates/player/README.ru.md", "examples/crate-garden/README.md"]
        for paths, route in [(docs, "docs"), (docs + ["site/roadmap_map.py"], "site"),
                             (["site/roadmap_world.html", "site/test_roadmap_map.py"], "site")]:
            selected = self.route(paths)
            self.assertEqual((selected["route"], selected["light"], selected["native"], selected["package"]),
                             (route, True, False, False), paths)
            self.assertEqual(selected["triggers"], [])

    def test_any_code_unknown_or_irregular_change_runs_the_full_matrix(self):
        light = ["docs/current-status.md", "site/roadmap_map.py"]
        for extra in ["crates/editor/src/game_ui.rs", "Cargo.lock", "scripts/verify.py", "scripts/test_ci.py",
                      "assets/texture.png", "game-project-v4.json", ".cargo/config.toml", "docs/editor-ui-tokens.json",
                      ".github/workflows/ci.yml", ".github/ci-routes.json", "docs/tool.py", "verify.sh",
                      "toolchain-manifest.json", "LICENSE-MIT", ".gitignore", "tools/dxc/LICENSE-MS.txt",
                      "future/new-source", "future/README.md", "work/notes.md", "outputs/report.html",
                      "crates/editor/src/README.md", "scripts/help.md"]:
            selected = self.route(light + [extra])
            self.assertEqual((selected["route"], selected["light"], selected["native"], selected["package"]),
                             ("full", False, True, True), extra)
            self.assertEqual(selected["triggers"], [extra])
        self.assertEqual(self.route(light, regular=False)["route"], "full")
        self.assertEqual(self.route([])["route"], "full")

    def test_unknown_paths_are_reported_as_unclassified(self):
        selected = self.route(["docs/a.md", "future/new-source"])
        self.assertEqual((selected["route"], selected["unclassified"]), ("full", ["future/new-source"]))

    def test_one_rule_for_every_event_uses_the_union_against_the_base(self):
        # pull_request, merge_group and push all call plan(base); a group or push carries the union of its changes.
        self.assertEqual(self.route(["docs/ci.md", "docs/ru/ci.md"])["route"], "docs")
        self.assertEqual(self.route(["docs/ci.md", "crates/core/src/lib.rs"])["route"], "full")
        import inspect
        self.assertNotIn("event", inspect.signature(ci_plan.plan).parameters)

    def test_glob_semantics(self):
        rules = [("a", ci_plan.glob_regex(pattern)) for pattern in ("docs/**/*.md", "*.json", "site/**")]
        for path, expected in [("docs/a.md", True), ("docs/ru/x/a.md", True), ("docs/a.txt", False),
                               ("a.json", True), ("dir/a.json", False), ("site/x/y", True), ("sitemap", False)]:
            self.assertEqual(any(regex.fullmatch(path) for _, regex in rules), expected, path)

    def test_summary_names_the_route_and_the_triggering_files(self):
        text = ci_plan.summary(self.route(["docs/a.md", "crates/x/src/lib.rs"]))
        self.assertIn("CI route: `full`", text)
        self.assertIn("`crates/x/src/lib.rs`", text)
        self.assertNotIn("`docs/a.md`", text)
        text = ci_plan.summary(self.route(["site/roadmap_map.py"]))
        self.assertIn("CI route: `site`", text)
        self.assertIn("`site/roadmap_map.py`", text)

    def test_initial_and_manual_run_require_every_route(self):
        with patch("ci_plan.subprocess.run") as process:
            self.assertEqual(ci_plan.select(*ci_plan.changed_files("0" * 40))["route"], "full")
            self.assertEqual(ci_plan.select(*ci_plan.changed_files(""))["route"], "full")
            process.assert_not_called()

    def test_diff_uses_nul_separation_no_rename_and_no_shell(self):
        with patch("ci_plan.subprocess.run", return_value=subprocess.CompletedProcess([], 0, b":000000 100644 0 a A\0docs/a.md\0:100755 000000 b 0 D\0crates/space name.rs\0")) as process:
            self.assertEqual(ci_plan.changed_files("a" * 40), ({"crates/space name.rs": None, "docs/a.md": None}, False))
            argv = process.call_args.args[0]
            self.assertIn("-z", argv)
            self.assertIn("--no-renames", argv)
            self.assertNotIn("shell", process.call_args.kwargs)
        with self.assertRaises(ValueError):
            ci_plan.changed_files("--all")

    def test_diff_failure_never_becomes_a_light_success(self):
        with patch("ci_plan.subprocess.run", side_effect=subprocess.CalledProcessError(1, "git")):
            with self.assertRaises(subprocess.CalledProcessError):
                ci_plan.changed_files("a" * 40)

    def test_policy_cli_runs_checks_before_emitting_outputs(self):
        for route in ("docs", "site", "policy", "full"):
            with ExitStack() as patches:
                patches.enter_context(patch.object(ci_plan, "plan", return_value={"route": route}))
                run = patches.enter_context(patch.object(ci_plan.subprocess, "run"))
                patches.enter_context(patch.object(ci_plan.sys, "argv", ["ci_plan.py"]))
                patches.enter_context(patch.dict(os.environ, {}, clear=True))
                patches.enter_context(patch("builtins.print"))
                ci_plan.main()
                if route == "policy":
                    run.assert_called_once_with(
                        [ci_plan.sys.executable, "-B", str(Path(ci_plan.__file__).with_name("test_ci.py"))],
                        check=True,
                    )
                else:
                    run.assert_not_called()

    def test_gate_rejects_every_non_success_prerequisite(self):
        for route in ("docs", "site", "policy", "full"):
            light = route != "full"
            native = package = not light
            selected = {"light": str(light).lower(), "native": str(native).lower(),
                        "package": str(package).lower(), "route": route}
            needs = {job: {"result": "skipped" if light else "success"} for job in
                     ("workspace", "release", "player", "clippy", "contracts")}
            for job, enabled in (("documentation", True), ("native", native), ("package", package)):
                needs[job] = {"result": "success" if enabled else "skipped"}
            needs["plan"] = {"result": "success", "outputs": selected}
            ci_plan.check_gate(needs)
            for job in needs:
                with self.assertRaises(ValueError):
                    ci_plan.check_gate({k: v for k, v in needs.items() if k != job})
                for result in {"success", "failure", "skipped", "cancelled", None} - {needs[job]["result"]}:
                    with self.subTest(job=job, result=result), self.assertRaises(ValueError):
                        ci_plan.check_gate({**needs, job: {**needs[job], "result": result}})
            if not light:
                for omitted in (("native",), ("package",), ("native", "package")):
                    outputs = {**selected, **dict.fromkeys(omitted, "false")}
                    partial = {**needs, "plan": {"result": "success", "outputs": outputs}}
                    partial.update({job: {"result": "skipped"} for job in omitted})
                    with self.subTest(omitted=omitted), self.assertRaises(ValueError):
                        ci_plan.check_gate(partial)
            flags = {key: value for key, value in selected.items() if key != "route"}
            for invalid in ({}, {**selected, "native": ""}, {**selected, "native": "true", "light": "true"},
                            {**selected, "route": "other"}, {**selected, "route": "full" if light else "site"},
                            flags):
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
        self.assertIn("permissions:\n  contents: read\n", text)
        self.assertIn("--component rustfmt --component clippy", setup)
        self.assertIn('echo "LD_LIBRARY_PATH=$GITHUB_WORKSPACE/.tools/ci-dxc/lib" >> "$GITHUB_ENV"', text)
        for build_override in ("CARGO_PROFILE_TEST_DEBUG:", "CARGO_PROFILE_DEV_DEBUG:"):
            self.assertNotIn(build_override, text.split("\njobs:\n", 1)[0])
        self.assertIn("if: steps.cargo-cache-context.outputs.enabled == 'true'", setup)
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
            route = job if job in ("native", "package") else "light"
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


class PackagePinRouteTests(unittest.TestCase):
    doc = "docs/a.md"
    site = "site/map.html"
    plan_script = "scripts/ci_plan.py"
    test_script = "scripts/test_ci.py"
    code = "scripts/verify.py"

    def route(self, writes, edit_manifest, field="backend_files", profile="full", executable=None,
              extra_pins=(), stale_base_pin=None):
        def git(*args):
            return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout.strip()

        def commit(executable=None):
            git("add", "--all")
            if executable:
                git("update-index", "--chmod=+x", executable)
            git("-c", "user.name=CI Fixture", "-c", "user.email=fixture@example.invalid",
                "-c", "commit.gpgsign=false", "commit", "-qm", "fixture")
            return git("rev-parse", "HEAD")

        def update_pins(manifest, paths):
            for path in paths:
                manifest[field][path] = hashlib.sha256(Path(path).read_bytes()).hexdigest()

        with ExitStack() as fixture:
            directory = fixture.enter_context(tempfile.TemporaryDirectory())
            fixture.callback(os.chdir, Path.cwd())
            os.chdir(directory)
            git("init", "-q")
            git("config", "core.autocrlf", "false")
            initial_paths = (self.doc, self.site, self.plan_script, self.test_script, self.code, *extra_pins)
            for path in initial_paths:
                Path(path).parent.mkdir(parents=True, exist_ok=True)
                Path(path).write_bytes(b"old\n")
            manifest = {"schema_version": 1, "engine_version": "0.4.0", "profile": profile, "files": {},
                        "module_sources": {}, "gameplay_source_ids": [], "blueprint_hash": None,
                        "sdk_files": {}, "backend_files": {}}
            update_pins(manifest, initial_paths)
            if stale_base_pin:
                manifest[field][stale_base_pin] = "0" * 64
            manifest_path = Path(ci_plan.PACKAGE_MANIFEST)
            manifest_path.write_bytes(json.dumps(manifest).encode())
            base = commit()
            for path in writes:
                Path(path).write_bytes(b"new\r\n")
            if edit_manifest is not None:
                update_pins(manifest, writes)
                payload = edit_manifest(manifest, manifest[field])
                manifest_path.write_bytes((payload if payload is not None else json.dumps(manifest)).encode())
            head = commit(executable)
            for path in writes:
                Path(path).write_bytes(b"uncommitted\n")
            manifest_path.write_bytes(b"not json in the working tree")
            return ci_plan.plan(base, head)["route"]

    def test_verified_blob_pins_keep_light_routes(self):
        for field, profile in (("sdk_files", "team"), ("backend_files", "full")):
            for paths, expected in (((self.doc,), "docs"), ((self.doc, self.site), "site"),
                                    ((self.plan_script, self.test_script, self.doc), "policy")):
                with self.subTest(field=field, expected=expected):
                    self.assertEqual(self.route(paths, lambda *_: None, field, profile), expected)
        text_paths = ("README.md", "AGENTS.md", "examples/crate-garden/README.ru.md")
        self.assertEqual(self.route(text_paths, lambda *_: None, extra_pins=text_paths), "docs")
        self.assertEqual(self.route((self.doc,), lambda *_: None, stale_base_pin=self.doc), "docs")

    def test_other_changes_and_unverified_pins_are_full(self):
        no_op = lambda *_: None
        cases = [
            ((self.doc, self.code), no_op, {}),
            ((self.plan_script, self.code), no_op, {}),
            ((self.doc,), no_op, {"profile": "team"}),
            ((self.doc,), no_op, {"executable": self.doc}),
            ((self.doc,), lambda manifest, _: manifest.update(engine_version="0.4.1"), {}),
            ((self.doc,), lambda manifest, _: manifest.update(schema_version=2), {}),
            ((self.doc,), lambda manifest, _: manifest.update(unknown=[]), {}),
            ((self.doc,), lambda manifest, _: json.dumps(manifest)[:-1] + ', "files": {}}', {}),
            ((self.doc,), lambda _, pins: pins.update({self.doc: "0" * 64}), {}),
            ((self.doc,), lambda _, pins: pins.update({self.doc: "invalid"}), {}),
            ((self.doc,), lambda _, pins: pins.update({self.site: "0" * 64}), {}),
            ((self.doc,), lambda _, pins: pins.update({"docs/new.md": "0" * 64}), {}),
            ((self.doc,), lambda _, pins: (pins.pop(self.site), None)[1], {}),
            ((self.doc,), lambda *_: "{", {}),
            ((self.plan_script,), None, {}),
        ]
        for index, (paths, edit, options) in enumerate(cases):
            with self.subTest(case=index):
                self.assertEqual(self.route(paths, edit, **options), "full")
        self.assertEqual(self.route((self.doc,), no_op, executable=ci_plan.PACKAGE_MANIFEST), "full")
        self.assertEqual(self.route(("scripts/other.py",), no_op, extra_pins=("scripts/other.py",)), "full")


class WorkspaceReportingTests(unittest.TestCase):
    def test_nextest_counts_cases_and_verifies_captured_output(self):
        xml = """<testsuites><testsuite name="crate::integration">
        <testcase name="ran"/>
        <testcase name="no_gpu"><system-err>MODULEENGINE_TEST_SKIPno_gpuVulkan: No loader</system-err></testcase>
        <testcase name="ignored"><skipped/></testcase>
        <testcase name="failed"><failure/></testcase>
        </testsuite></testsuites>"""
        counts, executed = ci_workspace_tests.report([], {"id": "crate"})
        ci_workspace_tests.nextest_results(io.StringIO(xml), counts, executed)
        self.assertEqual(executed, {("crate", "ran")})
        self.assertEqual(counts["crate"], {"passed": 1, "failed": 1, "ignored": 1,
                                         "capability_skips": {"no_gpu": "Vulkan: No loader"}})
        for invalid in [xml.replace("SKIPno_gpu", "SKIPno_gpu_child"), xml.replace("SKIPno_gpu", "SKIPother"), xml.replace("crate::", "unknown::"), "<testsuites/>",
                        xml.replace("MODULEENGINE_TEST_SKIP", "Vulkan validation ERROR:")]:
            with self.assertRaises(ci_workspace_tests.VerificationError):
                ci_workspace_tests.nextest_results(io.StringIO(invalid), counts, executed)

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
