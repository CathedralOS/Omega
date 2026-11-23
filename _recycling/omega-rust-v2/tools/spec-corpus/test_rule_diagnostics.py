import unittest

import rule_diagnostics
from check_layer_readiness import reflection_exports


class ContractDiagnostics(unittest.TestCase):
    target = "tests/resolver_cases.rs"
    test = "checks_a_name"
    rule = "Imports"
    marker = "OMEGA_RULE_DIAGNOSTIC\tL2\tchecks_a_name\tImports\tresolver\tmodule.omg\t8\t11\t1\t9\tparsed\t19\tunresolved name Foo"

    def measure(self, marker, status="ok", binary=None, repeats=1):
        report = {"measurement_errors": [], "suites": {"CLI": {"tests": [{"target": self.target, "test": self.test, "status": status}] * repeats}},
                  "handoff_rules": {"L2": [{"rule": self.rule, "line": 42}]}}
        log = f"Running {binary or self.target} (binary)\ntest {self.test} ... {status}\n---- {self.test} stdout ----\n{marker}\n"
        results = rule_diagnostics.measure(report, log, {("L2", self.test): self.rule})
        return results["L2"][self.test], report["measurement_errors"]

    def test_refusal_is_attributed_to_its_test_layer_and_contract(self):
        witness, errors = self.measure(self.marker)
        self.assertEqual(errors, [])
        self.assertEqual((witness["verified"], witness["contract_line"], witness["observation"]["byte_start"]), (True, 42, 8))
        for text in ("", self.marker + "\n" + self.marker, self.marker.replace("parsed", "typed"), self.marker.replace("resolver\t", "typer\t"), self.marker.replace("Imports", "Names"), self.marker.replace("8\t11", "11\t8")):
            with self.subTest(text=text):
                witness, errors = self.measure(text)
                self.assertFalse(witness["verified"])
                self.assertTrue(errors)
        witness, errors = self.measure(self.marker, binary="tests/typing_cases.rs")
        self.assertFalse(witness["verified"])
        self.assertTrue(errors)
        witness, errors = self.measure(self.marker, repeats=2)
        self.assertFalse(witness["verified"])
        self.assertTrue(errors)
        witness, errors = self.measure(self.marker, status="FAILED")
        self.assertFalse(witness["verified"])
        self.assertEqual(errors, [])

    def test_malformed_or_misattributed_record_invalidates_its_emitting_test(self):
        for bad in ("OMEGA_RULE_DIAGNOSTIC", self.marker.replace("checks_a_name", "other_test"),
                    self.marker.replace("L2", "L3")):
            for pair in ((bad, self.marker), (self.marker, bad)):
                with self.subTest(records=pair):
                    witness, errors = self.measure("\n".join(pair))
                    self.assertTrue(errors)
                    self.assertFalse(witness["verified"])

    def test_multiline_reason_round_trips_without_changing_literal_percent_escapes(self):
        expected = "unresolved name Foo\ncall chain: a -> b\t%0A"
        marker = self.marker.replace("\t19\t", f"\t{len(expected.encode())}\t") + "%0Acall chain: a -> b%09%250A"
        witness, errors = self.measure(marker)
        self.assertEqual(errors, [])
        self.assertEqual(witness["observation"]["reason"], expected)
        witness, errors = self.measure(marker.replace("%0Acall", "\ncall"))
        self.assertFalse(witness["verified"])
        self.assertTrue(errors)

    def test_absent_export_requires_an_observation_from_the_supplied_package(self):
        def record(member, state):
            return f"OMEGA_CONTEXT_EXPORT\tlanguage/reflection.md:77\t@omega/toolchain\tlanguage\\core\\reflect\\{member}\t{state}"
        lines = [record(member, state) for member, state in (("field", "absent"), ("callable", "Machine"), ("FieldInfo", "Data"), ("CallableInfo", "Data"))]
        errors = []
        exports = reflection_exports(lines, errors)["members"]
        self.assertEqual((exports[r"language\core\reflect\field"], exports[r"language\core\reflect\callable"]), ([], ["Machine"]))
        self.assertEqual(errors, [])
        reflection_exports(lines[1:] + [lines[1], lines[0].replace("@omega/toolchain", "other")], errors)
        self.assertEqual(len(errors), 3)
