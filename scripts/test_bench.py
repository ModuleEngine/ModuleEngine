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
    def test_vsync_request_is_part_of_each_scene_signature(self):
        for scene in bench.SCENES:
            with self.subTest(scene=scene):
                field = "native_vsync_requested" if scene in ("S1", "S2") else "gui_vsync"
                off = bench.summarize_scene(scene, {field: False})[3]
                on = bench.summarize_scene(scene, {field: True})[3]
                missing = bench.summarize_scene(scene, {})[3]
                self.assertIs(off["vsync_requested"], False)
                self.assertIs(on["vsync_requested"], True)
                self.assertIsNone(missing["vsync_requested"])


if __name__ == "__main__":
    unittest.main()
