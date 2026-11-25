"""Exercise the review workflow's actual prepare and validation steps without GitHub."""
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest


WORKFLOW = Path(__file__).resolve().parents[1] / ".github/workflows/claude-pr-review.yml"
PROSE = "Проверены изменения по каждому доступному текстовому файлу и пути в итоговом отчёте."


class ReviewPublicationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # These scripts live inline so a change to their checks changes the workflow itself.
        blocks = re.findall(r"<<'PY'\n(.*?)^ +PY$", WORKFLOW.read_text(encoding="utf-8"), re.M | re.S)
        cls.prepare, cls.validate = map(textwrap.dedent, blocks)
        temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(temp.cleanup)
        cls.root = Path(temp.name)

        def git(*args):
            subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=review@example.test",
                            "-c", "commit.gpgSign=false", *args], cwd=cls.root, check=True,
                           capture_output=True)

        git("init", "-q")
        git("config", "core.autocrlf", "false")
        old = cls.root / "original name.txt"
        lines = [f"stable line {index:03d}" for index in range(80)]
        old.write_text("\n".join([*lines, "old behaviour"]) + "\n", encoding="utf-8")
        git("add", "--all")
        git("commit", "-qm", "base")
        renamed = old.rename(cls.root / "new name.txt")
        renamed.write_text("\n".join([*lines, "new behaviour"]) + "\n", encoding="utf-8")
        (cls.root / "image.bin").write_bytes(bytes(range(256)))
        git("add", "--all")
        git("commit", "-qm", "candidate")
        cls.run_script(cls.prepare, check=True)
        cls.manifest = json.loads((cls.root / ".review-diffs/manifest.json").read_text(encoding="utf-8"))
        files = {entry["path"]: entry for entry in cls.manifest}
        cls.rename, cls.binary = files["new name.txt"], files["image.bin"]

    @classmethod
    def run_script(cls, script, *args, check=False):
        return subprocess.run([sys.executable, "-X", "utf8", "-", *args], input=script, text=True,
                              encoding="utf-8", capture_output=True, cwd=cls.root, check=check)

    def read_row(self, entry, note):
        diff = (self.root / entry["diff_file"]).read_text(encoding="utf-8")
        tokens = re.findall(r"^#REVIEW-CHUNK .* token=(\w+)$", diff, re.M)
        return f"- {entry['id']} | read | {'.'.join(tokens)} | {note}"

    def render(self, *, binary_read=False, note="Проверено изменение поведения.", remarks="нет"):
        rows = {self.rename["id"]: self.read_row(self.rename, note)}
        if binary_read:
            rows[self.binary["id"]] = self.read_row(self.binary, "Прочитан служебный заголовок.")
        else:
            rows[self.binary["id"]] = f"- {self.binary['id']} | skipped | - | Требуется просмотр изображения."
        report = "\n".join(["Ревью коммита 123abcd", "Вердикт: needs changes", "CI: pending",
                            "Что проверено: " + PROSE, f"Прочитано файлов: {1 + int(binary_read)} из 2",
                            "Файлы:", *(rows[entry["id"]] for entry in self.manifest), "Замечания: " + remarks])
        (self.root / "report.txt").write_text(report, encoding="utf-8")
        out = Path(tempfile.mkdtemp(dir=self.root))
        result = self.run_script(self.validate, "report.txt", "123abcd", ".review-diffs/manifest.json", str(out))
        return result, out

    def test_rename_keeps_both_paths_and_the_deleted_lines(self):
        self.assertEqual(self.rename["old_path"], "original name.txt")
        patch = (self.root / self.rename["diff_file"]).read_text(encoding="utf-8")
        for line in ("rename from original name.txt", "rename to new name.txt", "-old behaviour", "+new behaviour"):
            self.assertIn(line, patch.splitlines())
        result, out = self.render()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("original name.txt → new name.txt", (out / "review.md").read_text(encoding="utf-8"))
        self.assertFalse((out / "summary.md").exists())

    def test_binary_metadata_cannot_count_as_reviewed_content(self):
        result, _ = self.render(binary_read=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("binary", result.stderr)
        result, out = self.render()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("**НЕ ПРОЧИТАН**", (out / "review.md").read_text(encoding="utf-8"))

    def test_long_file_notes_preserve_verdict_and_findings(self):
        note = "Обнаружена разница. " * 3500
        finding = "Исправить обработку удалённой строки."
        result, out = self.render(note=note, remarks=finding)
        self.assertEqual(result.returncode, 0, result.stderr)
        full = (out / "review.md").read_text(encoding="utf-8")
        summary = (out / "summary.md").read_text(encoding="utf-8")
        self.assertIn(note.strip(), full)
        self.assertNotIn(note.strip(), summary)
        for report in (full, summary):
            self.assertIn("Вердикт: needs changes", report)
            self.assertIn("Замечания: " + finding, report)

    def test_oversized_findings_are_kept_in_full_report(self):
        finding = "Подробное замечание. " * 4000 + "Конец замечаний."
        result, out = self.render(remarks=finding)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(finding, (out / "review.md").read_text(encoding="utf-8"))
        summary = (out / "summary.md").read_text(encoding="utf-8")
        self.assertIn("Вердикт: needs changes", summary)
        self.assertIn("приложенном отчёте", summary)
        self.assertLess(len(summary.encode("utf-8")), 60000)


if __name__ == "__main__":
    unittest.main()
