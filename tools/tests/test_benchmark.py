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
import re
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
BENCHMARK = ROOT / "tools" / "benchmark" / "benchmark.py"
README = ROOT / "tools" / "benchmark" / "README.md"
RECORDS = ROOT / "tools" / "benchmark" / "records"
MATRIX_DOC = ROOT / "wiki" / "drafts" / "measurements" / "benchmarks.md"
TARGET_SOURCE = (
    ROOT / "omega-rust" / "omega" / "compilation" / "backend" / "target"
    / "src" / "target_profile.rs"
)


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
            "omega_binary_sha256": "a" * 64,
            "omega_version": "omega 0.1.0-dev (unrecorded; x86_64-unknown-linux)",
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
            record = json.loads(path.read_text(encoding="utf-8"))
            problems = benchmark.validate_record(record, str(path))
            self.assertEqual(problems, [])

    def test_record_filenames_match_row_key(self):
        for path in sorted(RECORDS.glob("*.json")):
            record = json.loads(path.read_text(encoding="utf-8"))
            label = benchmark.selection_label(record["key"]["selection"])
            expected = (
                f"{record['subject']['name']}__{record['key']['target']}"
                f"__{label}.json"
            )
            self.assertEqual(path.name, expected)


class SchemaDocumentation(unittest.TestCase):
    def test_readme_declares_current_schema(self):
        text = README.read_text(encoding="utf-8")
        self.assertIn(benchmark.SCHEMA, text)

    def test_readme_documents_every_metric(self):
        text = README.read_text(encoding="utf-8")
        for name in benchmark.METRIC_NAMES:
            self.assertIn(f"metrics.{name}", text)


def legacy_record():
    """A /1 row as committed before the compiler-build binding fields."""
    record = minimal_record()
    record["schema"] = benchmark.LEGACY_SCHEMA
    for field in ("omega_version", "omega_binary_sha256"):
        del record["host"][field]
    return record


class Validation(unittest.TestCase):
    def assert_invalid(self, record):
        problems = benchmark.validate_record(record, "<fixture>")
        self.assertTrue(problems)

    def test_minimal_record_validates(self):
        self.assertEqual(
            benchmark.validate_record(minimal_record(), "<fixture>"), []
        )

    def test_legacy_schema_record_validates(self):
        self.assertEqual(
            benchmark.validate_record(legacy_record(), "<fixture>"), []
        )

    def test_current_schema_requires_binary_identity(self):
        record = minimal_record()
        del record["host"]["omega_binary_sha256"]
        self.assert_invalid(record)

    def test_failed_exit_observation_rejected(self):
        record = minimal_record()
        record["metrics"]["runtime_ms"]["exit_code_match"] = False
        self.assert_invalid(record)

    def test_disabled_selection_requires_receipt(self):
        record = minimal_record()
        record["key"]["selection"]["disabled"] = ["CopyPropagation"]
        self.assert_invalid(record)

    def test_consistent_receipt_validates(self):
        record = minimal_record()
        record["key"]["selection"]["enabled"] = [
            "CopyPropagation",
            "GlobalValueNumbering",
        ]
        record["key"]["selection"]["disabled"] = ["GlobalValueNumbering"]
        record["key"]["probe_disable"] = "ControlFlowCleanup"
        record["key"]["selection_receipt"] = {
            "requested": ["ControlFlowCleanup", "GlobalValueNumbering"],
            "applied": ["GlobalValueNumbering"],
            "effective": ["CopyPropagation"],
        }
        self.assertEqual(
            benchmark.validate_record(record, "<fixture>"), []
        )

    def test_receipt_must_cover_the_requested_disables(self):
        record = minimal_record()
        record["key"]["selection"]["disabled"] = ["GlobalValueNumbering"]
        record["key"]["selection_receipt"] = {
            "requested": ["CopyPropagation"],
            "applied": [],
            "effective": ["GlobalValueNumbering"],
        }
        self.assert_invalid(record)

    def test_enabled_must_equal_the_receipt_selected_set(self):
        record = minimal_record()
        record["key"]["selection"]["enabled"] = ["CopyPropagation"]
        record["key"]["selection_receipt"] = {
            "requested": ["GlobalValueNumbering"],
            "applied": [],
            "effective": ["GlobalValueNumbering"],
        }
        self.assert_invalid(record)

    def test_probe_in_applied_set_rejected(self):
        record = minimal_record()
        record["key"]["selection"]["enabled"] = ["CopyPropagation"]
        record["key"]["probe_disable"] = "CopyPropagation"
        record["key"]["selection_receipt"] = {
            "requested": ["CopyPropagation"],
            "applied": ["CopyPropagation"],
            "effective": [],
        }
        self.assert_invalid(record)

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


def catalogued_target_names():
    """Profile names from TargetProfile::target_name() — the catalog the
    matrix's host-leg table must stay in step with."""
    text = TARGET_SOURCE.read_text(encoding="utf-8")
    # `HostedIntrinsicBundle` declares a delegating `target_name` above this
    # one; the profile's own is the arm table, so anchor on the match.
    anchor = "const fn target_name(self) -> &'static str {\n        match self {"
    section = text[text.index(anchor):]
    section = section[: section.index("\n    }\n")]
    return set(re.findall(r'=> "([a-z][a-z0-9_]*)",', section))


class HostRowMatrix(unittest.TestCase):
    def matrix(self):
        return benchmark.matrix_markdown(benchmark.load_records(RECORDS))

    def test_host_legs_cover_the_catalogued_targets(self):
        self.assertEqual(
            {leg["target"] for leg in benchmark.HOST_LEGS},
            catalogued_target_names(),
        )

    def test_unmeasured_host_legs_stay_explicit(self):
        output = self.matrix()
        # windows_x86_64, linux_arm64, and macos_arm64 carry committed
        # records now; cross_platform_cli and local_unchecked remain the
        # unrecorded build-host legs that must stay explicit.
        cli = next(
            line for line in output.splitlines() if "cross_platform_cli" in line
        )
        self.assertIn("measurable", cli)
        self.assertNotIn("unavailable", cli)
        unchecked = next(
            line for line in output.splitlines() if "local_unchecked" in line
        )
        self.assertIn("measurable", unchecked)
        self.assertNotIn("unavailable", unchecked)
        # The leg's own row is the one with no subject: a committed
        # non-applicable pairing sits beside it and must not retire it.
        uefi = next(
            line for line in output.splitlines()
            if "uefi_x86_64" in line and "QEMU or UEFI hardware" in line
        )
        self.assertIn("unavailable (needs QEMU or UEFI hardware)", uefi)

    def test_non_applicable_pairing_renders_beside_its_host_leg(self):
        output = self.matrix()
        pairing = next(
            line for line in output.splitlines()
            if "uefi_x86_64" in line and "wrapping_square_sum" in line
        )
        self.assertIn("non-applicable", pairing)
        self.assertIn("no bound required root slot", pairing)
        # Its host leg keeps its own explicit row.
        self.assertTrue(any(
            "uefi_x86_64" in line and "QEMU or UEFI hardware" in line
            for line in output.splitlines()))

    def test_measured_records_render_measured_cells(self):
        row = next(
            line for line in self.matrix().splitlines() if "cli_mvp" in line
        )
        self.assertIn("measured", row)
        self.assertIn("default", row)

    def test_skipped_runtime_keeps_the_pending_host_row(self):
        # windows_x86_64 and linux_arm64 carry committed compile rows, but
        # every runtime leg there was skipped: the named host environment is
        # still unrun, so its projected pending row must not be suppressed.
        output = self.matrix()
        self.assertTrue(any(
            "windows_x86_64" in line
            and "pending Windows x86-64 host" in line
            for line in output.splitlines()))
        self.assertTrue(any(
            "linux_arm64" in line
            and "pending Linux ARM64 host" in line
            for line in output.splitlines()))

    def test_measured_runtime_retires_the_pending_host_row(self):
        # linux_x86_64 and macos_arm64 have records with measured runtime
        # legs: their projected rows are gone while measured rows remain.
        pending = [
            line for line in self.matrix().splitlines() if "— | —" in line
        ]
        self.assertFalse(any("linux_x86_64" in line for line in pending))
        self.assertFalse(any("macos_arm64" in line for line in pending))

    def test_doc_embeds_the_current_matrix(self):
        lines = MATRIX_DOC.read_text(encoding="utf-8").splitlines()
        start = lines.index("<!-- benchmark-matrix:start -->")
        end = lines.index("<!-- benchmark-matrix:end -->")
        self.assertEqual("\n".join(lines[start + 1 : end]), self.matrix())


class Applicability(unittest.TestCase):
    def test_absent_applicability_still_validates(self):
        record = minimal_record()
        self.assertNotIn("applicability", record)
        self.assertEqual(benchmark.validate_record(record, "<fixture>"), [])

    def test_non_applicable_record_validates(self):
        record = minimal_record()
        reason = "no bound required root slot `uefi_x86_64::ProgramEntry`"
        record["applicability"] = {
            "status": "non_applicable", "reason": reason}
        for name in benchmark.METRIC_NAMES:
            unit = "bytes" if "bytes" in name else "ms"
            record["metrics"][name] = benchmark.unavailable(unit, reason)
        self.assertEqual(benchmark.validate_record(record, "<fixture>"), [])

    def test_non_applicable_rejects_an_unknown_status(self):
        record = minimal_record()
        record["applicability"] = {"status": "maybe", "reason": "x"}
        problems = benchmark.validate_record(record, "<fixture>")
        self.assertTrue(any("non_applicable" in p for p in problems))

    def test_non_applicable_requires_a_reason(self):
        record = minimal_record()
        record["applicability"] = {"status": "non_applicable", "reason": ""}
        problems = benchmark.validate_record(record, "<fixture>")
        self.assertTrue(any("reason is required" in p for p in problems))

    def test_non_applicable_cannot_carry_a_measured_metric(self):
        record = minimal_record()
        record["applicability"] = {
            "status": "non_applicable", "reason": "unbound root"}
        problems = benchmark.validate_record(record, "<fixture>")
        self.assertTrue(
            any("cannot carry a measured metric" in p for p in problems))

    def test_settlement_rejection_becomes_a_non_applicable_signal(self):
        text = ("error: no bound required root slot "
                "`uefi_x86_64::ProgramEntry`\n")
        found = benchmark.UNBOUND_ROOT_SLOT.search(text)
        self.assertIsNotNone(found)
        self.assertEqual(
            found.group(0),
            "no bound required root slot `uefi_x86_64::ProgramEntry`")

    def test_an_ordinary_settlement_failure_is_not_non_applicable(self):
        self.assertIsNone(
            benchmark.UNBOUND_ROOT_SLOT.search("error: package acceptance is missing"))


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

    def test_review_document_prefers_the_requested_target(self):
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            project = Path(directory)
            reviews = project / "build" / "package-manager"
            reviews.mkdir(parents=True)
            for target in ("linux_x86_64", "macos_arm64"):
                (reviews / f"review-{target}.txt").write_text(
                    "decision x pending\n")
            output = (
                f"review: {reviews}/review-linux_x86_64.txt\n"
                f"review: {reviews}/review-macos_arm64.txt\n"
            )
            chosen = benchmark.select_review_document(
                output, project, "macos_arm64")
            self.assertEqual(chosen.name, "review-macos_arm64.txt")

    def test_review_document_falls_back_to_the_conventional_path(self):
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            project = Path(directory)
            reviews = project / "build" / "package-manager"
            reviews.mkdir(parents=True)
            (reviews / "review-linux_arm64.txt").write_text(
                "decision x pending\n")
            chosen = benchmark.select_review_document(
                "no review lines\n", project, "linux_arm64")
            self.assertEqual(chosen.name, "review-linux_arm64.txt")

    def test_published_lock_survives_an_empty_resume(self):
        import tempfile
        from unittest.mock import patch

        with tempfile.TemporaryDirectory() as directory:
            project = Path(directory)
            reviews = project / "build" / "package-manager"
            reviews.mkdir(parents=True)
            review = reviews / "review-macos_arm64.txt"
            update_output = f"review: {review}\n"

            def fake_run(argv, cwd):
                if "--resume" in argv:
                    # A depend-free project publishes the lock in the
                    # first update; resuming finds nothing pending.
                    return 1, "no pending package review\n"
                review.write_text("decision x pending\n")
                (project / "omega.lock").write_text("lock v\n")
                return 0, update_output

            with patch.object(benchmark, "run_plain", fake_run):
                lock = benchmark.settle_package_review(
                    Path("/fake/omega"), project, "macos_arm64")
            self.assertEqual(lock, project / "omega.lock")


class ProbeSelection(unittest.TestCase):
    def test_probe_prefers_psi_phase_rules(self):
        catalog = [
            ("PostAllocationRule", "PostAllocationMachine"),
            ("PsiRule", "Psi"),
            ("SelectedRule", "SelectedLowering"),
        ]
        self.assertEqual(
            benchmark.probe_disable_candidates(catalog, [], []),
            ["PsiRule", "SelectedRule", "PostAllocationRule"],
        )

    def test_probe_skips_spoken_names(self):
        catalog = [("PsiRule", "Psi"), ("OtherPsi", "Psi")]
        self.assertEqual(
            benchmark.probe_disable_candidates(catalog, ["PsiRule"], []),
            ["OtherPsi"],
        )

    def test_catalog_parse_finds_known_rules(self):
        rules = benchmark.catalog_rules(ROOT)
        names = {name for name, _phase in rules}
        self.assertIn("CopyPropagation", names)
        self.assertIn("ControlFlowCleanup", names)
        self.assertIn("Psi", {phase for _name, phase in rules})


if __name__ == "__main__":
    unittest.main()
