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
            report["reviewed_comparisons"][comparison] = None
            self.assertFalse(frontend_diagnostics.preparation_phases(report)["ResumedTyping"]["verified"])
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

    def measure(self, log, stage="L0", kind="InvalidUtf8"):
        report = {"measurement_errors": [], "suites": {"L1": {"tests": []}}}
        with patch.dict(evidence.FAMILIES, {stage: evidence.FAMILIES[stage]}, clear=True), patch.object(evidence, "kinds", return_value={kind}):
            row = evidence.measure(report, log)[stage][kind]
        return row["state"], report["measurement_errors"]

    def test_missing_failed_or_misattributed_diagnostic_never_gets_credit(self):
        name = "public_diagnostics_match_the_lex_errors_inventory"
        diagnostic = "bad.omg:1:5: lexer: source is not valid UTF-8 (bytes 4..5)"
        receipt = f"OMEGA_DIAGNOSTIC_KIND\tL0\tInvalidUtf8\tInvalidUtf8\t{diagnostic}\t{diagnostic}\tsource is not valid UTF-8 (bytes 4..5)\tbad.omg\tabcd%FF\t4\t%FF"
        log = f"Running tests/lex_errors.rs (binary)\ntest {name} ... ok\n---- {name} stdout ----\n{receipt}"
        self.assertEqual(self.measure(log), ("CLI verified", []))
        self.assertEqual(self.measure(log.replace(" ... ok", " ... FAILED")), ("matched in a failed test", []))
        for broken in (log.replace(receipt, ""), log.replace("tests/lex_errors", "tests/unrelated"),
                       log + "\n" + receipt, log + "\n" + receipt.replace("4..5", "4..1"),
                       log + "\n" + receipt.rsplit("\t", 1)[0],
                       log.replace("\tsource is not valid UTF-8 (bytes 4..5)", ""),  # historical, CLI-only receipt
                       log.replace("\tsource is not valid UTF-8", "\tinvalid UTF-8"),
                       log.replace("(bytes 4..5)\tbad.omg", "(bytes 4..6)\tbad.omg"),
                       log.replace(f"test {name} ... ok\n", "") + f"\ntest {name} ... ok",
                       log + f"\n---- {name} stdout ----",
                       log.replace(receipt, "---- other stderr ----\n" + receipt),
                       log.replace("OMEGA_DIAGNOSTIC_KIND", "OMEGA_DIAGNOSTIC_KIND_BAD"), log.replace("4..5", "4..1")):
            with self.subTest(broken=broken):
                state, errors = self.measure(broken)
                self.assertTrue(errors)
                self.assertEqual(state, "unobserved")

    def test_a_swapped_kind_or_rendered_diagnostic_loses_credit_in_a_passing_suite(self):
        for stage, binary, producer in [("L0", "lex_errors", "lexer"), ("L1", "parse_errors", "parser")]:
            test = f"public_diagnostics_match_the_{binary}_inventory"
            header = f"Running tests/{binary}.rs (binary)\ntest {test} ... ok\n---- {test} stdout ----\n"
            first = f"case.omg:1:2: {producer}: first failure (bytes 1..2)"
            second = f"case.omg:2:3: {producer}: second failure (bytes 8..9)"
            with patch.dict(evidence.FAMILIES, {stage: (binary, "", "")}, clear=True), patch.object(evidence, "kinds", return_value={"First", "Second"}):
                for actual_kind, actual in [("Second", first), ("First", second), ("First", first.replace("1:2:", "2:2:"))]:
                    with self.subTest(stage=stage, actual_kind=actual_kind, actual=actual):
                        log = header + f"OMEGA_DIAGNOSTIC_KIND\t{stage}\tFirst\t{actual_kind}\t{first}\t{actual}\tfirst failure (bytes 1..2)\tcase.omg\txy\t1\ty\n"
                        log += f"OMEGA_DIAGNOSTIC_KIND\t{stage}\tSecond\tSecond\t{second}\t{second}\tsecond failure (bytes 8..9)\tcase.omg\t12345%0Axy?\t8\t?"
                        report = {"measurement_errors": []}
                        rows = evidence.measure(report, log)[stage]
                        self.assertEqual(rows["First"]["state"], "unobserved")
                        self.assertEqual(rows["Second"]["state"], "CLI verified")
                        self.assertTrue(report["measurement_errors"])

    def test_matching_strings_do_not_prove_a_source_occurrence(self):
        name = "public_diagnostics_match_the_lex_errors_inventory"
        header = f"Running tests/lex_errors.rs (binary)\ntest {name} ... ok\n---- {name} stdout ----\n"
        # The second '?' follows CRLF, a two-byte character, a literal % and an invalid byte.
        source = "?%0D%0A%C3%A9%25%FF?"

        def log(location="2:5", span="7..8", source=source, offset="7", spelling="?", file="case.omg"):
            diagnostic = f"case.omg:{location}: lexer: reason (bytes {span})"
            return header + "\t".join(["OMEGA_DIAGNOSTIC_KIND", "L0", "InvalidUtf8", "InvalidUtf8",
                                       diagnostic, diagnostic, f"reason (bytes {span})", file, source, offset, spelling])

        for valid in [log(), log("1:3", "2..3", offset="2", spelling="%0A")]:
            self.assertEqual(self.measure(valid), ("CLI verified", []))
        for broken in [log(span="7..9"), log("2:7", "9..9", offset="9", spelling=""),
                       log("1:1", "0..1"), log(span="7..7", spelling=""), log(spelling="!"),
                       log(location="3:5"), log(location="2:4"), log(location="2:05"), log(span="07..8"), log(offset="+7"), log(file="other.omg"),
                       log(source=source.replace("%FF", "%FG")), log(spelling="%"), log(spelling="?\vbad"),
                       log().rsplit("\t", 4)[0]]:
            with self.subTest(broken=broken):
                state, errors = self.measure(broken)
                self.assertEqual(state, "unobserved")
                self.assertTrue(errors)

    def test_parser_empty_spelling_is_valid_at_the_selected_eof_only(self):
        name = "public_diagnostics_match_the_parse_errors_inventory"
        header = f"Running tests/parse_errors.rs (binary)\ntest {name} ... ok\n---- {name} stdout ----\n"
        api = "expected `}` (bytes 15..15)"
        text = f"f.omg:2:1: parser: {api}"
        receipt = f"OMEGA_DIAGNOSTIC_KIND\tL1\tUnexpectedEnd\tUnexpectedEnd\t{text}\t{text}\t{api}\tf.omg\tmachine f() {{%0D%0A\t15\t"

        def measure(record):
            return self.measure(header + record, "L1", "UnexpectedEnd")

        self.assertEqual(measure(receipt), ("CLI verified", []))
        for broken in (receipt.replace("..15", "..16"), receipt.replace("15..15", "14..14"),
                       receipt.replace("2:1:", "1:1:"), receipt.replace("%0A\t", "%0Ax\t"), receipt.rstrip("\t")):
            with self.subTest(broken=broken):
                state, errors = measure(broken)
                self.assertEqual(state, "unobserved")
                self.assertTrue(errors)

    def test_index_capacity_has_separate_direct_evidence(self):
        parser_test = {"target": "unittests src/lib.rs", "test": evidence.INDEX_TEST, "status": "ok"}
        report = {"measurement_errors": [], "suites": {"L1": {"tests": [parser_test]}}}
        with patch.dict(evidence.FAMILIES, {"L1": ("parse_errors", "", "")}, clear=True), patch.object(evidence, "kinds", return_value={"SourceTooLarge"}):
            self.assertEqual(evidence.measure(report, "")["L1"]["SourceTooLarge"]["state"], "parser boundary verified")
            for tests in ([], [parser_test, parser_test], [dict(parser_test, target="tests/other.rs")],
                          [dict(parser_test, status="ignored")], [dict(parser_test, status="FAILED")],
                          [dict(parser_test, test="parsing::syntax_parser::tests::token_indices_report_the_first_unrepresentable_position")]):
                report["suites"]["L1"]["tests"] = tests
                report["measurement_errors"].clear()
                self.assertEqual(evidence.measure(report, "")["L1"]["SourceTooLarge"]["state"], "unobserved")
                self.assertEqual(report["measurement_errors"], ["L1 SourceTooLarge: missing verified diagnostic"])

    def test_capacity_cannot_claim_cli_coverage_even_with_a_passing_boundary_test(self):
        name = "public_diagnostics_match_the_parse_errors_inventory"
        report = {"measurement_errors": [], "suites": {"L1": {"tests": [
            {"target": "unittests src/lib.rs", "test": evidence.INDEX_TEST, "status": "ok"}]}}}
        text = "f.omg:1:1: parser: overflow (bytes 0..1)"
        record = f"OMEGA_DIAGNOSTIC_KIND\tL1\tSourceTooLarge\tSourceTooLarge\t{text}\t{text}\toverflow (bytes 0..1)\tf.omg\tx\t0\tx"
        log = f"Running tests/parse_errors.rs (binary)\ntest {name} ... ok\n---- {name} stdout ----\n{record}"
        with patch.dict(evidence.FAMILIES, {"L1": evidence.FAMILIES["L1"]}, clear=True), patch.object(evidence, "kinds", return_value={"SourceTooLarge"}):
            rows = evidence.measure(report, log)
            self.assertEqual(rows["L1"]["SourceTooLarge"]["state"], "unobserved")
            self.assertTrue(report["measurement_errors"])

    def test_gate_requires_each_declared_kind_and_the_correct_evidence_route(self):
        with patch.dict(evidence.FAMILIES, {"L1": evidence.FAMILIES["L1"]}, clear=True), patch.object(evidence, "kinds", return_value={"UnexpectedEnd", "SourceTooLarge"}):
            boundary = {"state": "parser boundary verified", "test": evidence.INDEX_TEST}
            cli = {"state": "CLI verified", "file": "f.omg", "line": 1, "byte_column": 1, "byte_start": 0, "byte_end": 0,
                   "reason": "expected `}`", "api_display": "expected `}` (bytes 0..0)"}
            rows = {"UnexpectedEnd": cli, "SourceTooLarge": boundary}
            self.assertFalse(evidence.blocks({"L1": rows}))
            for inventory in (None, {}, {"L1": {}}, {"L1": {"SourceTooLarge": boundary}},
                              {"L1": dict(rows, SourceTooLarge=cli)}, {"L1": dict(rows, UnexpectedEnd=boundary)},
                              {"L1": dict(rows, UnexpectedEnd=dict(cli, byte_column=None))},
                              {"L1": dict(rows, UnexpectedEnd=dict(cli, api_display="expected `}` (bytes 0..1)"))},
                              {"L1": dict(rows, UnexpectedEnd={"state": "CLI verified"})}):
                self.assertTrue(evidence.blocks(inventory))
