import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import frontend_fixture_cli as cli


class FrontendFixtureReplay(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.documents = {"L0": ("lexer/CONTRACT.md", ()), "L1": ("parser/GRAMMAR.md", ())}
        self.paths = {}
        reject = "reject_fixtures_fail_with_their_expected_error"
        self.report = {"last_layer": "L5", "measurement_errors": [], "handoff_observations": {"goldens": {}},
                       "suites": {}, "lowering_cli": {"build": {"executable": "omega"}}}
        for stage, doc in self.documents.items():
            folder = self.root / "layers" / Path(doc[0]).parent / "tests/fixtures" / "names/reject"
            folder.mkdir(parents=True)
            path = self.paths[stage] = folder / "name.omg"
            source = b"// \xce\xbb\r\n?"
            path.write_bytes(source)
            fields = "  text: ?\n" if stage == "L0" else "  found: ?\n  expected: a name\n"
            path.with_suffix(".expected").write_text("error Problem at 2:1 bytes 7..8\n" + fields + "  first problem\n")
            self.report["suites"][stage] = {"tests": [{"target": "tests/golden_fixtures.rs", "test": reject, "status": "ok"}]}
            self.report["handoff_observations"]["goldens"][stage + "/names/reject/name.omg"] = reject

    def run_check(self, change=None):
        def invoke(args, **kwargs):
            stage = "L0" if "/lexer/" in args[-1] else "L1"
            reason = f"{args[-1]}:2:1: {cli.PRODUCERS[stage]}: first problem (bytes 7..8)\n"
            result = subprocess.CompletedProcess(args, 1, b"", reason.encode())
            if change:
                change(stage, args[2], result)
            return result
        with patch.object(cli, "ROOT", self.root), patch.dict(cli.handoff_evidence.DOCUMENTS, self.documents), \
                patch.object(cli.subprocess, "run", side_effect=invoke):
            cli.finish(self.report, self.root, {})
        return {row["stage"]: row for row in self.report["frontend_fixture_cli"]["results"]}

    def test_both_routes_and_passing_native_comparison_are_required(self):
        del self.report["handoff_observations"]["goldens"]["L0/names/reject/name.omg"]
        def wrong_span(stage, action, result):
            if (stage, action) == ("L1", "lower"):
                result.stderr = result.stderr.replace(b"7..8", b"6..7")
        rows = self.run_check(wrong_span)
        self.assertFalse(rows["L0"]["verified"])
        self.assertTrue(all(c["matched"] for c in rows["L0"]["commands"]))
        self.assertFalse(rows["L1"]["verified"])
        self.assertEqual([c["matched"] for c in rows["L1"]["commands"]], [True, False])
        self.assertEqual(len(self.report["measurement_errors"]), 2)

    def test_before_lowering_checks_parse_and_prepare(self):
        self.report["last_layer"] = "L4"
        for row in self.run_check().values():
            self.assertTrue(row["verified"])
            self.assertEqual([c["action"] for c in row["commands"]], ["parse", "prepare"])
        self.assertEqual(self.report["measurement_errors"], [])

    def test_orphan_and_wrong_byte_column_are_visible(self):
        self.paths["L0"].unlink()
        golden = self.paths["L1"].with_suffix(".expected")
        golden.write_text(golden.read_text().replace("2:1", "2:2"))
        rows = self.run_check()
        self.assertFalse(any(row["verified"] for row in rows.values()))
        self.assertIn("L0 has no rejection fixtures", self.report["measurement_errors"])
        self.assertIn("golden line and byte column disagree with source", rows["L1"]["error"])
        self.assertEqual(len(self.report["measurement_errors"]), 3)

    def test_unavailable_cli_preserves_the_fixture_roster(self):
        self.report["lowering_cli"]["build"] = {"error": "missing executable"}
        rows = self.run_check()
        self.assertEqual(set(rows), {"L0", "L1"})
        for row in rows.values():
            self.assertTrue(row["native"])
            self.assertFalse(row["verified"])
            self.assertEqual(row["commands"], [])
        self.assertEqual(len(self.report["measurement_errors"]), 1)
