#!/usr/bin/env python3
"""Unit tests for tools/matching-logic-metrics/run_metrics.py; stdlib only."""

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


MODULE = (
    Path(__file__).resolve().parents[1]
    / "matching-logic-metrics"
    / "run_metrics.py"
)


def load_module():
    specification = importlib.util.spec_from_file_location(
        "matching_logic_metrics", MODULE)
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


METRICS = load_module()


class EnumVariantCount(unittest.TestCase):
    def write(self, root, relative, text):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path

    def test_counts_variants_across_forms(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = self.write(Path(tmp), "nodes.rs", '''
                pub enum ProofRule {
                    /// doc with {braces} inside
                    Primitive(PrimitiveJudgment),
                    SemanticAxiom {
                        index: usize,
                    },
                    Assumption { index: usize },
                    ConjunctionIntroduction(Vec<ProofNode>),
                    EqualitySymmetry,
                }
            ''')
            self.assertEqual(
                METRICS.enum_variant_count(path, "ProofRule"), 5)

    def test_comments_and_attributes_do_not_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = self.write(Path(tmp), "rules.rs", '''
                pub enum Rule {
                    // NotAVariant { brace }
                    First,
                    #[cfg(test)]
                    Second { field: u8 },
                }
            ''')
            self.assertEqual(
                METRICS.enum_variant_count(path, "Rule"), 2)

    def test_missing_enum_is_none(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = self.write(Path(tmp), "x.rs", "pub struct Other;\n")
            self.assertIsNone(METRICS.enum_variant_count(path, "Rule"))
        self.assertIsNone(
            METRICS.enum_variant_count(Path("/nonexistent/x.rs"), "Rule"))


class SourceInventory(unittest.TestCase):
    def test_splits_test_sources(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "src"
            (root / "checker").mkdir(parents=True)
            (root / "checker" / "mod.rs").write_text("a\nb\nc\n")
            (root / "checker" / "tests.rs").write_text("t\n")
            (root / "tests").mkdir()
            (root / "tests" / "more.rs").write_text("u\nv\n")
            inventory = METRICS.source_inventory(root)
            self.assertEqual(inventory["files"], 1)
            self.assertEqual(inventory["lines"], 3)
            self.assertEqual(inventory["test_files"], 2)
            self.assertEqual(inventory["test_lines"], 3)

    def test_missing_path_is_none(self):
        self.assertIsNone(
            METRICS.source_inventory(Path("/nonexistent/tree")))

    def test_pattern_selects_python_sources(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "tool"
            root.mkdir()
            (root / "checker.py").write_text("a\nb\n")
            (root / "helper.rs").write_text("r\n")
            inventory = METRICS.source_inventory(root, "*.py")
            self.assertEqual(inventory["files"], 1)
            self.assertEqual(inventory["lines"], 2)


class EncodingRoute(unittest.TestCase):
    def repo(self):
        return Path(subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"], text=True).strip())

    def test_encoding_route_measures_the_landed_slice(self):
        route = METRICS.measure_encoding_route(
            self.repo(), repetitions=1, skip_cases=False)
        self.assertEqual(route["status"], "measured")
        for axis in ("checker", "translation"):
            self.assertGreater(route[axis]["files"], 0)
            self.assertGreater(route[axis]["lines"], 0)
        self.assertEqual(route["case_status"], "measured")
        self.assertGreaterEqual(len(route["cases"]), 1)
        for case in route["cases"]:
            self.assertTrue(case["match"], case["path"])
        self.assertEqual(route["case_mismatches"], 0)
        self.assertGreater(route["certificate"]["bytes"], 0)
        theory = route["theory"]["rule_inventory"]
        self.assertGreater(theory.get("checkerRules", 0), 0)
        self.assertGreater(theory.get("encodingClauses", 0), 0)
        encoding_cases = route["theory"]["encoding_cases"]
        self.assertEqual(
            sum(1 for row in encoding_cases if row["consistent"]), 1)

    def test_encoding_route_pends_without_the_slice(self):
        with tempfile.TemporaryDirectory() as tmp:
            route = METRICS.measure_encoding_route(
                Path(tmp), repetitions=1, skip_cases=False)
        self.assertEqual(route["status"], "pending")

    def test_skip_cases_still_measures_inventories(self):
        route = METRICS.measure_encoding_route(
            self.repo(), repetitions=1, skip_cases=True)
        self.assertEqual(route["status"], "measured")
        self.assertEqual(route["case_status"], "skipped")
        self.assertGreater(route["checker"]["files"], 0)


class ParseTimings(unittest.TestCase):
    def test_parses_stage_table_and_total(self):
        output = (
            "     123.456 ms  Stage 01: SourceFiles -> TokenStreams\n"
            "      45.000 ms  Stage 05: TypedTrees -> CheckedTrees\n"
            "     999.000 ms  total elapsed\n"
        )
        stages, total, measurements = METRICS.parse_timings(output)
        self.assertEqual(
            stages["Stage 05: TypedTrees -> CheckedTrees"], 45.0)
        self.assertEqual(total, 999.0)
        self.assertEqual(measurements, [])

    def test_captures_measurement_lines(self):
        output = "OMEGA_PROOF_MEASUREMENTS foo=1\n"
        _, _, measurements = METRICS.parse_timings(output)
        self.assertEqual(measurements, ["OMEGA_PROOF_MEASUREMENTS foo=1"])


class MeasureCase(unittest.TestCase):
    def case_dir(self, root, relative):
        directory = root / relative
        directory.mkdir(parents=True)
        (directory / "main.omg").write_text("machine m { }\n")
        return directory

    def fake_runs(self, exit_code, compile_ms=None, total=40.0):
        return {
            "exit_code": exit_code,
            "wall_ms": 50.0,
            "compile_ms": compile_ms,
            "total_elapsed_ms": total,
            "stages": {},
            "proof_measurements": [],
        }

    def test_polarity_match_and_mismatch(self):
        import unittest.mock as mock
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.case_dir(root, "tests/omega/pass/proofs/pos")
            accept_run = self.fake_runs(0, compile_ms=30.0)
            with mock.patch.object(
                    METRICS, "run_check", return_value=accept_run):
                row, ok = METRICS.measure_case(
                    "omega", root, "tests/omega/pass/proofs/pos",
                    "accept", 3)
            self.assertTrue(ok)
            self.assertEqual(row["outcome"], "accept")
            self.assertEqual(row["check_time_ms"]["compile"], 30.0)
            self.assertEqual(len(row["runs"]), 3)

            with mock.patch.object(
                    METRICS, "run_check", return_value=accept_run):
                row, ok = METRICS.measure_case(
                    "omega", root, "tests/omega/pass/proofs/pos",
                    "reject", 1)
            self.assertFalse(ok)
            self.assertFalse(row["match"])

            reject_run = self.fake_runs(1, compile_ms=None)
            with mock.patch.object(
                    METRICS, "run_check", return_value=reject_run):
                row, ok = METRICS.measure_case(
                    "omega", root, "tests/omega/pass/proofs/pos",
                    "reject", 1)
            self.assertTrue(ok)
            self.assertEqual(row["outcome"], "reject")
            self.assertIsNone(row["check_time_ms"]["compile"])

    def test_missing_case_reports(self):
        import unittest.mock as mock
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            row, ok = METRICS.measure_case(
                "omega", root, "tests/omega/pass/proofs/absent", "accept", 1)
            self.assertFalse(ok)
            self.assertEqual(row["status"], "missing-case")


class Validate(unittest.TestCase):
    def test_schema_and_axes_checked(self):
        with tempfile.TemporaryDirectory() as tmp:
            record = {
                "schema": METRICS.SCHEMA,
                "revision": "x" * 40,
                "route": {"current": {
                    "checker": {key: 0 for key in METRICS.AXIS_KEYS},
                    "translation": {key: 0 for key in METRICS.AXIS_KEYS},
                    "trusted_derivation": {
                        key: 0 for key in METRICS.AXIS_KEYS},
                    "theory": {"rule_inventory": {
                        name[0].lower() + name[1:]: 1
                        for _, name in METRICS.RULE_INVENTORY}},
                }},
                "cases": [],
                "case_status": "skipped",
            }
            path = Path(tmp) / "rec.json"
            path.write_text(json.dumps(record))
            errors = []
            METRICS.validate_record(path, errors)
            self.assertEqual(errors, [])

    def test_wrong_schema_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "rec.json"
            path.write_text(json.dumps({"schema": "other"}))
            errors = []
            METRICS.validate_record(path, errors)
            self.assertTrue(errors)


class PinnedCasesFile(unittest.TestCase):
    def test_committed_pairs_resolve_on_checkout(self):
        repo = subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"], text=True).strip()
        document = json.loads(
            (Path(repo) / "tools/matching-logic-metrics/pinned_cases.json")
            .read_text())
        self.assertEqual(
            document["schema"], "omega-matching-logic-pinned-cases/1")
        for pair in document["pairs"]:
            self.assertTrue(
                (Path(repo) / pair["positive"] / "main.omg").is_file(),
                pair["positive"])
            for negative in pair["negative"]:
                self.assertTrue(
                    (Path(repo) / negative / "main.omg").is_file(), negative)


if __name__ == "__main__":
    unittest.main()
