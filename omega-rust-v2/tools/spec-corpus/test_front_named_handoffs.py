"""Scoped log replays: leave the other witnesses and libtest names intact."""
import os
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import callable_alias_evidence as callable_alias
import handoff_evidence as handoffs
from check_layer_readiness import collect
from test_l5_records import libtest


def inspect(**logs):
    report = {"measurement_errors": [], "suites": {}, "cases": {}, "expected": {}, "obligations": {}, "package_fixtures": []}
    for stage, log in logs.items():
        tests, _ = collect(log, stage, report)
        report["suites"][stage] = {"tests": tests}
    Path("target").mkdir(exist_ok=True)
    with TemporaryDirectory(dir="target") as directory:
        handoffs.finish(report, Path(directory))
    items = {(row["rule"], item["name"]): item["verified"] for stage in logs
             for row in report["handoff_rules"][stage] for item in row.get("named_requirements", [])}
    return items, report


class NamedRequirements(unittest.TestCase):
    def test_a_failed_parser_fixture_cannot_satisfy_either_side_of_a_handoff(self):
        target, test, _ = handoffs.GOLDEN_TESTS["L1"]
        receipt = "OMEGA_GOLDEN_MATCH\tlayers/01_tokens-to-syntax-trees/tests/fixtures/operators/accept/right_shift.omg"
        for status in ("ok", "FAILED"):
            _, report = inspect(L1=libtest(f"tests/{target}.rs", (test, status, [receipt])))
            producer = next(r for r in report["handoff_rules"]["L1"] if r["rule"] == "right shift")
            consumer = next(r for r in report["handoff_rules"]["L0"] if r["rule"] == "R16")
            self.assertEqual("operators/accept/right_shift.omg" in producer["direct"], status == "ok")
            self.assertEqual(any("operators/accept/right_shift.omg" in item for item in consumer["consumer"]), status == "ok")

    def test_unknown_contract_reference_is_not_silently_dropped(self):
        for cell in ("`unclosed", "byte_cases::missing", "`other_target::test`", "`one`, `one`"):
            with self.subTest(cell=cell), self.assertRaises(ValueError):
                handoffs.document_rules("L0", f"## Rules\n| R18 | promise | design | — | {cell} |\n")

    def test_l0_named_pair_and_exact_passing_emitter_are_individually_required(self):
        _, _, test = handoffs.GOLDEN_TESTS["L0"]
        fixture = "numbers/reject/invalid_suffix_f16.omg"
        marker = "OMEGA_GOLDEN_MATCH\tlayers/00_bytes-to-tokens/tests/fixtures/" + fixture
        other = marker.replace("f16.omg", "usize.omg")
        is_file = Path.is_file
        for fault in (None, ".omg", ".expected", "missing marker", "duplicate", "wrong test", "FAILED"):
            with self.subTest(fault=fault):
                lines = ([] if fault == "missing marker" else [marker]) + [other]
                if fault == "duplicate":
                    lines.append(marker)
                log = libtest("tests/golden_fixtures.rs", ("wrong" if fault == "wrong test" else test,
                                                         "FAILED" if fault == "FAILED" else "ok", lines))
                with patch.object(Path, "is_file", lambda p: p.name != f"invalid_suffix_f16{fault}" and is_file(p)):
                    items, report = inspect(L0=log)
                self.assertEqual(items["R11", fixture], fault is None)
                self.assertEqual(items["R11", "numbers/reject/invalid_suffix_usize.omg"], fault not in {"wrong test", "FAILED"})
                row = next(r for r in report["handoff_rules"]["L0"] if r["rule"] == "R11")
                self.assertEqual(fixture in row.get("refusals", []), fault is None)
                self.assertEqual(any(f"unconfirmed named fixture: {fixture}" in e for e in report["measurement_errors"]), fault is not None)

    def test_reviewed_registration_cannot_recredit_a_missing_fixture_partner(self):
        fixture = "quoted_literals/reject/codepoint_escape.omg"  # R13 also registers this in reviewed_handoffs.tsv.
        test = handoffs.GOLDEN_TESTS["L0"][2]
        log = libtest("tests/golden_fixtures.rs", (test, "ok", ["OMEGA_GOLDEN_MATCH\tlayers/00_bytes-to-tokens/tests/fixtures/" + fixture]))
        is_file = Path.is_file
        with patch.object(Path, "is_file", lambda p: p.name != "codepoint_escape.expected" and is_file(p)):
            items, report = inspect(L0=log)
        self.assertFalse(items["R13", fixture])
        row = next(r for r in report["handoff_rules"]["L0"] if r["rule"] == "R13")
        self.assertFalse(row.get("refusals"))

    def test_named_byte_test_cannot_be_replaced_by_a_file_or_a_green_test_name(self):
        name = "form_feed_rejects"
        marker = 'OMEGA_REVIEWED_COMPARISON\twhitespace/form-feed\texpect_error(b"a\\x0Cb", LexErrorKind::UnknownCharacter, SourceSpan::new(1, 2))'
        for emitter, status, lines, verified in [(name, "ok", [marker], True), (name, "ok", [], False),
                                                  ("another_test", "ok", [marker], False), (name, "FAILED", [marker], False)]:
            with self.subTest(emitter=emitter, status=status, lines=lines):
                log = libtest("tests/byte_cases.rs", (emitter, status, lines))
                items, report = inspect(L0=log)
                self.assertEqual(items["R3", "byte_cases::" + name], verified)
                self.assertNotIn(("R3", "reject/" + name + ".omg"), items)
                self.assertEqual(any(f"unconfirmed named unit test: byte_cases::{name}" in e for e in report["measurement_errors"]), not verified)

    def test_nested_return_needs_the_inner_keyword_in_that_input_with_that_payload(self):
        nested = ("OMEGA_HANDOFF_ASSERT\tinline-callable/nested-return\tdata Factory = machine()->machine()->u8;"
                  "\tUnexpectedToken\t26\t33\t" + callable_alias.EXPECTED)
        bound = ("OMEGA_HANDOFF_ASSERT\tinline-callable/const-bound\tmachine filter<const Keep:machine(item:u8)->bool>() {}"
                 "\tUnexpectedToken\t26\t33\t" + callable_alias.EXPECTED)
        for fault, lines in [(None, [nested, bound]), ("missing", [bound]), ("duplicate", [nested, nested, bound]),
                             ("outer keyword", [nested.replace("26\t33", "15\t22"), bound]),
                             ("swapped IDs", [nested.replace("nested-return", "const-bound"), bound.replace("const-bound", "nested-return")]),
                             ("wrong kind", [nested.replace("UnexpectedToken", "UnexpectedEnd"), bound]),
                             ("wrong payload", [nested + "!", bound]), ("wrong emitter", [nested, bound]), ("FAILED", [nested, bound])]:
            with self.subTest(fault=fault):
                log = libtest(callable_alias.TARGET, ("other" if fault == "wrong emitter" else callable_alias.TEST,
                                                     "FAILED" if fault == "FAILED" else "ok", lines))
                items, report = inspect(L1=log)
                self.assertEqual(items["types", "inline-callable/nested-return"], fault is None)
                self.assertEqual(items["types", "inline-callable/const-bound"], fault not in {"swapped IDs", "wrong emitter", "FAILED"})
                self.assertEqual(any("unconfirmed named parse_file comparison: inline-callable/nested-return" in e
                                     for e in report["measurement_errors"]), fault is not None)

    @unittest.skipUnless(os.getenv("OMEGA_FRONT_HANDOFF_LOGS"), "set OMEGA_FRONT_HANDOFF_LOGS to replay the scoped native targets")
    def test_current_native_outputs_and_each_missing_receipt(self):
        directory = Path(os.environ["OMEGA_FRONT_HANDOFF_LOGS"])
        logs = {s: (directory / name).read_text(encoding="utf-8") for s, name in [("L0", "lex.log"), ("L1", "callable.log")]}
        items, report = inspect(**logs)
        self.assertTrue(items and all(items.values()), [key for key in items if not items[key]])
        # Mutate each distinct receipt: repeated document references to one file must *all* lose credit.
        challenged = set()
        for stage, log in logs.items():
            for line in log.splitlines():
                if line.startswith("OMEGA_GOLDEN_MATCH\t"):
                    name = line.replace("\\", "/").split("/fixtures/")[-1]
                elif line.startswith("OMEGA_HANDOFF_ASSERT\tinline-callable/"):
                    name = line.split("\t")[1]
                elif line.startswith("OMEGA_REVIEWED_COMPARISON\t"):
                    label = line.split("\t")[1]
                    name = "byte_cases::" + handoffs.reviewed_comparisons.sources()[label][2]
                else:
                    continue
                affected = {key for key in items if key[1] == name}
                if affected:
                    challenged.update(affected)
                    with self.subTest(stage=stage, name=name):
                        changed, report = inspect(**(logs | {stage: log.replace(line + "\n", "")}))
                        self.assertEqual(changed.keys(), items.keys())
                        self.assertEqual({key for key, ok in changed.items() if not ok}, affected)
                        self.assertTrue(any("unconfirmed named" in e and name in e for e in report["measurement_errors"]))
        self.assertEqual(challenged, set(items))
        control = "OMEGA_HANDOFF_ASSERT\tstructural-callable/alias-rhs\n"
        self.assertIn(control, logs["L1"])
        _, no_control = inspect(L1=logs["L1"].replace(control, ""))
        self.assertTrue(any("missing product assertion: L1/callable_aliases/" in e for e in no_control["measurement_errors"]))


if __name__ == "__main__":
    unittest.main()
