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


if __name__ == "__main__":
    unittest.main()
