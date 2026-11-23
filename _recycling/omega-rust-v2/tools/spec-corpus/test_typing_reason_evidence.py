"""Replay the normal L3 collector/site join, with no derived identities or mocked observations."""
import os
from collections import defaultdict
from pathlib import Path
import unittest

from check_layer_readiness import collect, readiness_exit
import handoff_evidence
import rule_diagnostics
import typing_reason_evidence as reasons
import typing_sites


def inspect(stage_log, cli_log, sites):
    report = {"measurement_errors": [], "cases": {}, "suites": {}, "expected": {}, "obligations": {}, "package_fixtures": []}
    for suite, log in (("L3", stage_log), ("CLI", cli_log)):
        tests, _ = collect(log, suite, report)
        report["suites"][suite] = {"tests": tests, "exit": 0}
    # Support a dedicated typing_cases run and the combined CLI suite.
    targets = {row["target"].replace("\\", "/") for row in report["suites"]["CLI"]["tests"]}
    stages = [s for s, binary in rule_diagnostics.BINARIES.items() if f"tests/{binary}.rs" in targets]
    registered = {k: v for k, v in rule_diagnostics.registrations().items() if k[0] in stages}
    report["handoff_rules"] = {s: handoff_evidence.document_rules(s, (Path("layers") / handoff_evidence.DOCUMENTS[s][0]).read_text(encoding="utf-8"))
                               for s in stages}
    report["rule_diagnostics"] = rule_diagnostics.measure(report, cli_log, registered)
    typing_sites.verify(typing_sites.parse(sites), report)
    return report


class TypingReasonEvidence(unittest.TestCase):
    def test_only_the_named_passing_inventory_can_supply_the_denominator(self):
        target, test = reasons.EMITTER
        keys = list(dict.fromkeys(reasons.typing_reasons().values()))
        def inventory(keys):
            return "\t".join([reasons.PREFIX, str(len(keys)), *(field for key in keys for field in (key, "text"))])
        roster = inventory(keys)
        for suite, owner, status, records in [
            ("CLI", test, "ok", [roster]), ("L3", "other", "ok", [roster]), ("L3", test, "FAILED", [roster]),
            ("L3", test, "ok", [roster, roster]), ("L3", test, "ok", []), ("L3", test, "ok", [inventory(keys[:-1])]),
            ("L3", test, "ok", [inventory(keys + keys[:1])]), ("L3", test, "ok", [roster.rsplit("\t", 1)[0]]),
        ]:
            with self.subTest(suite=suite, owner=owner, status=status, records=records):
                report = {"measurement_errors": [], "suites": {"L3": {"tests": [{"target": target, "test": owner, "status": status}]}}}
                for line in records:
                    reasons.record(report, suite, target, (target, owner), line)
                self.assertFalse(reasons.measure(report, {"rows": [], "errors": []})["measured"])

    def test_an_uncovered_declared_reason_blocks_readiness_with_an_intact_measurement(self):
        target, test = reasons.EMITTER
        report = {"cases": {}, "measurement_errors": [], "suites": {"L3": {"tests": [
            {"target": target, "test": test, "status": "ok"}], "exit": 0}},
            "typing_reason_roster": dict.fromkeys(reasons.typing_reasons().values(), "reason")}
        inventory = {"rows": [], "errors": []}
        result = reasons.measure(report, inventory)
        self.assertTrue(result["measured"])
        self.assertFalse(inventory["errors"])
        report["typing_sites"] = {"typing_reasons": result}
        self.assertEqual(readiness_exit(report), 1)

    def test_a_genuine_typed_return_from_another_test_is_not_a_package_witness(self):
        target, test = reasons.EMITTER
        report = {"typing_reason_roster": dict.fromkeys(reasons.typing_reasons().values(), "same text"),
                  "suites": {"L3": {"tests": [{"target": target, "test": test, "status": "ok"}]}}}
        site = {"constructor": "for_reason"}
        inventory = {"errors": [], "rows": [site]}
        identity = reasons.typing_reasons()["crashes_none_rejects_a_route_guard"]
        observation = {"reason": "same text", "typing_reason": identity}
        reasons.attach(inventory, site, observation, "unrelated", "stage")
        reasons.attach(inventory, site, observation, "direct_call_pause::missing_block", "package")
        result = reasons.measure(report, inventory)
        self.assertEqual(result["families"]["crash-declaration"]["GuardOnNone"],
                         {"reason": "same text", "package": [], "stage": ["unrelated"]})
        self.assertTrue(any("unexpected typing reason" in error for error in inventory["errors"]))

    @unittest.skipUnless(os.environ.get("OMEGA_FAULT_EVIDENCE"), "set OMEGA_FAULT_EVIDENCE to replay real Rust logs")
    def test_faults_against_returned_package_and_stage_diagnostics(self):
        directory = Path(os.environ["OMEGA_FAULT_EVIDENCE"])
        stage, cli, sites = [(directory / name).read_text(encoding="utf-8") for name in ("L3.log", "frontend-cli.log", "typing-sites.tsv")]
        original = inspect(stage, cli, sites)
        self.assertEqual(original["measurement_errors"], [])
        families = original["typing_sites"]["typing_reasons"]["families"]
        by_reason = defaultdict(list)
        for test, identity in reasons.typing_reasons().items():
            by_reason[identity].append(test)
        for identity, tests in by_reason.items():
            family, name = identity.split(":")
            self.assertCountEqual(families[family][name]["package"], tests)
        for test, other in [("direct_call_pause::missing_suspend", "crashes_none_rejects_a_route_guard"),
                            ("crashes_none_rejects_a_route_guard", "direct_call_pause::missing_suspend")]:
            key = reasons.typing_reasons()[test]
            family, name = key.split(":")
            tag = "\t" + key
            record, = [line for line in cli.splitlines() if line.startswith("OMEGA_RULE_DIAGNOSTIC\tL3\t" + test + "\t")]
            unrelated, = [line for line in cli.splitlines() if line.startswith("OMEGA_RULE_DIAGNOSTIC\tL3\t" + other + "\t")]
            for label, replacement in [("same text without identity", record.replace(tag, "")),
                                       ("wrong family, original text", record.replace(key, reasons.typing_reasons()[other])),
                                       ("internally consistent refusal from another twin", unrelated.replace("\t" + other + "\t", "\t" + test + "\t"))]:
                with self.subTest(key=key, fault=label):
                    changed = inspect(stage, cli.replace(record, replacement), sites)
                    self.assertTrue(changed["rule_diagnostics"]["L3"][test]["verified"], "text record still present")
                    self.assertEqual(changed["typing_sites"]["typing_reasons"]["families"],
                                     {**families, family: {**families[family], name: {**families[family][name], "package": []}}})
                    self.assertTrue(reasons.blocks(changed["typing_sites"]["typing_reasons"]))
                    self.assertEqual(readiness_exit(changed), 1)
        # Strip one named twin's identity. Matching CLI text plus a genuine package
        # witness for the very same cause must not cover the damaged registration.
        for test in ("a_logical_parameter_cannot_gain_write_authority_at_specialization",
                     "cleanup_cannot_block", "cleanup_cannot_publish_an_abort_route"):
            with self.subTest(missing_identity=test):
                identity = reasons.typing_reasons()[test]
                record, = [line for line in cli.splitlines() if line.startswith("OMEGA_RULE_DIAGNOSTIC\tL3\t" + test + "\t")]
                tag = "\t" + identity + "\t"
                self.assertEqual(record.count(tag), 1)
                changed = inspect(stage, cli.replace(record, record.replace(tag, "\t")), sites)
                result = changed["typing_sites"]["typing_reasons"]
                self.assertTrue(changed["rule_diagnostics"]["L3"][test]["verified"])
                self.assertTrue(all(cause["package"] for family in result["families"].values() for cause in family.values()))
                self.assertEqual(result["missing_package_tests"], {test: identity})
                self.assertTrue(reasons.blocks(result))
                self.assertEqual(readiness_exit(changed), 1)
        roster, = [line for line in stage.splitlines() if line.startswith(reasons.PREFIX)]
        changed = inspect(stage.replace(roster, ""), cli, sites)
        self.assertFalse(changed["typing_sites"]["typing_reasons"]["measured"])
        self.assertEqual(readiness_exit(changed), 1)
