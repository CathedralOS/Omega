"""A CLI result must come from the binary exercising that declared diagnostic."""
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

import diagnostic_kinds as evidence
import frontend_diagnostics


class DiagnosticKindTests(unittest.TestCase):
    def test_a_new_preparation_variant_stays_visible_without_a_test(self):
        with TemporaryDirectory() as directory:
            declaration = Path(directory) / "phases.rs"
            declaration.write_text("pub enum PreparationFailure {\n    ResumedTyping(TypeError),\n    NewPhase(TypeError),\n}\n", encoding="utf-8")
            declared = evidence.kinds(declaration, "PreparationFailure", payload="TypeError")
        self.assertEqual(declared, {"ResumedTyping", "NewPhase"})
        comparison, cli = frontend_diagnostics.PREPARATION_TESTS["ResumedTyping"]
        source = {"test": "retype_after_evaluated_identity_is_resumed_typing", "status": "ok", "target": "tests/preparation_failure_phases.rs"}
        report = {"measurement_errors": [], "suites": {"L4": {"tests": [source]}},
                  "reviewed_comparisons": {comparison: "asserted"}, "frontend_diagnostics": {"cli": {cli: {}}}}
        with patch.object(evidence, "kinds", return_value=declared):
            states = frontend_diagnostics.preparation_phases(report)
            self.assertTrue(states["ResumedTyping"]["verified"])
            self.assertFalse(states["NewPhase"]["verified"])
            self.assertIn("preparation failure NewPhase: no producer and CLI comparison registered", report["measurement_errors"])
            report["reviewed_comparisons"].clear()
            self.assertFalse(frontend_diagnostics.preparation_phases(report)["ResumedTyping"]["verified"])
            report["reviewed_comparisons"][comparison] = "asserted"
            report["frontend_diagnostics"]["cli"].clear()
            self.assertFalse(frontend_diagnostics.preparation_phases(report)["ResumedTyping"]["verified"])

    def test_preparation_phases_may_carry_a_typed_refusal_but_no_unknown_payload(self):
        with TemporaryDirectory() as directory:
            declaration = Path(directory) / "phases.rs"
            declaration.write_text("pub enum PreparationFailure {\n    Typing(TypeError),\n    Preparation(PreparationRefusal),\n}\n", encoding="utf-8")
            declared = evidence.kinds(declaration, "PreparationFailure", payload=frontend_diagnostics.PREPARATION_PAYLOADS)
            self.assertEqual(declared, {"Typing", "Preparation"})
            with self.assertRaises(ValueError):
                evidence.kinds(declaration, "PreparationFailure", payload="TypeError")
            for payload in ("Box<PreparationRefusal>", "RefusalCause", "TypeError, RefusalCause"):
                with self.subTest(payload=payload):
                    declaration.write_text(f"pub enum PreparationFailure {{\n    Typing(TypeError),\n    Preparation({payload}),\n}}\n", encoding="utf-8")
                    with self.assertRaises(ValueError):
                        evidence.kinds(declaration, "PreparationFailure", payload=frontend_diagnostics.PREPARATION_PAYLOADS)

    def measure(self, log):
        report = {"measurement_errors": [], "suites": {"L1": {"tests": []}}}
        with patch.dict(evidence.FAMILIES, {"L0": ("lex_errors", "", "")}, clear=True), patch.object(evidence, "kinds", return_value={"InvalidUtf8"}):
            row = evidence.measure(report, log)["L0"]["InvalidUtf8"]
        return row["state"], report["measurement_errors"]

    def test_missing_failed_or_misattributed_diagnostic_never_gets_credit(self):
        name = "public_diagnostics_match_the_lex_errors_inventory"
        receipt = "OMEGA_DIAGNOSTIC_KIND\tL0\tInvalidUtf8\tbad.omg\t4\t5\tsource is not valid UTF-8"
        log = f"Running tests/lex_errors.rs (binary)\ntest {name} ... ok\n---- {name} stdout ----\n{receipt}"
        self.assertEqual(self.measure(log), ("CLI verified", []))
        self.assertEqual(self.measure(log.replace(" ... ok", " ... FAILED")), ("matched in a failed test", []))
        for broken in (log.replace(receipt, ""), log.replace("tests/lex_errors", "tests/unrelated"),
                       log + "\n" + receipt, log + "\n" + receipt.replace("\t5\t", "\t1\t"),
                       log + "\n" + receipt.rsplit("\t", 1)[0],
                       log.replace("OMEGA_DIAGNOSTIC_KIND", "OMEGA_DIAGNOSTIC_KIND_BAD"), log.replace("\t5\t", "\t1\t")):
            with self.subTest(broken=broken):
                state, errors = self.measure(broken)
                self.assertTrue(errors)
                self.assertEqual(state, "unobserved")

    def test_index_capacity_has_separate_direct_evidence(self):
        parser_test = {"target": "unittests src/lib.rs", "test": evidence.INDEX_TEST, "status": "ok"}
        report = {"measurement_errors": [], "suites": {"L1": {"tests": [parser_test]}}}
        with patch.dict(evidence.FAMILIES, {"L1": ("parse_errors", "", "")}, clear=True), patch.object(evidence, "kinds", return_value={"SourceTooLarge"}):
            self.assertEqual(evidence.measure(report, "")["L1"]["SourceTooLarge"]["state"], "parser boundary verified")
            parser_test["target"] = "tests/other.rs"
            self.assertEqual(evidence.measure(report, "")["L1"]["SourceTooLarge"]["state"], "unobserved")
            self.assertEqual(report["measurement_errors"], ["L1 SourceTooLarge: missing verified diagnostic"])
