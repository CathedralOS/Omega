"""Every owned preparation reason needs a typed witness; a broken roster is not missing coverage."""
from pathlib import Path
import json
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import preparation_reason_evidence as evidence
from check_layer_readiness import collect, readiness_exit
from diagnostic_fields import unescape
from test_l5_records import libtest

ROOT = Path(__file__).absolute().parent.parent.parent
PREFIX = "static evaluation cannot admit this invocation: "
TEXTS = {"ZeroDivisor": "integer division or remainder requires a nonzero divisor", "CyclicConstantInitializer": "cyclic constant initializer"}
UNIT, ROSTER_TEST = evidence.INVENTORY


def roster(*names):
    return f"OMEGA_PREPARATION_REASONS\t{','.join(names or TEXTS)}"


def witness(reason="ZeroDivisor", actual=None, message=None, package="app", file="main.omg", start=10, end=15, size=None):
    message = PREFIX + TEXTS.get(reason, "other") if message is None else message
    if size is None:
        try:
            size = len(unescape(message))
        except ValueError:
            size = len(message.encode())
    return f"OMEGA_PREPARATION_REASON\t{reason}\t{actual or reason}\t{package}\t{file}\t{start}\t{end}\t{size}\t{message}"


def forwarded(family="TargetLayout", message="the selected target has no validated layout for this type"):
    return f"OMEGA_PREPARATION_FORWARDED\t{family}\t{message}\tapp\tmain.omg\t3\t9\t{len((PREFIX + message).encode())}\t{PREFIX}{message}"


class PreparationReasonTests(unittest.TestCase):
    def inspect(self, *targets, inventory=None, roster_status="ok", texts=TEXTS):
        """`targets` are (target, (test, status, lines)...) blocks of the L4 suite."""
        report = {"expected": {}, "cases": {}, "measurement_errors": [], "suites": {}, "package_fixtures": [], "obligations": {}}
        blocks = [(UNIT, (ROSTER_TEST, roster_status, [roster() if inventory is None else inventory]))] if inventory != "" else []
        log = "\n".join(libtest(target, *tests) for target, *tests in [*blocks, *targets])
        tests, _ = collect(log, "L4", report)
        report["suites"]["L4"] = {"tests": tests, "exit": 0}
        with patch.object(evidence, "declared", return_value=dict(texts)):
            report["preparation_reasons"] = evidence.measure(report)
        return report

    def both(self):
        return ("tests/integer_constants.rs", ("zero", "ok", [witness()]), ("cycle", "ok", [witness("CyclicConstantInitializer")]))

    def test_each_declared_reason_with_a_passing_typed_witness_is_verified(self):
        report = self.inspect(self.both())
        result = report["preparation_reasons"]
        self.assertEqual(report["measurement_errors"], [])
        self.assertEqual((result["measured"], result["missing"]), (True, []))
        self.assertEqual(result["reasons"]["ZeroDivisor"]["witnesses"][0]["byte_start"], 10)
        self.assertEqual(readiness_exit(report), 0)

    def test_a_declared_reason_without_a_witness_is_missing_coverage_not_a_broken_measurement(self):
        report = self.inspect(("tests/integer_constants.rs", ("zero", "ok", [witness()])))
        result = report["preparation_reasons"]
        self.assertEqual(report["measurement_errors"], [])
        self.assertEqual((result["measured"], result["missing"]), (True, ["CyclicConstantInitializer"]))
        self.assertEqual(readiness_exit(report), 1)
        self.assertIn("1/2 owned preparation reasons", evidence.summary(result)[0])
        self.assertIn("- CyclicConstantInitializer · missing · cyclic constant initializer", evidence.details(result))

    def test_an_untrusted_roster_leaves_coverage_unmeasured(self):
        faults = [{"inventory": ""}, {"inventory": roster("ZeroDivisor", "ZeroDivisor", "CyclicConstantInitializer")},
                  {"inventory": roster() + ","}, {"inventory": roster() + "\textra"}, {"roster_status": "FAILED"},
                  {"inventory": roster("ZeroDivisor")}, {"inventory": roster("CyclicConstantInitializer", "ZeroDivisor")},
                  {"texts": {**TEXTS, "LandingNotIntegral": "the typed landing does not supply an integral value"}}]
        for fault in faults:
            with self.subTest(fault=fault):
                report = self.inspect(self.both(), **fault)
                result = report["preparation_reasons"]
                self.assertFalse(result["measured"])
                self.assertEqual(result["reasons"], {})
                self.assertTrue(report["measurement_errors"])
                self.assertEqual(readiness_exit(report), 1)

    def test_the_roster_must_come_from_its_one_trusted_test(self):
        for blocks in [[("tests/other.rs", ("listing", "ok", [roster()]))],
                       [(UNIT, (ROSTER_TEST, "ok", [roster(), roster()]))],
                       [(UNIT, ("reasons::tests::another", "ok", [roster()]))]]:
            with self.subTest(blocks=blocks):
                report = self.inspect(*blocks, self.both(), inventory="")
                self.assertFalse(report["preparation_reasons"]["measured"])
                self.assertTrue(report["measurement_errors"])

    def test_wrong_emitter_cause_location_or_text_cannot_verify_a_reason(self):
        target = "tests/integer_constants.rs"
        faults = [("tests/integer_constants.rs", ("zero", "FAILED", [witness()])),
                  (target, ("zero", "ok", [witness(actual="CyclicConstantInitializer")])),
                  (target, ("zero", "ok", [witness(size=3)])),
                  (target, ("zero", "ok", [witness(start=16)])),
                  (target, ("zero", "ok", [witness(package="")])),
                  (target, ("zero", "ok", [witness(message=PREFIX + "cyclic constant initializer")])),
                  (target, ("zero", "ok", [witness(message=PREFIX + TEXTS["ZeroDivisor"] + "\x0b")])),
                  (target, ("zero", "ok", [witness(message=PREFIX + TEXTS["ZeroDivisor"] + "%0")])),
                  (target, ("zero", "ok", [witness("Undeclared", message=PREFIX + "undeclared")])),
                  (target, ("zero", "ok", [witness(), witness()])),
                  (target, ("zero", "ok", [witness() + "\textra"])),
                  (target, ("zero", "ok", [witness().replace("OMEGA_PREPARATION_REASON", "OMEGA_PREPARATION_REASONZ")]))]
        for fault in faults:
            with self.subTest(fault=fault):
                report = self.inspect(fault)
                result = report["preparation_reasons"]
                self.assertTrue(result["measured"])
                self.assertIn("ZeroDivisor", result["missing"])
                self.assertEqual(readiness_exit(report), 1)
                if fault[1][1] == "FAILED":
                    self.assertEqual(result["reasons"]["ZeroDivisor"]["state"], "witness in a failed test")
                else:
                    self.assertTrue(report["measurement_errors"])

    def test_context_cannot_supply_the_primary_preparation_reason(self):
        reason = TEXTS["ZeroDivisor"]
        for message in ("resumed typing: " + reason, PREFIX + reason + " was not the cause",
                        PREFIX + TEXTS["CyclicConstantInitializer"] + "%0Acall chain: " + reason):
            with self.subTest(message=message):
                report = self.inspect(("tests/integer_constants.rs", ("wrong_reason", "ok", [witness(message=message)])))
                self.assertIn("ZeroDivisor", report["preparation_reasons"]["missing"])
                self.assertIn("primary diagnostic", report["measurement_errors"][0])

    def test_an_unsupported_float_reason_precedes_its_target_context(self):
        name = "UnsupportedFloatOperation"
        text = "the selected target does not support this floating operation"
        for message, valid in [(text + " (Binary32 Add on test)", True), ("failed (Binary32 Add on " + text + ")", False)]:
            with self.subTest(message=message):
                report = self.inspect(("tests/owned_floats.rs", ("unsupported", "ok", [witness(name, message=message)])),
                                      inventory=roster(name), texts={name: text})
                self.assertEqual(report["preparation_reasons"]["missing"], [] if valid else [name])
                self.assertEqual(bool(report["measurement_errors"]), not valid)

    def test_records_outside_a_captured_l4_test_are_misplaced(self):
        report = {"expected": {}, "cases": {}, "measurement_errors": [], "suites": {}, "package_fixtures": [], "obligations": {}}
        collect(libtest("tests/typing_cases.rs", ("zero", "ok", [witness()])), "CLI", report)
        collect("\n".join([libtest(UNIT, (ROSTER_TEST, "ok", [roster()])), witness()]), "L4", report)
        self.assertEqual(len(report["measurement_errors"]), 2)
        self.assertEqual(report[evidence.KEY]["witnesses"], [])

    def test_a_package_cause_requires_the_same_location_and_reason_in_the_verified_cli_case(self):
        for mismatch in (False, True):
            report = {"cases": {}, "measurement_errors": [], "suites": {}, "expected": {}, "obligations": {}, "package_fixtures": []}
            records, _ = collect(libtest(UNIT, (ROSTER_TEST, "ok", [roster()])), "L4", report)
            report["suites"]["L4"] = {"tests": records}
            records, _ = collect(libtest("tests/preparation_cases.rs", ("zero", "ok", [witness(package="root")])), "CLI", report)
            report["suites"]["CLI"] = {"tests": records}
            report["rule_diagnostics"] = {"L4": {"zero": {"verified": True, "observation": {
                "file": "main.omg", "byte_start": 9 if mismatch else 10, "byte_end": 15, "reason": PREFIX + TEXTS["ZeroDivisor"]}}}}
            with patch.object(evidence, "declared", return_value=TEXTS), patch.object(evidence, "preparation_causes", return_value={"zero": "preparation:ZeroDivisor"}):
                row = evidence.measure(report)["reasons"]["ZeroDivisor"]
            self.assertEqual(row["state"], "missing" if mismatch else "verified")
            self.assertEqual(bool(report["measurement_errors"]), mismatch)

    def test_a_valid_witness_cannot_mask_corruption_in_the_same_test(self):
        report = self.inspect(("tests/integer_constants.rs", ("zero", "ok", [witness(), witness(size=1, start=0, end=1)])))
        self.assertTrue(report["measurement_errors"])
        self.assertEqual(report["preparation_reasons"]["reasons"]["ZeroDivisor"]["state"], "missing")

    def test_forwarded_causes_are_listed_but_never_cover_an_owned_reason(self):
        report = self.inspect(("tests/target_layout.rs", ("layout", "ok", [forwarded(), forwarded("IntegerPrimitive", "shift exceeds width")])))
        result = report["preparation_reasons"]
        self.assertEqual(report["measurement_errors"], [])
        self.assertEqual([row["cause"] for row in result["forwarded"]], ["TargetLayout", "IntegerPrimitive"])
        self.assertEqual(result["missing"], list(TEXTS))
        self.assertIn("TargetLayout (distinct cause texts observed at the API: 1, through package commands: 0)", evidence.summary(result)[1])
        report = self.inspect(("tests/target_layout.rs", ("layout", "ok", [forwarded("ZeroDivisor")])))
        self.assertTrue(report["measurement_errors"])
        self.assertEqual(report["preparation_reasons"]["forwarded"], [])

    def test_an_unlocated_refusal_is_accepted_only_with_an_empty_range(self):
        text = "assembly catalog and preparation selected different targets"
        texts = {"AssemblyTargetMismatch": text}
        line = f"OMEGA_PREPARATION_REASON\tAssemblyTargetMismatch\tAssemblyTargetMismatch\t\t\t0\t0\t{len(text)}\t{text}"
        for record, verified in [(line, True), (line.replace("\t0\t0\t", "\t0\t1\t"), False)]:
            with self.subTest(record=record):
                report = self.inspect(("tests/assembly.rs", ("mismatch", "ok", [record])),
                                      inventory=roster("AssemblyTargetMismatch"), texts=texts)
                self.assertEqual(report["preparation_reasons"]["missing"] == [], verified)
                self.assertEqual(bool(report["measurement_errors"]), not verified)

    def test_control_characters_round_trip_losslessly_into_the_report(self):
        message = PREFIX + TEXTS["ZeroDivisor"] + "\ncall chain: a -> b\t%0A\u0085\u2028"
        encoded = evidence.display(message)
        self.assertEqual(encoded, PREFIX + TEXTS["ZeroDivisor"] + "%0Acall chain: a -> b%09%250A%C2%85%E2%80%A8")
        report = self.inspect(("tests/integer_constants.rs", ("zero", "ok", [witness(message=encoded, package="app%09x")]),
                               ("cycle", "ok", [witness("CyclicConstantInitializer")])))
        self.assertEqual(report["measurement_errors"], [])
        row = report["preparation_reasons"]["reasons"]["ZeroDivisor"]["witnesses"][0]
        self.assertEqual((row["reason"], row["package"]), (message, "app\tx"))
        line = next(line for line in evidence.details(report["preparation_reasons"]) if r"app\tx" in line)
        self.assertIn(r"call chain: a -> b\t%0A\u0085\u2028", line)
        self.assertFalse(any(c in line for c in "\n\t\u0085\u2028"))
        self.assertEqual(json.loads(line.rsplit(" · ", 1)[1]), message)

    def test_the_declaration_parser_refuses_unfamiliar_lines(self):
        declaration = ("macro_rules! preparation_reasons { () => {} }\n\npreparation_reasons! {\n    // Section.\n    /// Doc.\n"
                       "    ZeroDivisor => \"integer division \\\"x\\\"\",\n    CyclicConstantInitializer => \"cyclic\",\n}\n")
        with TemporaryDirectory() as directory:
            path = Path(directory) / "reasons.rs"
            path.write_text(declaration, encoding="utf-8")
            self.assertEqual(evidence.declared(path), {"ZeroDivisor": "integer division \"x\"", "CyclicConstantInitializer": "cyclic"})
            for broken in (declaration.replace("    /// Doc.\n", "    #[cfg(test)]\n"),
                           declaration.replace("CyclicConstantInitializer", "ZeroDivisor"),
                           declaration.replace("\"cyclic\",", "\"cyclic\""),
                           declaration + declaration[declaration.index("preparation_reasons! {"):],
                           declaration.replace("preparation_reasons! {", "other! {")):
                with self.subTest(broken=broken):
                    path.write_text(broken, encoding="utf-8")
                    with self.assertRaises(ValueError):
                        evidence.declared(path)

    def test_the_actual_declaration_parses(self):
        reasons = evidence.declared(ROOT / evidence.DECLARATION)
        self.assertIn("ZeroDivisor", reasons)
        self.assertEqual(len(reasons), len(set(reasons)))


if __name__ == "__main__":
    unittest.main()
