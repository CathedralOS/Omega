"""Refusal controls for the CI adapter; compiler behavior stays in the corpus."""

from copy import deepcopy
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ci"))
import compiler_pilot as pilot
import compiler_cleanup as cleanup
import compiler_retained_foreign as retained


class CompilerPilotTests(unittest.TestCase):
    def fixture_root(self, root, *fixtures):
        for fixture in fixtures:
            directory = root / "tests" / "omega" / fixture
            directory.mkdir(parents=True, exist_ok=True)
            (directory / "main.omg").write_text("machine main() {}", encoding="utf-8")

    def test_exact_fixture_ids_split_runtime_and_rejection_rosters(self):
        positive = "pass/float/named_conversion"
        negative = "fail/float/unproved_conversion"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, positive, negative)
            with patch.object(pilot, "ROOT", root):
                self.assertEqual(pilot.select_fixtures(f" {negative}, {positive} "),
                                 ((positive,), (negative,)))

    def test_malformed_or_missing_exact_id_never_builds_or_runs_defaults(self):
        fixture = "pass/float/named_conversion"
        invalid = ("", " ", ",", fixture + ",", fixture + "," + fixture,
                   "float", "run/float/named_conversion", "/" + fixture,
                   "pass/float/../named_conversion", "pass//float/named_conversion",
                   "pass/float/./named_conversion", "pass\\float\\named_conversion",
                   "tests/omega/" + fixture, "pass/float/missing")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, fixture)
            with patch.object(pilot, "ROOT", root), \
                    patch.object(pilot, "execute") as execute, \
                    patch.object(pilot, "prepare_producer") as build:
                for request in invalid:
                    with self.subTest(request=request):
                        self.assertEqual(pilot.run_suite(
                            "refused", pilot.POSITIVES, pilot.NEGATIVES, fixtures=request), 1)
                        summary = json.loads((root / "build/refused/summary.json").read_text())
                        self.assertFalse(summary["passed"])
                        self.assertTrue(summary["error"])
                execute.assert_not_called()
                build.assert_not_called()

    def test_exact_selector_refuses_substring_collision_before_build(self):
        fixture = "pass/float/named_conversion"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, fixture, fixture + "_extra")
            with patch.object(pilot, "ROOT", root), \
                    patch.object(pilot, "prepare_producer") as build:
                with self.assertRaisesRegex(ValueError, "unrequested fixtures"):
                    pilot.select_fixtures(fixture)
                self.assertEqual(pilot.select_fixtures(fixture + "," + fixture + "_extra"),
                                 ((fixture, fixture + "_extra"), ()))
                build.assert_not_called()

    def test_existing_directory_without_main_is_not_a_fixture(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "tests/omega/pass/float/not_a_fixture").mkdir(parents=True)
            with patch.object(pilot, "ROOT", root):
                with self.assertRaisesRegex(ValueError, "main.omg"):
                    pilot.select_fixtures("pass/float/not_a_fixture")

    def test_cli_and_environment_requests_reach_the_same_suite(self):
        with patch.dict(os.environ, {pilot.FIXTURE_ENVIRONMENT: "pass/float/from_environment"}), \
                patch.object(pilot, "run_suite", return_value=0) as suite:
            self.assertEqual(pilot.main([]), 0)
            self.assertEqual(suite.call_args.kwargs["fixtures"], "pass/float/from_environment")
            self.assertEqual(pilot.main(["--fixtures", "fail/float/from_cli"]), 0)
            self.assertEqual(suite.call_args.kwargs["fixtures"], "fail/float/from_cli")
            self.assertEqual(pilot.main(["--fixtures", ""]), 0)
            self.assertEqual(suite.call_args.kwargs["fixtures"], "")
        with patch.dict(os.environ, {pilot.FIXTURE_ENVIRONMENT: ""}), \
                patch.object(pilot, "run_suite", return_value=0) as suite:
            self.assertEqual(pilot.main([]), 0)
            self.assertEqual(suite.call_args.args, ("compiler-pilot", pilot.POSITIVES, pilot.NEGATIVES))
            self.assertIsNone(suite.call_args.kwargs["fixtures"])

    def test_nonempty_bad_environment_request_never_falls_back(self):
        with tempfile.TemporaryDirectory() as temporary:
            with patch.object(pilot, "ROOT", Path(temporary)), \
                    patch.dict(os.environ, {pilot.FIXTURE_ENVIRONMENT: "pass/float/missing,"}), \
                    patch.object(pilot, "execute") as execute:
                self.assertEqual(pilot.main([]), 1)
                execute.assert_not_called()

    def test_failed_retained_cohort_does_not_hide_later_measurements_or_turn_green(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            facts = (("shared", "shared", "shared", (), ()),
                     ("mutable", "mutable", "mutable", (), ()),
                     ("nested/arrays", "array_cases", "nested", (), ()))
            with patch.object(retained, "ROOT", root), \
                    patch.object(retained, "FACTS", facts), \
                    patch.dict(os.environ), \
                    patch.object(retained, "qualify_cohort", side_effect=[
                        ValueError("native timeout"), OSError("provider unavailable"),
                        root / "nested/omega"] ) as qualify, \
                    patch.object(retained, "unoffered_descriptor"), \
                    patch.object(pilot, "command", return_value="a" * 40), \
                    patch.object(pilot.corpus_gate, "host_target", return_value="windows_x86_64"):
                self.assertEqual(retained.main(), 1)
            self.assertEqual([call.args[2] for call in qualify.call_args_list],
                             ["shared", "mutable", "nested/arrays"])
            report = json.loads((root / "build/compiler-retained-foreign/summary.json").read_text())
            self.assertFalse(report["passed"])
            self.assertEqual([row["passed"] for row in report["observations"]], [False, False, True])
            self.assertEqual(report["observations"][0]["error"], "native timeout")
            self.assertEqual(report["observations"][1]["error"], "provider unavailable")

    def test_rejection_only_run_never_launches_an_empty_runtime_filter(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            producer = Mock(revision="a" * 40, target="linux_x86_64",
                            runner_profile={"sha256": "b" * 64})
            fixture = "fail/example/intended_rejection"
            def corpus(arguments, **kwargs):
                self.assertEqual(arguments[arguments.index("--filter") + 1], fixture)
                self.assertNotIn("--both", arguments)
                report = self.report()
                report["observations"] = [dict(fixture=fixture, status="rejected", expected_satisfied=True)]
                Path(arguments[arguments.index("--json") + 1]).write_text(json.dumps(report))
                return Mock(returncode=0)
            with patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                 patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}), \
                 patch.object(pilot.corpus_gate, "host_target", return_value="linux_x86_64"), \
                 patch.object(pilot, "observation_golden", return_value=output / "golden"), \
                 patch.object(pilot.subprocess, "run", side_effect=corpus) as execute, \
                 patch.object(pilot, "prepare_producer") as build:
                summary = {}
                pilot.execute(output, summary, positives=(), negatives=(fixture,), producer=producer)
                execute.assert_called_once()
                self.assertNotIn("native and Terminal execution", [stage["name"] for stage in summary["stages"]])
                with self.assertRaisesRegex(ValueError, "at least one fixture"):
                    pilot.execute(output, {}, positives=(), negatives=())
                build.assert_not_called()

    def test_first_host_keeps_unknown_history_separate_from_other_host_results(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            corpus = root / "tests/omega"
            corpus.mkdir(parents=True)
            other = corpus / "corpus_both_macos_arm64.txt"
            other.write_text("an existing host's measured rows\n", encoding="utf-8")
            output = root / "build"
            output.mkdir()
            with patch.object(pilot, "ROOT", root):
                baseline = pilot.observation_golden(output, both=True, target="linux_arm64")
                with self.assertRaisesRegex(ValueError, "published corpus baseline"):
                    pilot.observation_golden(output, both=False, target="linux_arm64")
            self.assertEqual(baseline.parent, output)
            self.assertTrue(all(line.startswith("#") for line in baseline.read_text().splitlines()))
            self.assertFalse((corpus / "corpus_both_linux_arm64.txt").exists())
            self.assertEqual(other.read_text(), "an existing host's measured rows\n")
            # Missing historical rows never permit a failed authored oracle.
            report = self.report(True)
            report["oracle_failures"] = ["expected native 0, observed timeout"]
            with self.assertRaises(ValueError):
                self.validate(report, True)

    def test_existing_matching_host_baseline_is_preserved(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            baseline = root / "tests/omega/corpus_both_linux_arm64.txt"
            baseline.parent.mkdir(parents=True)
            original = b"existing matching-host observations\n"
            baseline.write_bytes(original)
            with patch.object(pilot, "ROOT", root):
                self.assertEqual(pilot.observation_golden(
                    root / "absent-output", both=True, target="linux_arm64"), baseline)
            self.assertEqual(baseline.read_bytes(), original)
            self.assertFalse((root / "absent-output").exists())

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
                      {"exit": "70", "interpreter-exit": "70", "interpreter": "declined"},
                      {"exit": "signal 5", "interpreter-exit": "signal 5", "interpreter": "agrees"},
                      {"exit": "timeout", "interpreter-exit": "timeout", "interpreter": "agrees"},
                      {"exit": 70, "interpreter-exit": 70, "interpreter": "agrees"}):
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

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, fixture)
            (root / "tests/omega" / fixture / "expected_stdout.txt").write_text("cleanup\n")
            with patch.object(pilot, "ROOT", root):
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
