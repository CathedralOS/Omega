"""A passing test name alone cannot verify a resolver diagnostic kind."""
import unittest

from check_layer_readiness import collect, readiness_exit
from resolver_kind_evidence import INVENTORY_TEST, measure
from test_l5_records import libtest


class ResolverKindTests(unittest.TestCase):
    kind = "OMEGA_RESOLVER_KIND\tUnresolvedName\tUnresolvedName\troot\tcase.omg\t2\t3\t14\tunknown name x"
    roster = "OMEGA_RESOLVER_INVENTORY\tUnresolvedName=Source,ReservedPackageIdentity=StageInput"

    def inspect(self, lines, roster=None, test="missing_name", status="ok", location=(2, 3)):
        report = {"expected": {}, "cases": {}, "measurement_errors": [], "suites": {}, "package_fixtures": [], "obligations": {}}
        for stage, log in [
            ("L2", libtest("tests/stage.rs", (INVENTORY_TEST, "ok", [roster or self.roster]))),
            ("CLI", libtest("tests/resolver_cases.rs", (test, status, lines))),
        ]:
            tests, _ = collect(log, stage, report)
            report["suites"][stage] = {"tests": tests, "exit": int(status == "FAILED")}
        report["rule_diagnostics"] = {"L2": {"missing_name": {"verified": True, "observation": {
            "file": "case.omg", "byte_start": location[0], "byte_end": location[1], "reason": "unknown name x"}}}}
        report["resolver_diagnostic_kinds"] = measure(report)
        return report

    def test_the_compiled_inventory_includes_kinds_without_a_registered_test(self):
        report = self.inspect([self.kind])
        self.assertEqual(report["measurement_errors"], [])
        kinds = report["resolver_diagnostic_kinds"]
        self.assertEqual(len(kinds["UnresolvedName"]), 1)
        self.assertEqual(kinds["ReservedPackageIdentity"], [])
        self.assertEqual(readiness_exit(report), 1)

    def test_wrong_emitter_origin_reason_and_failed_test_cannot_verify_the_kind(self):
        faults = [([], {}), ([self.kind], {"test": "other"}), ([self.kind], {"status": "FAILED"}),
                  ([self.kind], {"location": (0, 1)}), ([self.kind.replace("root", "dep")], {}),
                  ([self.kind.replace("unknown name x", "unknown name y")], {}),
                  ([self.kind.replace("14\t", "12\t")], {})]
        for lines, arguments in faults:
            with self.subTest(lines=lines, arguments=arguments):
                report = self.inspect(lines, **arguments)
                self.assertTrue(report["measurement_errors"])
                self.assertEqual(report["resolver_diagnostic_kinds"]["UnresolvedName"], [])

    def test_invalid_or_repeated_inventory_entries_do_not_supply_a_denominator(self):
        for roster in [self.roster + ",UnresolvedName=Source", self.roster + ","]:
            report = self.inspect([self.kind], roster=roster)
            self.assertTrue(report["measurement_errors"])
            self.assertEqual(report["resolver_diagnostic_kinds"], {})

    def test_a_valid_witness_cannot_mask_corruption_in_the_same_test(self):
        for corrupted in [self.kind.replace("14\t", "15\t"), self.kind.replace("UnresolvedName", "UnregisteredKind")]:
            with self.subTest(corrupted=corrupted):
                report = self.inspect([self.kind, corrupted])
                self.assertTrue(report["measurement_errors"])
                self.assertEqual(report["resolver_diagnostic_kinds"]["UnresolvedName"], [])


if __name__ == "__main__":
    unittest.main()
