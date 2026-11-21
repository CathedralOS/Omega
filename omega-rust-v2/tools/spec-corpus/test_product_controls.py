"""The readiness run credits a Psi corruption control only from its own passing test's report line."""
import unittest

from check_layer_readiness import collect
import product_controls

DISPATCH = "runtime-dispatch/swapped-arms"


def log(skip=(), status="ok"):
    """A native L5 log of the control target in which every registered control reports its rejection."""
    tests = dict.fromkeys(product_controls.CONTROLS.values())
    lines = ["     Running tests\\negative_controls.rs (target\\debug\\negative_controls.exe)", "",
             f"running {len(tests)} tests"]
    lines += [f"test {test} ... {status if test == product_controls.CONTROLS[DISPATCH] else 'ok'}" for test in tests]
    lines += ["", "successes:", ""]
    for test in tests:
        lines.append(f"---- {test} stdout ----")
        lines += [f"OMEGA_NEGATIVE_CONTROL\t{label}\trejected\tthe witness disagreed with {label}"
                  for label, owner in product_controls.CONTROLS.items() if owner == test and label not in skip]
        lines.append("")
    failed = int(status == "FAILED")
    lines.append(f"test result: {'FAILED' if failed else 'ok'}. {len(tests) - failed} passed; {failed} failed; "
                 "0 ignored; 0 measured; 0 filtered out")
    return "\n".join(lines) + "\n"


def measure(output):
    report = {"suites": {}, "cases": {}, "expected": {}, "cohort_sources": {}, "obligations": {},
              "package_fixtures": [], "measurement_errors": [], "last_layer": "L5"}
    tests, _ = collect(output, "L5", report)
    report["suites"]["L5"] = {"tests": tests}
    product_controls.finish(report)
    return report["product_controls"], report["measurement_errors"]


class ProductControls(unittest.TestCase):
    def test_runtime_dispatch_control_is_registered_to_its_test(self):
        self.assertEqual(product_controls.CONTROLS[DISPATCH], "swapped_dispatch_arms_fail_the_arriving_descriptor_witness")

    def test_every_reported_rejection_in_a_passing_test_is_verified(self):
        controls, errors = measure(log())
        self.assertEqual(errors, [])
        self.assertEqual(set(controls), set(product_controls.CONTROLS))
        self.assertTrue(all(row["verified"] for row in controls.values()))
        self.assertEqual(controls[DISPATCH]["rejection"], f"the witness disagreed with {DISPATCH}")

    def test_a_missing_report_line_is_a_measurement_error_while_the_test_stays_green(self):
        controls, errors = measure(log(skip={DISPATCH}))
        self.assertFalse(controls[DISPATCH]["verified"])
        self.assertTrue(all(row["verified"] for label, row in controls.items() if label != DISPATCH))
        self.assertEqual(errors, [f"missing product corruption check: {DISPATCH} · "
                                  f"{product_controls.TARGET}::{product_controls.CONTROLS[DISPATCH]}"])

    def test_a_failed_control_is_not_verified_and_is_left_to_its_test_failure(self):
        controls, errors = measure(log(skip={DISPATCH}, status="FAILED"))
        self.assertFalse(controls[DISPATCH]["verified"])
        self.assertEqual(errors, [])

    def test_a_report_line_from_another_test_is_rejected(self):
        output = log().replace(f"OMEGA_NEGATIVE_CONTROL\t{DISPATCH}\t", "OMEGA_NEGATIVE_CONTROL\tbranch-target/swapped\t")
        controls, errors = measure(output)
        self.assertFalse(controls[DISPATCH]["verified"])
        self.assertIn(f"missing product corruption check: {DISPATCH} · "
                      f"{product_controls.TARGET}::{product_controls.CONTROLS[DISPATCH]}", errors)
        self.assertTrue(any(error.startswith("unexpected product corruption observation:") for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
