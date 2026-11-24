import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from check_doc_links import forbidden_target, new_violations


class DocumentationLinkTests(unittest.TestCase):
    def test_local_path_resolution(self):
        doc = "docs/evidence/r24/README.md"
        for target in ["../../../outputs/report.json", "../../../%77ork/example",
                       "../../../work/../outputs/report", r"D:\local\scene.png",
                       "/home/user/log.txt", "file:///E:/GPU%20bench", r"\\server\share\image.png"]:
            with self.subTest(target=target):
                self.assertIsNotNone(forbidden_target(doc, target))
        for target in ["../../hardware-tiers.md#r24-culling", "../work/example",
                       "./outputs/example", "https://example.com/work/report", "#local-anchor"]:
            with self.subTest(target=target):
                self.assertIsNone(forbidden_target(doc, target))

    def test_link_forms_and_occurrences(self):
        before = "[old](../work/kept)\n"
        after = before + r'''[duplicate](../work/kept)
![graph](<../outputs/capture with space.png>)
[benchmark]:
  ../work/new-run.json "Optional title"
<a href='../outputs/log.txt'>log</a>
<file:///C:/tmp/capture.png>
[balanced](reference(variant(v2))/../../work/output.json)
`[a literal](../work/inline)`
```md
[an example](../work/fenced)
```
'''
        problems = list(new_violations("docs/report.md", before, after))
        self.assertEqual([line for line, _, _ in problems], [2, 3, 5, 6, 7, 8])

    def test_quoted_fence_does_not_check_example_or_hide_a_later_link(self):
        contents = '> ```md\n> [code](../work/example)\n\n[outside](../work/result)\n'
        problems = list(new_violations("docs/report.md", "", contents))
        self.assertEqual([line for line, _, _ in problems], [4])

    def test_list_fence_does_not_check_example_or_hide_a_later_link(self):
        contents = '- ```md\n  [code](../work/example)\n  ```\n\n[outside](../work/result)\n'
        problems = list(new_violations("docs/report.md", "", contents))
        self.assertEqual([line for line, _, _ in problems], [5])

    def test_committed_positive_and_negative_checks(self):
        checker = Path(__file__).resolve().with_name("check_doc_links.py")
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            git = ["git", "-c", "user.name=Doc Test", "-c", "user.email=test@example.com"]

            def run_git(*args):
                return subprocess.check_output([*git, *args], cwd=root).decode().strip()

            def commit(text):
                doc.write_text(text, encoding="utf-8")
                run_git("add", "docs/report.md")
                run_git("commit", "-qm", "Fixture update")

            def run_check(base):
                return subprocess.run([sys.executable, str(checker), "--base", base],
                                      cwd=root, capture_output=True, text=True, check=False)

            run_git("init", "-q")
            doc = root / "docs/report.md"
            doc.parent.mkdir()
            legacy = "[legacy](../work/old)\n"
            commit(legacy)
            base = run_git("rev-parse", "HEAD")
            good = legacy + "[evidence](report.md#result)\n"
            commit(good)
            self.assertEqual(run_check(base).returncode, 0)
            commit(good + "[new](../outputs/new)\n[drive](<E:/my files/capture.png>)\n"
                   '<a title="x > y" href="../work/html">HTML link</a>\n\n'
                   '> [Reference][ref]\n>\n> [ref]: ../work/quoted\n\n'
                   '- [item]: ../outputs/list\n  [Reference][item]\n')
            negative = run_check(base)
            self.assertEqual(negative.returncode, 1, negative.stderr)
            self.assertIn("docs/report.md:3: checkout-local artifacts", negative.stderr)
            self.assertIn("docs/report.md:4: absolute local path", negative.stderr)
            self.assertIn("docs/report.md:5: checkout-local artifacts: ../work/html", negative.stderr)
            self.assertIn("docs/report.md:9: checkout-local artifacts: ../work/quoted", negative.stderr)
            self.assertIn("docs/report.md:11: checkout-local artifacts: ../outputs/list", negative.stderr)


if __name__ == "__main__":
    unittest.main()
