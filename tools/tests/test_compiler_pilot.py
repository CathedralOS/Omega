"""Refusal controls for the CI adapter; compiler behavior stays in the corpus."""

from copy import deepcopy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ci"))
import compiler_pilot as pilot
import compiler_cleanup as cleanup


class CompilerPilotTests(unittest.TestCase):
    def report(self, both=False):
        fixtures = pilot.POSITIVES if both else pilot.POSITIVES + pilot.NEGATIVES
        return dict(schema="omega-corpus-verdict-v1", revision="a" * 40, dirty=False,
                    exit_code=0, oracle_failures=[], infrastructure_error=None,
                    leg="both" if both else "checked", target="linux_x86_64",
                    observations=[dict(fixture=name,
                                       status="rejected" if name.startswith("fail/") else "built",
                                       expected_satisfied=True if name.startswith("fail/") else None,
                                       facts={"exit": "70", "interpreter-exit": "70", "interpreter": "agrees"})
                                  for name in fixtures])

    def validate(self, report, both=False):
        pilot.validate_report(report, pilot.POSITIVES if both else pilot.POSITIVES + pilot.NEGATIVES,
                              both=both, revision="a" * 40, target="linux_x86_64")

    def test_complete_runtime_and_rejections_pass(self):
        self.validate(self.report())
        self.validate(self.report(True), True)

    def test_golden_movement_is_not_a_failed_oracle(self):
        report = self.report()
        report["exit_code"] = 1
        self.validate(report)

    def test_wrong_source_or_failed_gate_refuses(self):
        for key, value in (("schema", "other"), ("revision", "b" * 40), ("dirty", True),
                           ("exit_code", 3), ("exit_code", 4), ("oracle_failures", ["wrong exit"]),
                           ("infrastructure_error", "missing compiler")):
            report = self.report()
            report[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                self.validate(report)

    def test_missing_duplicate_or_extra_observation_refuses(self):
        for operation in (lambda rows: rows.pop(), lambda rows: rows.append(deepcopy(rows[0])),
                          lambda rows: rows[0].update(fixture="pass/unrelated")):
            report = self.report()
            operation(report["observations"])
            with self.assertRaises(ValueError):
                self.validate(report)

    def test_wrong_reason_or_accepted_invalid_source_refuses(self):
        for key, value in (("expected_satisfied", False), ("expected_satisfied", None),
                           ("status", "checked")):
            report = self.report()
            report["observations"][-1][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.validate(report)

    def test_no_runtime_or_disagreement_refuses(self):
        for facts in ({}, {"exit": "70"},
                      {"exit": "70", "interpreter-exit": "71", "interpreter": "agrees"},
                      {"exit": "70", "interpreter-exit": "70", "interpreter": "declined"}):
            report = self.report(True)
            report["observations"][0]["facts"] = facts
            with self.subTest(facts=facts), self.assertRaises(ValueError):
                self.validate(report, True)

    def test_wrong_engine_or_target_refuses(self):
        for key, value in (("leg", "native"), ("target", "macos_arm64")):
            report = self.report(True)
            report[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.validate(report, True)

    def test_cleanup_report_requires_both_authored_output_observations(self):
        fixture = "pass/drops/runtime_projected_nominal_cleanup_exit"
        report = self.report(True)
        report["observations"] = [dict(fixture=fixture, status="built", facts={
            "exit": "0", "interpreter-exit": "0", "interpreter": "agrees",
            "stdout": "match", "interpreter-stdout": "match"})]

        def validate(value):
            pilot.validate_report(value, (fixture,), both=True,
                                  revision="a" * 40, target="linux_x86_64")

        validate(report)
        for key in ("stdout", "interpreter-stdout"):
            for value in (None, "mismatch"):
                missing = deepcopy(report)
                missing["observations"][0]["facts"][key] = value
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    validate(missing)

    def test_cleanup_cannot_lose_a_transport_or_intended_rejection(self):
        report = self.report()
        report["observations"] = [dict(
            fixture=fixture, status="checked" if fixture.startswith("pass/") else "rejected",
            expected_satisfied=fixture.startswith("fail/"))
            for fixture in cleanup.POSITIVES + cleanup.NEGATIVES]

        def validate(value):
            pilot.validate_report(value, cleanup.POSITIVES + cleanup.NEGATIVES,
                                  both=False, revision="a" * 40, target="linux_x86_64")

        validate(report)
        for fixture in (cleanup.POSITIVES[-1], cleanup.NEGATIVES[-1]):
            missing = deepcopy(report)
            missing["observations"] = [row for row in missing["observations"]
                                       if row["fixture"] != fixture]
            with self.subTest(fixture=fixture), self.assertRaises(ValueError):
                validate(missing)


if __name__ == "__main__":
    unittest.main()
