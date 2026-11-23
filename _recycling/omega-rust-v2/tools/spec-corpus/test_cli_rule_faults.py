"""Replay a recorded package CLI log after deleting or moving one contract refusal per front-end layer.

Set OMEGA_FAULT_EVIDENCE to a readiness report directory. The replay reads that run's
frontend-cli.log, the versioned diagnostic_rules.tsv registry and the layer contracts in
this checkout. It never reads the report's own rule_diagnostics results.
"""
from collections import Counter
import os
from pathlib import Path
import unittest
from urllib.parse import unquote

from check_layer_readiness import collect
import handoff_evidence
import preparation_reason_evidence
import resolver_kind_evidence
import rule_diagnostics
import typing_sites

ROOT = Path(__file__).absolute().parent.parent.parent
MARKER = "OMEGA_RULE_DIAGNOSTIC\t"
STAGES = tuple(rule_diagnostics.BINARIES)


def contract_rules():
    return {stage: handoff_evidence.document_rules(stage, (ROOT / "layers" / handoff_evidence.DOCUMENTS[stage][0]).read_text(encoding="utf-8"))
            for stage in STAGES}


def inspect(log, rules, registered):
    report = {"suites": {}, "cases": {}, "expected": {}, "cohort_sources": {}, "obligations": {}, "package_fixtures": [],
              "measurement_errors": [], "last_layer": "L4", "handoff_rules": rules}
    tests, _ = collect(log, "CLI", report)
    report["suites"]["CLI"] = {"tests": tests}
    results = rule_diagnostics.measure(report, log, registered)
    return tests, results, report["measurement_errors"]


def credit(results):
    return {(stage, test): row["verified"] for stage, rows in results.items() for test, row in rows.items()}


@unittest.skipUnless(os.environ.get("OMEGA_FAULT_EVIDENCE"), "set OMEGA_FAULT_EVIDENCE to replay a recorded readiness run")
class RecordedCliRuleFaults(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        directory = Path(os.environ["OMEGA_FAULT_EVIDENCE"])
        cls.log = (directory / "frontend-cli.log").read_text(encoding="utf-8", errors="replace")
        cls.lines = cls.log.splitlines()
        cls.registered = rule_diagnostics.registrations()
        cls.rules = contract_rules()
        cls.tests, cls.results, cls.errors = inspect(cls.log, cls.rules, cls.registered)
        cls.credit = credit(cls.results)

    def marker(self, key):
        """Index of the single recorded marker for a registered (stage, test)."""
        found = [i for i, line in enumerate(self.lines) if line.startswith(MARKER) and tuple(line.split("\t")[1:3]) == key]
        self.assertEqual(len(found), 1, key)
        return found[0]

    def verified(self, stage):
        keys = [key for key in self.registered if key[0] == stage and self.credit.get(key)]
        if not keys:
            self.fail(f"{stage}: the recorded run has no verified registered refusal to remove")
        return keys

    def target(self, stage):
        return f"tests/{rule_diagnostics.BINARIES[stage]}.rs"

    def replay(self, lines, lost, misplaced=()):
        """Removing a witness affects its case. A foreign marker also invalidates the host test's records."""
        tests, results, errors = inspect("\n".join(lines), self.rules, self.registered)
        self.assertEqual(tests, self.tests, "a marker mutation must not change any test status")
        affected = {lost, *(host for _, host in misplaced)}
        self.assertEqual(credit(results), {**self.credit, **dict.fromkeys(affected, False)})
        for (stage, test), verified in self.credit.items():
            if verified and (stage, test) not in affected:
                self.assertEqual(results[stage][test]["observation"], self.results[stage][test]["observation"])
        self.assertEqual(Counter(self.errors) - Counter(errors), Counter(), "a baseline error disappeared")
        added = list((Counter(errors) - Counter(self.errors)).elements())
        for stage, test in affected:
            missing = f"{stage}::{test}: missing verified contract diagnostic (test ok)"
            self.assertEqual(added.count(missing), 1, added)
            added.remove(missing)
        self.assertEqual(len(added), len(misplaced), added)
        for line, (stage, host) in misplaced:
            prefix = f"invalid contract diagnostic in {self.target(stage)}::{host}: "
            self.assertEqual(sum(error.startswith(prefix) and error.endswith(": " + line) for error in added), 1, added)

    def test_recorded_run_credits_registered_refusals(self):
        self.assertEqual(set(self.credit), set(self.registered))
        for stage in STAGES:
            for key in self.verified(stage):
                with self.subTest(key=key):
                    line = self.lines[self.marker(key)]
                    self.assertEqual(self.results[stage][key[1]]["test"], "ok")
                    self.assertEqual(self.results[stage][key[1]]["observation"]["reason"], unquote(line.rsplit("\t", 1)[1]))

    def test_losing_one_rust_origin_does_not_borrow_an_identical_typing_message(self):
        names = ["domain_membership_checks_the_declared_carrier", "case_membership_checks_the_owning_sum"]
        sites = (Path(os.environ["OMEGA_FAULT_EVIDENCE"]) / "typing-sites.tsv").read_text(encoding="utf-8")
        def measured(lines):
            _, results, errors = inspect("\n".join(lines), self.rules, self.registered)
            self.assertEqual(errors, [])
            inventory = typing_sites.parse(sites)
            typing_sites.attach_witnesses(inventory, {"rule_diagnostics": results})
            origins = {test: (row["path"], row["line"], row["column"]) for row in inventory["rows"]
                       for test in row.get("package_tests", ()) if test in names}
            return results["L3"], origins, inventory["errors"]

        results, origins, errors = measured(self.lines)
        self.assertEqual(errors, [])
        self.assertEqual(set(origins), set(names))
        self.assertEqual(results[names[0]]["observation"]["reason"], results[names[1]]["observation"]["reason"])
        changed = list(self.lines)
        index = self.marker(("L3", names[0]))
        fields = changed[index].split("\t")
        fields[-3] = "999999"
        changed[index] = "\t".join(fields)
        results, after, errors = measured(changed)
        self.assertTrue(results[names[0]]["verified"])
        self.assertEqual(after, {names[1]: origins[names[1]]})
        self.assertEqual(len(errors), 1)
        self.assertIn(names[0], errors[0])

    def test_deleting_a_typed_refusal_leaves_the_cli_text_verified_but_removes_its_kind_witness(self):
        directory = Path(os.environ["OMEGA_FAULT_EVIDENCE"])
        layer_logs = {stage: (directory / f"{stage}.log").read_text(encoding="utf-8")
                      for stage in ("L2", "L3")}

        def typed_witnesses(cli_log):
            report = {"measurement_errors": [], "cases": {}, "expected": {}, "obligations": {},
                      "package_fixtures": [], "suites": {}, "rule_diagnostics": self.results}
            for stage, log in {**layer_logs, "CLI": cli_log}.items():
                tests, _ = collect(log, stage, report)
                report["suites"][stage] = {"tests": tests}
            return resolver_kind_evidence.measure(report), report["measurement_errors"]

        baseline, errors = typed_witnesses(self.log)
        self.assertEqual(errors, [])
        kind, witness = next((kind, row) for kind, rows in baseline.items() for row in rows if row["suite"] == "CLI")
        begin = self.lines.index(f"---- {witness['test']} stdout ----")
        marker = next(i for i in range(begin + 1, len(self.lines)) if self.lines[i].startswith("OMEGA_RESOLVER_KIND\t"))
        changed = "\n".join(self.lines[:marker] + self.lines[marker + 1:])
        self.assertEqual(credit(inspect(changed, self.rules, self.registered)[1]), self.credit)
        observed, errors = typed_witnesses(changed)
        self.assertEqual(errors, [f"L2::{witness['test']}: missing typed resolver diagnostic for the verified CLI refusal"])
        self.assertEqual(observed, {**baseline, kind: [row for row in baseline[kind] if row != witness]})

    def test_removing_one_refusal_per_layer_loses_only_that_credit(self):
        for stage in STAGES:
            keys = self.verified(stage)
            # At L4, prefer a refusal whose reason continues on a second line.
            multiline = [key for key in keys if "\n" in self.results[stage][key[1]]["observation"]["reason"]]
            victim = (multiline or keys)[0]
            with self.subTest(stage=stage, test=victim[1]):
                index = self.marker(victim)
                self.replay(self.lines[:index] + self.lines[index + 1:], victim)

    def test_multiline_reason_is_one_encoded_record(self):
        keys = [key for stage in STAGES for key in self.verified(stage)
                if "\n" in self.results[key[0]][key[1]]["observation"]["reason"]]
        if not keys:
            self.skipTest("the recorded run credited no multiline reason")
        for key in keys:
            with self.subTest(key=key):
                line = self.lines[self.marker(key)]
                reason = self.results[key[0]][key[1]]["observation"]["reason"]
                self.assertIn("%0A", line.rsplit("\t", 1)[1])
                # Each continuation stays inside the marker rather than on a separate log line.
                for continuation in reason.splitlines()[1:]:
                    self.assertNotIn(continuation, self.lines)

    def test_marker_printed_by_another_suite_loses_credit(self):
        for stage, host_stage in zip(STAGES, STAGES[1:] + STAGES[:1]):
            victim, host = self.verified(stage)[0], self.verified(host_stage)[0]
            with self.subTest(stage=stage, host=host):
                lines = list(self.lines)
                moved = lines.pop(self.marker(victim))
                lines.insert(lines.index(self.lines[self.marker(host)]), moved)
                self.replay(lines, victim, [(moved, host)])

    def test_marker_borrowed_from_a_sibling_loses_credit(self):
        for stage in STAGES:
            keys = self.verified(stage)
            if len(keys) < 2:
                continue
            victim = keys[0]
            # A sibling under the same contract entry is the hardest borrow to tell apart.
            same_rule = [key for key in keys[1:] if self.registered[key] == self.registered[victim]]
            sibling = (same_rule or keys[1:])[0]
            with self.subTest(stage=stage, test=victim[1], sibling=sibling[1]):
                lines = list(self.lines)
                borrowed = self.lines[self.marker(sibling)]
                lines[self.marker(victim)] = borrowed
                self.replay(lines, victim, [(borrowed, victim)])


    def preparation_evidence(self, cli):
        layer_log = (Path(os.environ["OMEGA_FAULT_EVIDENCE"]) / "L4.log").read_text(encoding="utf-8")
        report = {"measurement_errors": [], "cases": {}, "expected": {}, "obligations": {},
                  "package_fixtures": [], "suites": {}, "rule_diagnostics": self.results}
        for suite, log in (("L4", layer_log), ("CLI", cli)):
            tests, _ = collect(log, suite, report)
            report["suites"][suite] = {"tests": tests}
        result = preparation_reason_evidence.measure(report)
        return result, report["measurement_errors"]

    def test_removing_a_real_preparation_cause_cannot_borrow_another_calls_cause(self):
        case = "discarded_operand_still_refuses_its_failed_precondition"
        measured, errors = self.preparation_evidence(self.log)
        row = measured["reasons"]["IntegerPreconditionFalse"]
        self.assertFalse(errors, errors)
        self.assertIn(case, [site["test"] for site in row["witnesses"]])
        index = self.lines.index(f"---- {case} stdout ----")
        records = list(self.lines)
        while not records[index].startswith("OMEGA_PREPARATION_REASON\t"):
            index += 1
            self.assertNotEqual(records[index], "")
        del records[index]
        measured, errors = self.preparation_evidence("\n".join(records))
        row = measured["reasons"]["IntegerPreconditionFalse"]
        self.assertEqual(row["state"], "verified")
        self.assertNotIn(case, [site["test"] for site in row["witnesses"]])
        self.assertIn(f"CLI::{case}: missing typed preparation cause for the verified package refusal", errors)


    def test_a_forwarded_cause_must_preserve_its_owner_payload_and_source(self):
        test = "a_forwarded_integer_refusal_identifies_the_nested_call_in_its_defining_file"
        baseline, errors = self.preparation_evidence(self.log)
        self.assertFalse(errors, errors)
        self.assertFalse(baseline["missing_package_tests"])
        start = self.lines.index(f"---- {test} stdout ----") + 1
        record = next(line for line in self.lines[start:] if line.startswith("OMEGA_PREPARATION_FORWARDED\t"))
        tag, owner, original, package, path, first, last, length, text = record.split("\t")
        self.assertEqual((owner, path), ("IntegerPrimitive", "helpers.omg"))
        self.assertIn("%0Acall chain: via -> shift", text)
        self.assertEqual(self.log.count(record), 1)
        for fault, replacement in {
            "missing": "",
            "other owner": record.replace("\tIntegerPrimitive\t", "\tTargetLayout\t", 1),
            "different original payload": record.replace(original, "unrelated primitive error", 1),
            "caller file instead of failing invocation": record.replace("\thelpers.omg\t", "\tuse.omg\t", 1),
            "chain removed": "\t".join([tag, owner, original, package, path, first, last,
                                         str(len(text.split("%0A")[0].encode())), text.split("%0A")[0]]),
        }.items():
            with self.subTest(fault=fault):
                measured, errors = self.preparation_evidence(self.log.replace(record, replacement))
                self.assertTrue(errors)
                self.assertEqual(measured["missing_package_tests"], [test])
                self.assertTrue(preparation_reason_evidence.blocks(measured))
                self.assertTrue(any(r["suite"] == "CLI" and r["cause"] == owner for r in measured["forwarded"]))
                rule = {"rule": "Every refusal L4 renders carries its typed cause.", "consumer": []}
                handoff_evidence.preparation_diagnostic_consumers({"preparation_reasons": measured, "handoff_rules": {"L4": [rule]}})
                self.assertTrue(rule["consumer"])
                self.assertFalse(any(test in credit for credit in rule["consumer"]))


if __name__ == "__main__":
    unittest.main()
