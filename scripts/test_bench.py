import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import bench


class CargoArtifactTest(unittest.TestCase):
    def test_explicit_native_target_uses_artifact_even_with_stale_implicit_binary(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            stale = root / "target/release/module-engine-editor.exe"
            stale.parent.mkdir(parents=True)
            stale.write_bytes(b"stale")
            artifact = root / "target/x86_64-pc-windows-msvc/release/module-engine-editor.exe"
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b"built")
            self.assertEqual(self.select(root, artifact), artifact)

    def test_rejects_foreign_target_and_non_bench_binary(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            foreign = root / "target/x86_64-unknown-linux-gnu/release/module-engine-editor"
            with self.assertRaisesRegex(ValueError, "non-native"):
                self.select(root, foreign)
            normal = root / "target/release/module-engine-editor.exe"
            with self.assertRaisesRegex(ValueError, "exactly one bench"):
                self.select(root, normal, features=[])

    @staticmethod
    def select(root, executable, features=("bench",)):
        messages = root / "build.jsonl"
        events = [
            {"reason": "build-finished", "success": True},
            {"reason": "compiler-artifact", "executable": str(executable),
             "features": features, "target": {
                 "name": "module-engine-editor", "kind": ["bin"],
                 "src_path": str(root / "crates/editor/src/main.rs"),
             }},
        ]
        messages.write_text("\n".join(map(json.dumps, events)), encoding="utf-8")
        with patch.object(bench, "ROOT", root):
            return bench.built_editor(messages, "x86_64-pc-windows-msvc")


class PresentationSignatureTest(unittest.TestCase):
    def test_present_mode_and_profiling_are_part_of_each_scene_signature(self):
        for scene in bench.SCENES:
            with self.subTest(scene=scene):
                field = "native_vsync_requested" if scene in ("S1", "S2") else "gui_vsync"
                off = bench.summarize_scene(scene, {
                    field: False, "effective_present_mode": "Fifo", "frame_profiling": False,
                })[3]
                on = bench.summarize_scene(scene, {
                    field: True, "effective_present_mode": "Fifo", "frame_profiling": True,
                })[3]
                missing = bench.summarize_scene(scene, {})[3]
                self.assertIs(off["vsync_requested"], False)
                self.assertIs(on["vsync_requested"], True)
                self.assertIsNone(missing["vsync_requested"])
                self.assertEqual(off["effective_present_mode"], "Fifo")
                self.assertIsNone(missing["effective_present_mode"])
                self.assertIs(off["frame_profiling"], False)
                self.assertIs(on["frame_profiling"], True)


class FrameTimingSummaryTest(unittest.TestCase):
    def test_pass_samples_are_grouped_by_role_and_missing_timings_stay_unavailable(self):
        summary = bench.summarize_frame_timings([
            {"timings": {"gpu": {"status": "unavailable", "reason": "queue unsupported"}}},
            {"timings": {"cpu_submit_ms": 0.2, "gpu": {
                "status": "available", "graph_ms": 3.0, "passes": [
                    {"id": "post", "bypassed": False, "ms": 1.6},
                ],
            }}},
            {"timings": {"gpu": {"status": "available", "graph_ms": 1.0, "passes": [
                {"id": "post", "bypassed": True, "ms": 0.3},
            ]}}},
            {"timings": None},
        ])
        self.assertEqual(summary["profiled_frames"], 3)
        self.assertEqual(summary["gpu_graph_ms"]["mean_ms"], 2.0)
        self.assertEqual(summary["gpu_unavailable_reasons"], {"queue unsupported": 1})
        self.assertIsNone(summary["cpu"]["cpu_tick_ms"])
        self.assertEqual(summary["cpu"]["cpu_submit_ms"]["samples"], 1)
        self.assertEqual(
            [(p["bypassed"], p["mean_ms"]) for p in summary["gpu_pass_ms"]],
            [(False, 1.6), (True, 0.3)],
        )


def recorded_signature(**changes):
    """Shape of an R0.6 S2 signature: stable known present mode, DX12 GUI on the RTX adapter."""
    signature = {"vsync_requested": False, "effective_present_mode": "Immediate",
                 "frame_profiling": False, "vulkan_device": "NVIDIA GeForce RTX 4080 SUPER",
                 "offscreen_render_pixels": [1920, 1080], "measured_requested": 240,
                 "gui_graphics": {"renderer": "wgpu", "backend": "Dx12",
                                  "adapter_name": "NVIDIA GeForce RTX 4080 SUPER"}}
    for key, value in changes.items():
        if key.startswith("gui_"):
            signature["gui_graphics"] = {**signature["gui_graphics"], key[4:]: value}
        else:
            signature[key] = value
    return signature


def scene_case(scene, scale=1.0, peak=500 * 1024 ** 2, status="complete", signature=None, p99=None):
    """A complete per-scene result whose declared metrics are scaled from 10 ms."""
    metrics = {key: {"mean_ms": 10.0 * scale, "p95_ms": 12.0 * scale,
                     "p99_ms": (p99 or 14.0) * scale} for key in bench.GATE_METRICS[scene]}
    return {"scene": scene, "status": status, "metrics": metrics,
            "signature": signature or recorded_signature(),
            "fixture": {"fixture": scene}, "peak_working_set_bytes": peak}


def side_runs(side, scales, scenes=("S2",), first=1, **kwargs):
    return [{"side": side, "run": first + index,
             "results": {scene: scene_case(scene, scale, **kwargs) for scene in scenes}}
            for index, scale in enumerate(scales)]


class NotWorseRuleTest(unittest.TestCase):
    def test_quiet_metric_uses_five_percent_floor_on_medians(self):
        row = bench.judge_metric("timing", [10.0, 10.1, 10.05, 9.95, 10.0],
                                 [10.4, 10.45, 30.0, 10.4, 10.5], True)
        self.assertEqual(row["tolerance_percent"], 5.0)
        self.assertAlmostEqual(row["change_percent"], 4.5)
        self.assertEqual(row["verdict"], "same")  # one 30 ms outlier does not move the median

    def test_noisy_metric_tolerance_is_twice_measured_spread(self):
        baseline = [10.0, 10.5, 11.1, 10.2, 10.4]  # spread 11 % of median 10.4
        row = bench.judge_metric("timing", baseline, [11.0] * 5, True)
        self.assertAlmostEqual(row["baseline_spread_percent"], 100 * 1.1 / 10.4)
        self.assertAlmostEqual(row["tolerance_percent"], 2 * 100 * 1.1 / 10.4)
        self.assertEqual(row["verdict"], "same")
        self.assertEqual(bench.judge_metric("timing", baseline, [13.5] * 5, True)["verdict"], "worse")
        self.assertEqual(bench.judge_metric("timing", baseline, [7.5] * 5, True)["verdict"], "better")

    def test_memory_is_near_strict(self):
        baseline = [1000.0] * 5
        self.assertEqual(bench.judge_metric("memory", baseline, [1009.0] * 5, True)["verdict"], "same")
        self.assertEqual(bench.judge_metric("memory", baseline, [1020.0] * 5, True)["verdict"], "worse")

    def test_borderline_band_and_missing_values(self):
        self.assertTrue(bench.judge_metric("timing", [10.0] * 5, [10.55] * 5, True)["borderline"])
        self.assertFalse(bench.judge_metric("timing", [10.0] * 5, [10.2] * 5, True)["borderline"])
        missing = bench.judge_metric("timing", [10.0, None], [10.0, 10.0], True)
        self.assertEqual(missing["verdict"], "missing")
        self.assertFalse(missing["borderline"])

    def test_blocking_verdicts_tail_tracking_and_comparability(self):
        runs = side_runs("baseline", [1.0] * 5) + side_runs("candidate", [1.0] * 5, p99=40.0)
        scene = bench.compare_sides(runs, ["S2"])["S2"]
        tail = [row for row in scene["metrics"] if row["statistic"] == "p99_ms"]
        self.assertTrue(tail and all(row["verdict"] == "worse" and not row["blocking"] for row in tail))
        self.assertTrue(scene["passed"])

        slower = side_runs("baseline", [1.0] * 5) + side_runs("candidate", [1.08] * 5)
        self.assertFalse(bench.compare_sides(slower, ["S2"])["S2"]["passed"])

        other = side_runs("baseline", [1.0] * 5) + side_runs(
            "candidate", [1.0] * 5, signature=recorded_signature(effective_present_mode="Fifo"))
        item = bench.compare_sides(other, ["S2"])["S2"]
        self.assertFalse(item["comparable"] or item["passed"])
        self.assertIn("signature.effective_present_mode", item["reasons"][0])
        self.assertIn("INCOMPARABLE", bench.render_table({"S2": item}))

        failed = side_runs("baseline", [1.0] * 5) + side_runs("candidate", [1.0] * 5, status="failed")
        self.assertFalse(bench.compare_sides(failed, ["S2"])["S2"]["comparable"])

        no_memory = side_runs("baseline", [1.0] * 5) + side_runs("candidate", [1.0] * 5, peak=None)
        self.assertFalse(bench.compare_sides(no_memory, ["S2"])["S2"]["passed"])

    def test_unknown_or_changing_present_mode_is_incomparable(self):
        unknown = recorded_signature(effective_present_mode=None)
        both = side_runs("baseline", [1.0] * 5, signature=unknown) + side_runs(
            "candidate", [1.0] * 5, signature=unknown)
        item = bench.compare_sides(both, ["S2"])["S2"]
        self.assertFalse(item["comparable"] or item["passed"])
        self.assertTrue(any("effective_present_mode unknown" in r for r in item["reasons"]))
        self.assertIn("INCOMPARABLE", bench.render_table({"S2": item}))

        one = side_runs("baseline", [1.0] * 5) + side_runs("candidate", [1.0] * 5)
        one[7]["results"]["S2"]["signature"] = unknown  # one candidate run lost a stable mode
        item = bench.compare_sides(one, ["S2"])["S2"]
        self.assertFalse(item["comparable"] or item["passed"])
        self.assertTrue(any("candidate run 3" in r for r in item["reasons"]))

        stable = side_runs("baseline", [1.0] * 5) + side_runs("candidate", [1.0] * 5)
        self.assertTrue(bench.compare_sides(stable, ["S2"])["S2"]["passed"])

    def test_declared_presentation_backend_change_reaches_a_timing_verdict(self):
        vulkan = recorded_signature(gui_backend="Vulkan")
        runs = side_runs("baseline", [1.0] * 5) + side_runs("candidate", [1.0] * 5, signature=vulkan)
        strict = bench.compare_sides(runs, ["S2"])["S2"]
        self.assertFalse(strict["comparable"])
        self.assertIn("signature.gui_graphics.backend", strict["reasons"][0])

        declared = bench.compare_sides(runs, ["S2"], ("gui_graphics.backend",))["S2"]
        self.assertTrue(declared["comparable"] and declared["passed"])
        self.assertEqual(declared["declared_changes"],
                         {"gui_graphics.backend": {"baseline": "Dx12", "candidate": "Vulkan"}})
        self.assertIn("Dx12 → Vulkan", bench.render_table({"S2": declared}))
        slower = side_runs("baseline", [1.0] * 5) + side_runs("candidate", [1.2] * 5, signature=vulkan)
        self.assertFalse(bench.compare_sides(slower, ["S2"], ("gui_graphics.backend",))["S2"]["passed"])

        for change in ({"gui_adapter_name": "GTX 1660"}, {"vulkan_device": "GTX 1660"},
                       {"offscreen_render_pixels": [2560, 1440]}, {"measured_requested": 120},
                       {"effective_present_mode": "Fifo"}):
            with self.subTest(change=change):
                other = recorded_signature(gui_backend="Vulkan", **change)
                runs = side_runs("baseline", [1.0] * 5) + side_runs(
                    "candidate", [1.0] * 5, signature=other)
                item = bench.compare_sides(runs, ["S2"], ("gui_graphics.backend",))["S2"]
                self.assertFalse(item["comparable"])

        changed_fixture = side_runs("baseline", [1.0] * 5) + side_runs("candidate", [1.0] * 5, signature=vulkan)
        for run in changed_fixture[5:]:
            run["results"]["S2"]["fixture"] = {"fixture": "other"}
        self.assertFalse(bench.compare_sides(
            changed_fixture, ["S2"], ("gui_graphics.backend",))["S2"]["comparable"])

    def test_schedule_interleaves_sides_and_rerun_starts_with_candidate(self):
        self.assertEqual(bench.interleaved_schedule(2), [
            ("baseline", 1), ("candidate", 1), ("baseline", 2), ("candidate", 2)])
        self.assertEqual(bench.interleaved_schedule(1, candidate_first=True)[0][0], "candidate")

    def test_scene_subset_and_tail_statistics(self):
        self.assertEqual(bench.parse_scenes("s3, S2"), ["S2", "S3"])
        with self.assertRaises(bench.argparse.ArgumentTypeError):
            bench.parse_scenes("S9")
        self.assertEqual(bench.stats(range(1, 101))["p99_ms"], 99)
        self.assertEqual(bench.with_tail({"mean_ms": 1.0}, list(range(1, 201)))["p99_ms"], 198)
        self.assertIsNone(bench.with_tail(None, [1.0]))


class CompareSeriesTest(unittest.TestCase):
    """Drive the A/B orchestration with fixture results instead of a GPU."""

    def run_series(self, candidate_scale, declared=(), **case):
        calls = []

        def fake_pass(side, label, folder, scenes, number):
            calls.append((side["side"], number, tuple(scenes)))
            scale = 1.0 if side["side"] == "baseline" else candidate_scale(number)
            extra = case if side["side"] == "candidate" else {}
            return side_runs(side["side"], [scale], scenes, first=number, **extra)[0]

        with tempfile.TemporaryDirectory() as folder:
            series = Path(folder)
            sides = {name: {"side": name, "ref": name, "root": series, "exe": series / "exe",
                            "head": "0" * 40, "source_digest": "digest", "binary_sha256": "hash"}
                     for name in bench.SIDES}
            with patch.object(bench, "measure_pass", fake_pass), \
                    patch.object(bench, "capture", return_value={"source_digest": "digest"}), \
                    patch.object(bench, "entry", return_value={"sha256": "hash"}), \
                    patch("builtins.print"):
                code = bench.compare_series(series, sides, ["S2", "S3"], 3, {}, declared)
            report = json.loads((series / "compare.json").read_text(encoding="utf-8"))
            self.assertTrue((series / "compare.md").is_file())
        return code, report, calls

    def test_clear_pass_runs_one_interleaved_session(self):
        code, report, calls = self.run_series(lambda number: 1.01)
        self.assertEqual(code, 0)
        self.assertTrue(report["accepted"])
        self.assertEqual(report["rerun_scenes"], [])
        self.assertEqual([side for side, _, _ in calls],
                         ["baseline", "candidate"] + ["baseline", "candidate"] * 3)

    def test_borderline_scene_is_repeated_once_and_judged_on_both_sessions(self):
        # Session 1 is 5.5 % slower (borderline); the rerun shows 2 % slower.
        code, report, calls = self.run_series(lambda number: 1.055 if number <= 3 else 1.02)
        self.assertEqual(report["rerun_scenes"], ["S2", "S3"])
        rerun = [call for call in calls if call[1] > 3]
        self.assertEqual(rerun[0][0], "candidate")
        self.assertEqual(len(rerun), 6)
        self.assertEqual(report["scenes"]["S2"]["runs"], {"baseline": 6, "candidate": 6})
        self.assertEqual(code, 0)

    def test_unknown_present_mode_rejects_the_series(self):
        code, report, _ = self.run_series(
            lambda number: 1.0, signature=recorded_signature(effective_present_mode=None))
        self.assertEqual(code, 1)
        self.assertFalse(report["scenes"]["S2"]["comparable"])

    def test_declared_backend_change_is_recorded_in_the_report(self):
        vulkan = recorded_signature(gui_backend="Vulkan")
        self.assertEqual(self.run_series(lambda number: 1.0, signature=vulkan)[0], 1)
        code, report, _ = self.run_series(lambda number: 1.0, ("gui_graphics.backend",), signature=vulkan)
        self.assertEqual(code, 0)
        self.assertEqual(report["scenes"]["S3"]["declared_changes"]["gui_graphics.backend"],
                         {"baseline": "Dx12", "candidate": "Vulkan"})

    def test_clear_regression_fails_without_rerun(self):
        code, report, calls = self.run_series(lambda number: 1.2)
        self.assertEqual(code, 1)
        self.assertFalse(report["accepted"])
        self.assertEqual(report["rerun_scenes"], [])
        self.assertEqual(len(calls), 8)


if __name__ == "__main__":
    unittest.main()
