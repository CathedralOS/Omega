#!/usr/bin/env python3
"""Guard the benchmark record format: committed rows stay valid and the
documented schema version cannot drift from the validator unnoticed.

Runs locally only; no GitHub access or non-standard Python packages:

    python3 tools/tests/test_benchmark.py -v
"""

import copy
import importlib.util
import json
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
BENCHMARK = ROOT / "tools" / "benchmark" / "benchmark.py"
README = ROOT / "tools" / "benchmark" / "README.md"
RECORDS = ROOT / "tools" / "benchmark" / "records"


def load_benchmark():
    specification = importlib.util.spec_from_file_location(
        "benchmark_under_test", BENCHMARK
    )
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


benchmark = load_benchmark()


def minimal_record():
    return {
        "schema": benchmark.SCHEMA,
        "recorded_utc": "2026-09-19T00:00:00Z",
        "subject": {
            "name": "fixture",
            "root": "samples/cli/basics/cli_mvp/main.omg",
            "source_revision": "0" * 40,
        },
        "key": {
            "target": "linux_x86_64",
            "selection": {"enabled": [], "disabled": []},
        },
        "host": {
            "os": "linux",
            "machine": "x86_64",
            "omega_binary": "target/debug/omega",
        },
        "metrics": {
            "compile_time_ms": {
                "status": "measured",
                "unit": "ms",
                "samples": [1.0],
                "median_ms": 1.0,
                "min_ms": 1.0,
            },
            "peak_memory_bytes": {
                "status": "measured",
                "unit": "bytes",
                "compile_max_rss": 1,
                "run_max_rss": 1,
            },
            "code_size_bytes": {
                "status": "measured",
                "unit": "bytes",
                "value": 1,
                "samples": [1],
                "stable": True,
            },
            "runtime_ms": {
                "status": "measured",
                "unit": "ms",
                "samples": [1.0],
                "median_ms": 1.0,
                "min_ms": 1.0,
                "exit_codes": [0],
            },
        },
        "notes": [],
    }


class CommittedRecords(unittest.TestCase):
    def test_every_committed_record_validates(self):
        records = sorted(RECORDS.glob("*.json"))
        self.assertTrue(records, "records/ must not be empty")
        for path in records:
            record = json.loads(path.read_text())
            problems = benchmark.validate_record(record, str(path))
            self.assertEqual(problems, [])

    def test_record_filenames_match_row_key(self):
        for path in sorted(RECORDS.glob("*.json")):
            record = json.loads(path.read_text())
            label = benchmark.selection_label(record["key"]["selection"])
            expected = (
                f"{record['subject']['name']}__{record['key']['target']}"
                f"__{label}.json"
            )
            self.assertEqual(path.name, expected)


class SchemaDocumentation(unittest.TestCase):
    def test_readme_declares_current_schema(self):
        text = README.read_text()
        self.assertIn(benchmark.SCHEMA, text)

    def test_readme_documents_every_metric(self):
        text = README.read_text()
        for name in benchmark.METRIC_NAMES:
            self.assertIn(f"metrics.{name}", text)


class Validation(unittest.TestCase):
    def assert_invalid(self, record):
        problems = benchmark.validate_record(record, "<fixture>")
        self.assertTrue(problems)

    def test_minimal_record_validates(self):
        self.assertEqual(
            benchmark.validate_record(minimal_record(), "<fixture>"), []
        )

    def test_wrong_schema_version_rejected(self):
        record = minimal_record()
        record["schema"] = "omega-benchmark-record/0"
        self.assert_invalid(record)

    def test_unsorted_selection_rejected(self):
        record = minimal_record()
        record["key"]["selection"]["enabled"] = [
            "CopyPropagation",
            "ControlFlowCleanup",
        ]
        self.assert_invalid(record)

    def test_non_exact_rule_name_rejected(self):
        record = minimal_record()
        record["key"]["selection"]["disabled"] = ["copy-propagation"]
        self.assert_invalid(record)

    def test_unavailable_metric_needs_reason(self):
        record = minimal_record()
        record["metrics"]["runtime_ms"] = {
            "status": "unavailable", "unit": "ms"
        }
        self.assert_invalid(record)

    def test_measured_metric_needs_samples(self):
        record = minimal_record()
        record["metrics"]["compile_time_ms"]["samples"] = []
        self.assert_invalid(record)

    def test_short_revision_rejected(self):
        record = minimal_record()
        record["subject"]["source_revision"] = "abc123"
        self.assert_invalid(record)


class SelectionIdentity(unittest.TestCase):
    def test_empty_selection_is_default(self):
        self.assertEqual(
            benchmark.selection_label({"enabled": [], "disabled": []}),
            "default",
        )

    def test_nonempty_selection_is_hashed(self):
        label = benchmark.selection_label(
            {"enabled": ["CopyPropagation"], "disabled": []}
        )
        self.assertTrue(label.startswith("sel-"))

    def test_authored_selection_reads_enable_calls(self):
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "main.omg"
            root.write_text("machine Main::main(&mut self) {}\n")
            (Path(directory) / "build.omg").write_text(
                "machine build(builder: &mut Build) {\n"
                "    builder.optimizations.enable(\n"
                "        Optimization::CopyPropagation,\n"
                "    );\n"
                "    builder.optimizations.enable(Optimization::DeadPureScalarElimination);\n"
                "}\n"
            )
            self.assertEqual(
                benchmark.authored_selection(root),
                ["CopyPropagation", "DeadPureScalarElimination"],
            )

    def test_authored_selection_without_build_file(self):
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "main.omg"
            root.write_text("")
            self.assertEqual(benchmark.authored_selection(root), [])


class ReviewSettlement(unittest.TestCase):
    def test_pending_decisions_become_accept(self):
        document = (
            "comparison abc\n"
            "decision row 0123 pending\n"
            "decision root-role pending\n"
            "audit-recommended true\n"
        )
        settled, count = benchmark.accept_pending_decisions(document)
        self.assertEqual(count, 2)
        self.assertIn("decision row 0123 accept\n", settled)
        self.assertIn("decision root-role accept\n", settled)
        self.assertIn("audit-recommended true\n", settled)

    def test_non_decision_pending_text_is_untouched(self):
        document = "pending review content\nno decisions here\n"
        settled, count = benchmark.accept_pending_decisions(document)
        self.assertEqual(count, 0)
        self.assertEqual(settled, document)


if __name__ == "__main__":
    unittest.main()
