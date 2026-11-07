#!/usr/bin/env python3
"""Portable checks for the Windows installer ownership contract."""

import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as xml

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "release"))
import windows_installer as installer
import windows_install_check


class WindowsInstallerTests(unittest.TestCase):
    def document(self, **changes):
        manifest = dict(schema="omega-release-bundle/1", dirty=False, host=installer.HOST,
                        version="0.1.0-alpha.3", commit="a" * 40,
                        files={"bin/omega.exe": "0", "lib/omega/library/core.omg": "1"})
        manifest.update(changes)
        with patch.object(installer.package, "verify_manifest", return_value=manifest):
            return xml.fromstring(installer.author(Path("bundle"), "0.1.3"))

    def test_complete_payload(self):
        document = self.document()
        files = document.findall(".//{*}File")
        self.assertEqual(len(files), 3)
        self.assertEqual({Path(item.get("Source")).name for item in files}, {"omega.exe", "core.omg", "manifest.json"})

    def test_user_scope_and_transactional_upgrade(self):
        document = self.document()
        self.assertEqual(document.find("{*}Package").get("Scope"), "perUser")
        self.assertEqual(document.find(".//{*}MajorUpgrade").get("Schedule"), "afterInstallInitialize")
        environment = document.find(".//{*}Environment")
        self.assertEqual(environment.get("System"), "no")
        self.assertEqual(environment.get("Permanent"), "no")
        self.assertEqual(environment.get("Part"), "last")
        self.assertFalse(document.findall(".//{*}CustomAction"))
        self.assertFalse(document.findall(".//{*}RemoveFile"))

    def test_product_identity_and_stable_component_identity(self):
        self.assertEqual(installer.product_code("0.1.3"), installer.product_code("0.1.3"))
        self.assertNotEqual(installer.product_code("0.1.3"), installer.product_code("0.1.4"))
        old = self.document(version="0.1.0-alpha.2")
        new = self.document()
        self.assertEqual([item.get("Guid") for item in old.findall(".//{*}Component")],
                         [item.get("Guid") for item in new.findall(".//{*}Component")])

    def test_invalid_installer_versions(self):
        for value in ("0.1.0-alpha.3", "1.2", "1.2.3.4", "256.0.0", "0.0.65536", "01.2.3", "-1.0.0"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                installer.installer_version(value)

    def test_wrong_host_or_dirty_bundle_rejects(self):
        for changes in ({"host": "aarch64-apple-darwin"}, {"dirty": True}, {"schema": "unknown"}, {"files": {}}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                self.document(**changes)

    def test_corrupt_bundle_rejects_before_tool_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "manifest.json").write_text(json.dumps({"files": {"bin/omega.exe": "missing"}}))
            with self.assertRaisesRegex(ValueError, "content differs"):
                installer.author(root, "0.1.3")

    def test_installation_mutations_refuse_workstations(self):
        arguments = ["check", "check", "--previous-tag", "v0.1.0-alpha.2", "--tag", "v0.1.0-alpha.3",
                     "--previous-installer-version", "0.1.2", "--installer-version", "0.1.3"]
        with patch.object(sys, "argv", arguments), patch.dict(installer.os.environ, {"GITHUB_ACTIONS": "false"}):
            with self.assertRaisesRegex(ValueError, "disposable"):
                windows_install_check.main()


if __name__ == "__main__":
    unittest.main()
