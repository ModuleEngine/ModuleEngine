"""Third-party license policy: SPDX parsing, allow list and notice rendering without Cargo."""
from pathlib import Path
import shutil
import tempfile
import unittest

import third_party_licenses as tpl


ROOT = Path(__file__).resolve().parent.parent


def package(directory, name, license, authors=("Ada <ada@example.com>",), source="registry+https://github.com/rust-lang/crates.io-index"):
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "Cargo.toml").write_text("[package]\n", encoding="utf-8")
    return {"id": name, "name": name, "version": "1.0.0", "license": license, "authors": list(authors),
            "source": source, "manifest_path": str(directory / "Cargo.toml")}


def metadata(*packages, dev=()):
    member = {"id": "member", "name": "member", "version": "0.0.0", "license": "MIT",
              "authors": [], "source": None, "manifest_path": "Cargo.toml"}
    deps = [{"pkg": p["id"], "dep_kinds": [{"kind": "dev" if p["id"] in dev else None}]} for p in packages]
    nodes = [{"id": "member", "deps": deps}] + [{"id": p["id"], "deps": []} for p in packages]
    return {"workspace_members": ["member"], "packages": [member, *packages], "resolve": {"nodes": nodes}}


class ExpressionTests(unittest.TestCase):
    def test_permissive_alternatives_and_conjunctions_are_allowed(self):
        for expression in ["MIT", "MIT OR Apache-2.0", "MIT/Apache-2.0", "Apache-2.0 OR GPL-2.0-only",
                           "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0",
                           "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
                           "MIT OR Apache-2.0 OR LGPL-2.1-or-later"]:
            self.assertIsNotNone(tpl.allowed_option(expression), expression)

    def test_copyleft_only_or_conjunction_is_rejected(self):
        for expression in ["GPL-3.0-only", "MIT AND GPL-2.0-only", "LGPL-2.1-or-later", "MPL-2.0",
                           "Apache-2.0 WITH Classpath-exception-2.0", "AGPL-3.0-or-later"]:
            self.assertIsNone(tpl.allowed_option(expression), expression)

    def test_malformed_expressions_fail_closed(self):
        for expression in ["", "MIT OR", "(MIT", "MIT)", "AND MIT", "MIT WITH", "MIT Apache-2.0"]:
            with self.assertRaises(tpl.PolicyError, msg=expression):
                tpl.license_options(expression)


class RenderTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.root)
        for name in ["LICENSE-MIT", "LICENSE-APACHE", "scripts/license-texts/BSL-1.0.txt"]:
            (self.root / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, self.root / name)

    def test_dev_only_dependencies_are_not_shipped(self):
        shipped = package(self.root / "a", "a", "MIT")
        dev = package(self.root / "b", "b", "GPL-3.0-only")
        packages = tpl.collect(self.root, [metadata(shipped, dev, dev={"b"})])
        self.assertEqual([p["name"] for p in packages], ["a"])

    def test_disallowed_or_unlicensed_shipped_package_fails(self):
        for license in ["GPL-3.0-only", None]:
            bad = package(self.root / "c", "c", license)
            with self.assertRaises(tpl.PolicyError):
                tpl.collect(self.root, [metadata(bad)])

    def test_git_and_path_sources_fail(self):
        for source in ["git+https://example.com/x", None]:
            bad = package(self.root / "d", "d", "MIT", source=source)
            with self.assertRaises(tpl.PolicyError):
                tpl.collect(self.root, [metadata(bad)])

    def test_shipped_texts_are_deduplicated_and_missing_files_use_standard_text(self):
        first = package(self.root / "e", "e", "MIT")
        second = package(self.root / "f", "f", "MIT")
        for directory in ("e", "f"):
            (self.root / directory / "LICENSE").write_text("Same notice\r\n", encoding="utf-8")
        bare = package(self.root / "g", "g", "Zlib OR MIT", authors=())
        boost = package(self.root / "h", "h", "BSL-1.0")
        text = tpl.render(self.root, [first, second, bare, boost])
        self.assertEqual(text.count("Same notice"), 1)
        self.assertIn("T001: e 1.0.0, f 1.0.0", text)
        self.assertIn("Copyright (c) The g authors", text)
        self.assertIn("Boost Software License - Version 1.0", text)
        self.assertNotIn("\r", text)

    def test_package_without_file_or_standard_text_fails(self):
        bare = package(self.root / "i", "i", "ISC")
        with self.assertRaises(tpl.PolicyError):
            tpl.render(self.root, [bare])

    def test_declared_extra_notice_must_exist(self):
        fonts = package(self.root / "epaint_default_fonts", "epaint_default_fonts", "MIT")
        with self.assertRaises(tpl.PolicyError):
            tpl.render(self.root, [fonts])


if __name__ == "__main__":
    unittest.main()
