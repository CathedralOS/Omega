"""Confirmed L5 limits stay in the case ledger until a native test measures them. Run from the V2 workspace."""
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

import check_layer_readiness as readiness
import package_cases

CASE = "machine one() -> f32 { 1.0 }"
CONTROL = "machine one() -> u32 { 1 }"
ANCHORS = {"handoffs-and-remaining-work"}
REFUSAL = "case.omg:1:24: lowering: unsupported typed return record: Literal (bytes 23..26)"


def row(**fields):
    base = {"case": "float literal result", "spec": "wiki/spec/language/numeric_values.md#literal-landing-and-destinations",
            "contract": "CONTRACT.md#handoffs-and-remaining-work", "fixture": "float-literal", "file": "case.omg",
            "byte_start": "23", "byte_end": "26", "message": "unsupported typed return record: Literal",
            "justification": "The integer control lowers."}
    return {**base, **fields}


def report(**fields):
    base = {"last_layer": "L5", "measurement_errors": [], "cases": {}, "suites": {"L5": {"exit": 0, "counts": {}, "tests": []}},
            "unverified_contracts": [], "lowering_evidence": {"obligations": {}, "contract": [], "assertions": {}}}
    return {**base, **fields}


class Compiler:
    """Answers the package commands the way a compiler with the given case outcome would."""

    def __init__(self, lower=(1, "", REFUSAL + "\n"), prepare=(0, "", ""), control=(0, "", "")):
        self.lower, self.prepare, self.control, self.calls = lower, prepare, control, []

    def __call__(self, directory, action, file):
        self.calls.append((Path(directory).name, action))
        if Path(directory).name == "control":
            return self.control
        return self.prepare if action == "prepare" else self.lower


class PackageCaseTests(unittest.TestCase):
    def setUp(self):
        Path("target").mkdir(exist_ok=True)
        self.temp = TemporaryDirectory(dir="target")
        self.fixtures = Path(self.temp.name)
        for role, source in (("case", CASE), ("control", CONTROL)):
            (self.fixtures / "float-literal" / role).mkdir(parents=True)
            (self.fixtures / "float-literal" / role / "case.omg").write_text(source, encoding="utf-8", newline="\n")

    def tearDown(self):
        self.temp.cleanup()

    def measure(self, compiler, observed=None, rows=None):
        observed = observed or report()
        validate = package_cases.validate
        package_cases.validate = lambda rows, fixtures: validate(rows, fixtures, ANCHORS)
        try:
            package_cases.apply(observed, [row()] if rows is None else rows, compiler, self.fixtures)
        finally:
            package_cases.validate = validate
        return observed

    def test_a_reproduced_limit_is_an_open_gap_measured_only_by_the_package_command(self):
        compiler = Compiler()
        observed = self.measure(compiler)
        self.assertEqual(observed["measurement_errors"], [])
        case = observed["cases"]["handoff/float literal result"]
        self.assertEqual((case["status"], case["measured_by"], case["stage"]), ("open gap", "package command", "L5"))
        # The source command compiles its input files together as package root.
        self.assertEqual(case["fixture_locations"][0]["package"], "root")
        # Line and column come from the fixture bytes, not from the registry.
        self.assertEqual({k: case["fixture_locations"][0][k] for k in ("line", "byte_column", "occurrence")},
                         {"line": 1, "byte_column": 24, "occurrence": "1.0"})
        self.assertTrue(readiness.case_blocks_readiness(case))
        self.assertEqual(compiler.calls, [("control", "prepare"), ("control", "lower"), ("case", "prepare"), ("case", "lower")])

    def test_a_case_that_now_lowers_must_gain_a_native_measurement_before_its_row_goes(self):
        observed = self.measure(Compiler(lower=(0, "", "")))
        self.assertNotIn("handoff/float literal result", observed["cases"])
        self.assertEqual(observed["measurement_errors"], [
            "package case handoff/float literal result: its case now lowers; add a native product test and closure for it, then delete its row"])

    def test_a_changed_refusal_site_or_message_is_not_accepted(self):
        for stderr in (REFUSAL.replace("23..26", "22..26"), REFUSAL.replace("Literal", "Constant"),
                       REFUSAL.replace("lowering:", "typer:")):
            observed = self.measure(Compiler(lower=(1, "", stderr + "\n")))
            self.assertNotIn("handoff/float literal result", observed["cases"])
            self.assertIn("its refusal changed", observed["measurement_errors"][0])

    def test_an_abnormal_exit_does_not_verify_a_normal_refusal(self):
        observed = self.measure(Compiler(lower=(101, "", REFUSAL + "\n")))
        self.assertEqual(observed["package_cases"]["reproduced"], [])
        self.assertIn("exit 101", observed["measurement_errors"][0])

    def test_a_stopping_control_or_unprepared_case_is_an_error_not_a_gap(self):
        observed = self.measure(Compiler(control=(1, "", "case.omg:1:1: lowering: x (bytes 0..1)\n")))
        self.assertIn("its control no longer prepares and lowers silently", observed["measurement_errors"][0])
        observed = self.measure(Compiler(prepare=(1, "", "case.omg:1:1: typer: x (bytes 0..1)\n")))
        self.assertIn("its case no longer prepares silently", observed["measurement_errors"][0])
        self.assertEqual(observed["cases"], {})

    def test_a_native_case_with_the_same_label_reconciles_the_row_instead_of_hiding_it(self):
        native = {"handoff/float literal result": {"cohort": "handoff", "stage": "L5", "status": "Psi product compared", "detail": "closure"}}
        compiler = Compiler()
        observed = self.measure(compiler, report(cases=dict(native)))
        self.assertEqual(observed["cases"], native)
        self.assertEqual(compiler.calls, [])
        self.assertIn("the native ledger already reports it (Psi product compared); delete its", observed["measurement_errors"][0])

    def test_without_a_built_cli_every_row_is_unmeasured_and_blocks(self):
        observed = report()
        validate = package_cases.validate
        package_cases.validate = lambda rows, fixtures: validate(rows, fixtures, ANCHORS)
        try:
            package_cases.apply(observed, [row()], None, self.fixtures)
        finally:
            package_cases.validate = validate
        self.assertEqual(observed["measurement_errors"], ["1 confirmed L5 limits unmeasured: no package CLI was built"])
        self.assertEqual(observed["package_cases"]["unreconciled"], {"float literal result": "unmeasured: no package CLI was built"})

    def test_invalid_rows_are_rejected_before_any_command_runs(self):
        for bad in (row(contract="CONTRACT.md#line-97"), row(spec="wiki/spec/language/numeric_values.md"),
                    row(fixture="missing"), row(byte_end="99"), row(message=""), row(fixture="Float Literal")):
            with self.assertRaises(ValueError, msg=bad):
                package_cases.validate([bad], self.fixtures, ANCHORS)
        with self.assertRaises(ValueError):
            package_cases.validate([row(), row()], self.fixtures, ANCHORS)

    def test_the_real_registry_names_existing_fixtures_contract_sections_and_byte_ranges(self):
        rows = package_cases.validate(package_cases.registrations())
        self.assertTrue(rows)
        self.assertEqual(len({r["case"] for r in rows}), len(rows))


class CompletionTests(unittest.TestCase):
    def test_package_command_cases_and_unverified_obligations_are_counted_apart(self):
        observed = report(cases={
            "handoff/a": {"cohort": "handoff", "stage": "L5", "status": "open gap", "detail": "", "measured_by": "package command"},
            "handoff/b": {"cohort": "handoff", "stage": "L5", "status": "open gap", "detail": ""},
            "handoff/c": {"cohort": "handoff", "stage": "L5", "status": "checker input compared; judgement pending", "detail": ""}},
            lowering_evidence={"obligations": {"EvaluateAccessPlan": {"state": "reached L5, every input refused"}},
                               "contract": [], "assertions": {}})
        text = "\n".join(readiness.completion(observed))
        self.assertIn("**Not complete through L5.**", text)
        self.assertIn("| Executable cases measured by native tests | program | 1 | yes |", text)
        self.assertIn("| Executable cases measured only by the package command (no Psi product or consumer check) | program | 1 | yes |", text)
        self.assertIn("| Typed obligation kinds without a credited Psi property | unverified obligation | 1 | yes |", text)
        self.assertIn("| Cases whose Psi is compared and await the L6 judgement | program | 1 | no (L6 owns the verdict) |", text)
        self.assertIn("Unverified obligations: EvaluateAccessPlan.", text)

    def test_an_unverified_obligation_alone_still_refuses_completion(self):
        observed = report(lowering_evidence={"obligations": {"VerifyLibraryImplementation": {"state": "lowered in a passing test, no Psi property checked"}},
                                             "contract": [], "assertions": {}})
        self.assertIn("**Not complete through L5.**", "\n".join(readiness.completion(observed)))

    def test_an_unreconciled_confirmed_limit_refuses_completion(self):
        observed = report(package_cases={"registered": 1, "reproduced": [], "unreconciled": {"x": "its case now lowers"}},
                          measurement_errors=["package case handoff/x: its case now lowers"])
        text = "\n".join(readiness.completion(observed))
        self.assertIn("| Confirmed limits whose package measurement is unreconciled | program | 1 | yes |", text)
        self.assertIn("**Not complete through L5.**", text)

    def test_only_an_all_zero_measurement_is_complete(self):
        self.assertIn("**All measured checks through L5 passed.**", "\n".join(readiness.completion(report())))
        self.assertEqual(readiness.completion(report(last_layer="L4")), [])


if __name__ == "__main__":
    unittest.main()
