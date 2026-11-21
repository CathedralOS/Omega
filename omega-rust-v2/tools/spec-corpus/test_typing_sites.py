import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from typing_sites import GOLDEN_TEST, HEADER, SOURCE, attach_witnesses, details, finish, parse, record_return, source_link, summary


def _tsv(*rows, count=None):
    return "\n".join([*HEADER, *rows, f"#end\tsites={len(rows) if count is None else count}\tfiles=2\texcluded_items=1"]) + "\n"


class TypingSitesTests(unittest.TestCase):
    def test_equal_wording_keeps_distinct_origins_and_requires_a_passing_emitter(self):
        rows = ["a.rs\t3\t5\tf\tat\tliteral\tbad", "a.rs\t3\t35\tg\tat\tliteral\tbad",
                "a.rs\t4\t5\th\twaiting\tpending\t"]
        inventory = parse(_tsv(*rows))
        report = {"suites": {"L3": {"tests": []}}, "measurement_errors": []}
        for test, line, col, phase, status in [("domain", 3, 5, "refused", "ok"), ("case", 3, 35, "refused", "ok"),
                                               ("pending", 4, 5, "pending", "ok"), ("fails", 3, 5, "refused", "FAILED")]:
            report["suites"]["L3"]["tests"].append({"test": test + " - should panic" if test in ("case", "fails") else test, "target": "tests/x.rs", "status": status})
            record_return(report, "L3", "tests/x.rs", ("tests/x.rs", test),
                          f"OMEGA_TYPING_RETURN\t\t{SOURCE}/a.rs\t{line}\t{col}\t{phase}\tapp\tf.omg\t0\t1\t3\tbad")
        attach_witnesses(inventory, report)
        self.assertEqual(report["measurement_errors"] + inventory["errors"], [])
        self.assertEqual([r["stage_tests"] for r in inventory["rows"]], [["tests/x.rs::domain"], ["tests/x.rs::case"], ["tests/x.rs::pending"]])
        self.assertTrue(all("package_tests" not in row for row in inventory["rows"]))

    def test_returned_wording_distinguishes_branches_at_the_same_dynamic_site(self):
        inventory = parse(_tsv("a.rs\t3\t5\tf\tat\tdynamic\treason", "a.rs\t4\t5\tg\tat\tliteral\tbad"))
        target = "tests/x.rs"
        report = {"suites": {"L3": {"tests": []}}, "measurement_errors": []}
        for test, line, messages in [("field", 3, ["expected field", "expected field"]),
                                     ("callable", 3, ["expected callable", "expected field"]),
                                     ("literal", 4, ["changed wording"])]:
            report["suites"]["L3"]["tests"].append({"test": test, "target": target, "status": "ok"})
            for message in messages:
                record_return(report, "L3", target, (target, test),
                              f"OMEGA_TYPING_RETURN\t\t{SOURCE}/a.rs\t{line}\t5\trefused\tapp\tf.omg\t0\t1\t{len(message)}\t{message}")
        attach_witnesses(inventory, report)
        dynamic, literal = inventory["rows"]
        self.assertEqual(dynamic["returned_messages"], {
            "expected field": [f"{target}::field", f"{target}::callable"], "expected callable": [f"{target}::callable"]})
        self.assertNotIn("stage_tests", literal)
        self.assertEqual(len(inventory["errors"]), 1)
        self.assertIn("disagrees with the literal", inventory["errors"][0])
        rendered = "\n".join(details(inventory, Path.cwd()))
        self.assertIn(f"Returned “expected callable” to `{target}::callable`", rendered)

    def test_exact_api_credit_requires_a_passing_test_and_the_independent_expected_tuple(self):
        target, name = "tests/specialization_receipts.rs", "check"
        actual = f"OMEGA_TYPING_RETURN_CHECKED\t\t{SOURCE}/a.rs\t2\t5\trefused\tapp\tmain.omg\t4\t8\t3\tbad"
        expected = "app\tmain.omg\t4\t8\tbad"
        for record, status, exact in [(actual + "\t" + expected, "ok", True),
                                      (actual + "\t" + expected, "FAILED", False),
                                      (actual.replace("_CHECKED", ""), "ok", False),
                                      (actual, "ok", False)] + [
                (actual + "\t" + expected.replace(before, after), "ok", False)
                for before, after in [("app", "other"), ("main.omg", "other.omg"), ("4", "3"), ("8", "7"), ("bad", "err")]]:
            with self.subTest(record=record, status=status):
                report = {"measurement_errors": [], "suites": {"L3": {"tests": [{"target": target, "test": name, "status": status}]}}}
                inventory = parse(_tsv("a.rs\t2\t5\tf\tfor_site\tliteral\tbad"))
                record_return(report, "L3", target, (target, name), record)
                attach_witnesses(inventory, report)
                row = inventory["rows"][0]
                self.assertEqual(row.get("api_checks", {}), {"bad": [target + "::" + name]} if exact else {})
                self.assertNotIn("package_tests", row)
                self.assertEqual(bool(report["measurement_errors"]), "_CHECKED" in record and status == "ok" and not exact)

    def test_another_tree_cannot_supply_a_witness_for_the_same_relative_file(self):
        root = Path(__file__).absolute().parents[2]
        for path, valid in [(f"{SOURCE}/a.rs", True), ((root / SOURCE / "a.rs").as_posix(), True),
                            (f"other/{SOURCE}/a.rs", False), ((root / "other" / SOURCE / "a.rs").as_posix(), False)]:
            with self.subTest(path=path):
                inventory = parse(_tsv("a.rs\t3\t5\tf\tat\tliteral\tbad"))
                target, test = "tests/x.rs", "domain"
                report = {"suites": {"L3": {"tests": [{"target": target, "test": test, "status": "ok"}]}}, "measurement_errors": []}
                record_return(report, "L3", target, (target, test),
                              f"OMEGA_TYPING_RETURN\t\t{path}\t3\t5\trefused\tapp\tf.omg\t0\t1\t3\tbad")
                attach_witnesses(inventory, report)
                if valid:
                    self.assertEqual(inventory["rows"][0]["stage_tests"], [f"{target}::{test}"])
                    self.assertEqual(inventory["errors"], [])
                else:
                    self.assertNotIn("stage_tests", inventory["rows"][0])
                    self.assertIn("outside the inventory", inventory["errors"][0])

    def test_a_fixture_requires_its_original_source_bytes_location_reason_and_match(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "numbers/reject").mkdir(parents=True)
            path = root / "numbers/reject" / "value.omg"
            path.write_bytes("π\nbad".encode())
            path.with_suffix(".expected").write_bytes(b"refusal | 2:1 | bad | wrong type\n")
            for mutation in ("none", "omit", "duplicate", "wrong site", "lost text", "missing match"):
                with self.subTest(mutation=mutation):
                    inventory = parse(_tsv("a.rs\t11\t4\tf\tat\tdynamic\tm"))
                    target = "tests/expected.rs"
                    result = {"test": GOLDEN_TEST, "status": "ok", "target": target}
                    report = {"measurement_errors": [], "suites": {"L3": {"tests": [result]}},
                              "handoff_observations": {"goldens": {} if mutation == "missing match" else {"L3/numbers/reject/value.omg": GOLDEN_TEST}}}
                    start = 4 if mutation == "wrong site" else 3
                    message = "other type" if mutation == "lost text" else "wrong type"
                    marker = f"OMEGA_TYPING_RETURN\tnumbers/reject/value.omg\t{SOURCE}/a.rs\t11\t4\trefused\tgame\tfixture.omg\t{start}\t6\t10\t{message}"
                    for _ in range({"omit": 0, "duplicate": 2}.get(mutation, 1)):
                        record_return(report, "L3", target, (target, GOLDEN_TEST), marker)
                    attach_witnesses(inventory, report, root)
                    self.assertEqual(report["measurement_errors"], [])
                    self.assertEqual(bool(inventory["errors"]), mutation != "none")
                    self.assertEqual(inventory["rows"][0].get("fixtures", []), ["numbers/reject/value.omg"] if mutation == "none" else [])

    def test_malformed_or_repeated_test_results_cannot_supply_stage_witnesses(self):
        target, test = "tests/x.rs", "check"
        marker = f"OMEGA_TYPING_RETURN\t\t{SOURCE}/a.rs\t1\t2\trefused\ta\tf.omg\t0\t2\t3\tbad"
        for corruption in ("truncated record", "duplicate test", "outside captured test"):
            with self.subTest(corruption=corruption):
                names = [{"target": target, "test": test, "status": "ok"}]
                report = {"measurement_errors": [], "suites": {"L3": {"tests": names}}}
                inventory = parse(_tsv("a.rs\t1\t2\tf\tat\tliteral\tbad"))
                if corruption == "outside captured test":
                    record_return(report, "L3", target, None, marker)
                else:
                    record_return(report, "L3", target, (target, test), marker)
                    if corruption == "duplicate test":
                        names.extend(names.copy())
                    else:
                        record_return(report, "L3", target, (target, test), marker.rsplit("\t", 1)[0])
                attach_witnesses(inventory, report)
                self.assertNotIn("stage_tests", inventory["rows"][0])
                if corruption != "duplicate test":
                    self.assertTrue(report["measurement_errors"])

    def test_empty_or_impossible_file_roster_is_unmeasured(self):
        self.assertTrue(parse(_tsv())["errors"])
        single = _tsv("a.rs\t1\t1\tf\tat\tliteral\tm")
        self.assertTrue(parse(single.replace("files=2", "files=0"))["errors"])

    def test_rows_and_invalid_rows(self):
        good = "a.rs\t3\t5\tf\tat\tliteral\tbad%09type"
        inventory = parse(_tsv(good, "a.rs\tx\t1\tf\tat\tliteral\tm", "a.rs\t4\t1\tf\tat\tbogus\tm",
                               "a.rs\t5\t1\tf\tat\tliteral", "a.rs\t6\t1\tf\tat\tliteral\t%zz", "b.rs\t1\t1\t-\t-\tmalformed\toops"))
        self.assertEqual(inventory["rows"][0]["text"], "bad\ttype")
        self.assertEqual(len(inventory["rows"]), 2)
        self.assertEqual(len([e for e in inventory["errors"] if e.startswith("row ")]), 4)
        self.assertTrue(any("unparsed source" in e for e in inventory["errors"]))
        self.assertIn("missing or changed header", parse("nothing\n")["errors"])
        self.assertTrue(any("footer counts" in e for e in parse(_tsv(good, count=2))["errors"]))
        self.assertIn("missing or malformed footer", parse("\n".join([*HEADER, good]))["errors"])
        self.assertIn("rows are unsorted or repeat a location", parse(_tsv("b.rs\t1\t1\t-\tat\tliteral\tm", good))["errors"])

    def test_report_summary_links_and_failure(self):
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)

            def run(command, env, stdin, stdout, stderr):
                stdout.write(_tsv("a.rs\t3\t5\tf\tat\tliteral\tm", "a.rs\t9\t1\tg\tunsupported\tunsupported\t").encode())
                return subprocess.CompletedProcess(command, 0)
            report = {"measurement_errors": []}
            with mock.patch.object(subprocess, "run", run):
                finish(report, directory, {})
            text = (directory / "typing-sites.md").read_text(encoding="utf-8")
            self.assertIn(f"]({source_link('a.rs', directory)}#L3)", text)
            self.assertIn("## Unsupported-form fallbacks (not rules) (1)", text)
            self.assertIn("2 explicit `TypeError` call sites (1 literal, 1 unsupported)", summary(report)[1])
            self.assertEqual(report["measurement_errors"], [])
            with mock.patch.object(subprocess, "run", side_effect=OSError("no mbx")):
                finish(report, directory, {})
            self.assertIn("typing site inventory: command exited 1; see typing-sites.log", report["measurement_errors"])
            self.assertIn("unavailable", summary(report)[1])
            self.assertEqual(summary({}), [])


if __name__ == "__main__":
    unittest.main()
