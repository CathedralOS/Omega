"""Catalog integrity gate for tools/test_select_candidates.json.

Two contracts, matching the repository's catalog-roster pattern:

Validity: every candidate command must resolve — each ``-p`` package is a real
workspace member, each ``--test`` target exists in that package, every literal
file path in a command exists, and only the documented placeholders appear.

Coverage: every workspace package that declares test targets is accounted
for — either named by some candidate command's ``-p`` or listed in the
catalog's ``uncataloged_packages`` with a reason. A new package or a new
``tests/`` tree fails here until someone decides whether Jev should be able
to select it; silently uncataloged surfaces are recall holes.
"""
import json
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CATALOG = ROOT / "tools/test_select_candidates.json"
PLACEHOLDERS = {"{runner}", "{python}"}


def workspace_metadata():
    cargo = shutil.which("cargo") or shutil.which("mbx")
    if cargo is None:
        raise unittest.SkipTest("cargo is required to enumerate test targets")
    result = subprocess.run(
        [cargo, "metadata", "--no-deps", "--format-version", "1"],
        cwd=ROOT, capture_output=True, text=True, check=True)
    return json.loads(result.stdout)


class CatalogValidity(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
        cls.metadata = workspace_metadata()
        cls.packages = {p["name"]: p for p in cls.metadata["packages"]}

    def commands(self):
        for cand in self.catalog["candidates"]:
            yield cand["id"], cand.get("command") or []

    def test_every_candidate_has_id_and_covers(self):
        ids = [c["id"] for c in self.catalog["candidates"]]
        self.assertEqual(len(ids), len(set(ids)), "duplicate candidate ids")
        for cand in self.catalog["candidates"]:
            self.assertTrue(cand.get("covers"), cand["id"])

    def test_command_packages_exist(self):
        for cid, command in self.commands():
            for index, arg in enumerate(command[:-1]):
                if arg == "-p":
                    self.assertIn(command[index + 1], self.packages,
                                  f"{cid}: unknown package {command[index + 1]}")

    def test_command_test_targets_exist(self):
        for cid, command in self.commands():
            package = None
            for index, arg in enumerate(command[:-1]):
                if arg == "-p":
                    package = command[index + 1]
                if arg == "--test":
                    self.assertIsNotNone(package, f"{cid}: --test without -p")
                    names = {t["name"] for t in
                             self.packages[package]["targets"]
                             if "test" in t["kind"]}
                    self.assertIn(command[index + 1], names,
                                  f"{cid}: no test target "
                                  f"{command[index + 1]} in {package}")

    def test_command_literal_paths_exist(self):
        for cid, command in self.commands():
            for arg in command:
                if arg in PLACEHOLDERS or arg.startswith("-"):
                    continue
                candidate = ROOT / arg
                if (arg.endswith(".py") or arg.endswith(".sh")
                        or "/" in arg or "\\" in arg):
                    self.assertTrue(candidate.exists(),
                                    f"{cid}: missing path {arg}")

    def test_only_documented_placeholders(self):
        for cid, command in self.commands():
            for arg in command:
                if arg.startswith("{") or arg.endswith("}"):
                    self.assertIn(arg, PLACEHOLDERS,
                                  f"{cid}: unknown placeholder {arg}")


class CatalogCoverage(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
        cls.metadata = workspace_metadata()

    def test_every_package_with_test_targets_is_accounted_for(self):
        covered = set()
        for cand in self.catalog["candidates"]:
            command = cand.get("command") or []
            for index, arg in enumerate(command[:-1]):
                if arg == "-p":
                    covered.add(command[index + 1])
        with_tests = {p["name"] for p in self.metadata["packages"]
                      if any("test" in t["kind"] for t in p["targets"])}
        exempt = set(self.catalog.get("uncataloged_packages", {}))
        for name, reason in self.catalog.get("uncataloged_packages",
                                             {}).items():
            self.assertTrue(reason.strip(),
                            f"uncataloged {name} needs a reason")
        missing = with_tests - covered - exempt
        stale = exempt - with_tests
        self.assertFalse(missing,
                         f"packages with uncataloged test targets: "
                         f"{sorted(missing)} — add a candidate or a reasoned "
                         f"uncataloged_packages entry")
        self.assertFalse(stale,
                         f"uncataloged entries for packages with no test "
                         f"targets: {sorted(stale)}")


if __name__ == "__main__":
    unittest.main()
