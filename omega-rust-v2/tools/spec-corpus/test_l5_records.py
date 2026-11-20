"""L5 case, inventory, expected-count, source, fixture and obligation records are credited only to their emitters.

The capture blocks reproduce the shape and records of a real run
(target/layer-readiness/20261120T101628Z-efsw1wsf/L5.log). The fixed-sample
capture, including its six compiler refusal receipts, is copied from
target/diagnostics-sample.log.
"""
import os
from pathlib import Path
import unittest

from check_layer_readiness import collect, incomplete_cohorts, readiness_exit

REVISION = "36d45157895e1674fcdc01222fdee0faa1b68082"
SPEC_TEST = "spec_fragments_have_reviewed_lowering_outcomes"
SAMPLE_TEST = "fixed_sample_configurations_through_l5"
PREDICATE_TEST = "unsupported_predicate_call_is_refused_at_its_occurrence"
ORDERING_TEST = "contract_ordering_keeps_same_carrier_subjects_and_strictness"


def libtest(target, *tests):
    """One integration target as printed by `mbx test -- --include-ignored --show-output --format pretty`.

    Each test is (name, "ok" | "FAILED", captured lines)."""
    binary = Path(target.replace("\\", "/")).stem
    passed = [test for test in tests if test[1] == "ok"]
    failed = [test for test in tests if test[1] == "FAILED"]
    lines = ["", f"     Running {target} (target\\debug\\build\\omega-lowering/0123456789abcdef\\out\\{binary}-0123456789abcdef.exe)",
             "", f"running {len(tests)} test{'' if len(tests) == 1 else 's'}",
             *(f"test {name} ... {status}" for name, status, _ in tests), "", "successes:", ""]
    for name, _, captured in passed:
        if captured:
            lines += [f"---- {name} stdout ----", *captured, ""]
    lines += ["", "successes:", *(f"    {name}" for name, _, _ in passed), ""]
    if failed:
        lines += ["failures:", ""]
        for name, _, captured in failed:
            lines += [f"---- {name} stdout ----", *captured, "",
                      f"thread '{name}' (53048) panicked at layers\\05_prepared-trees-to-terminal-psi\\{target}:160:5:",
                      "assertion failed: failures.is_empty()",
                      "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace", ""]
        lines += ["", "failures:", *(f"    {name}" for name, _, _ in failed), ""]
    lines += [f"test result: {'FAILED' if failed else 'ok'}. {len(passed)} passed; {len(failed)} failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s", ""]
    if failed:
        lines.append(f"error: test failed, to rerun pass `-p omega-lowering --test {binary}`")
    return "\n".join(lines) + "\n"


SAMPLE_LINES = [
    "OMEGA_EXPECTED\tsample\t10",
    f"OMEGA_SOURCE\thistorical_samples\t{REVISION}",
    "standalone\tpsi\twitness matched",
    "OMEGA_CASE\tsample\tstandalone\tL5\texecutable product inspected\twitness matched",
    "module-library\tpsi\twitness matched",
    "OMEGA_CASE\tsample\tmodule-library\tL5\texecutable product inspected\twitness matched",
    "OMEGA_SAMPLE_REFUSAL\tcross-width\tL2\troot\tmain.omg\t475\t485\t20\t9\tMain::main\ta data carrier cannot own a machine",
    "cross-width\tL2\tmain.omg:20:9\ta data carrier cannot own a machine",
    "OMEGA_CASE\tsample\tcross-width\tL2\tcompatibility blocked\tmain.omg:20:9 a data carrier cannot own a machine",
    "OMEGA_SAMPLE_REFUSAL\trational-landing\tL2\troot\tmain.omg\t720\t730\t25\t9\tMain::main\ta data carrier cannot own a machine",
    "rational-landing\tL2\tmain.omg:25:9\ta data carrier cannot own a machine",
    "OMEGA_CASE\tsample\trational-landing\tL2\tcompatibility blocked\tmain.omg:25:9 a data carrier cannot own a machine",
    "OMEGA_SAMPLE_REFUSAL\tdirect-imports\tL2\troot\tm/b.omg\t4\t7\t1\t5\tm\\a\tunavailable import m\\a",
    "direct-imports\tL2\tm/b.omg:1:5\tunavailable import m\\a",
    "OMEGA_CASE\tsample\tdirect-imports\tL2\tcompatibility blocked\tm/b.omg:1:5 unavailable import m\\a",
    "direct-imports-v2\tpsi\twitness matched",
    "OMEGA_CASE\tsample\tdirect-imports-v2\tL5\texecutable product inspected\twitness matched",
    "OMEGA_SAMPLE_REFUSAL\tmissing-import-v2\tL2\troot\tmain.omg\t65\t66\t4\t30\tA\tunresolved name A",
    "missing-import-v2\tL2\tmain.omg:4:30\tunresolved name A",
    "OMEGA_CASE\tsample\tmissing-import-v2\tL2\tintended rejection verified\tmain.omg:4:30 unresolved name A",
    "OMEGA_SAMPLE_REFUSAL\tabstract-premise\tL1\troot\tmain.omg\t772\t776\t37\t27\tTrap\texpected `;` (bytes 772..776)",
    "abstract-premise\tL1\tmain.omg:37:27\texpected `;` (bytes 772..776)",
    "OMEGA_CASE\tsample\tabstract-premise\tL1\tcompatibility blocked\tmain.omg:37:27 expected `;` (bytes 772..776)",
    "OMEGA_SAMPLE_REFUSAL\tfalse-abstract-premise\tL1\troot\tmain.omg\t773\t777\t37\t27\tTrap\texpected `;` (bytes 773..777)",
    "false-abstract-premise\tL1\tmain.omg:37:27\texpected `;` (bytes 773..777)",
    "OMEGA_CASE\tsample\tfalse-abstract-premise\tL1\tcompatibility blocked\tmain.omg:37:27 expected `;` (bytes 773..777)",
    "import-from-dependency\tsetup\trequires Build dependency alias kit -> dep (mod_kernel), selecting other.omg",
    "OMEGA_CASE\tsample\timport-from-dependency\tsetup\tmissing context\trequires Build dependency alias kit -> dep (mod_kernel), selecting other.omg",
]

SPEC_LINES = [
    "OMEGA_INVENTORY\tBorrow,CallPrecondition,OperationContract,VerifyLibraryImplementation,MathematicalFact,Termination,ProveGuarantee",
    "OMEGA_EXPECTED\tspec\t3",
    "OMEGA_CASE\tspec\tbuild/assumed_binding_contracts.md:27\tL2\trecorded refusal\texample.omg:1:1: unresolved name Build::assume_binding_contract (bytes 0..30)",
    "OMEGA_OBLIGATION\tproofs/contracts.md:826\tBorrow\t1",
    "OMEGA_OBLIGATION\tproofs/contracts.md:826\tCallPrecondition\t1",
    "OMEGA_OBLIGATION\tproofs/contracts.md:826\tOperationContract\t1",
    "OMEGA_OBLIGATION\tproofs/contracts.md:826\tVerifyLibraryImplementation\t1",
    "OMEGA_CASE\tspec\tproofs/contracts.md:826\tL5\texecutable product inspected\tL5 lowered area, get",
    "OMEGA_OBLIGATION\tlanguage/machine_values.md:44\tVerifyLibraryImplementation\t1",
    "OMEGA_CASE\tspec\tlanguage/machine_values.md:44\tL5\topen gap\tL5 open gap: no Psi lowering for carrier runtime callable machine(u32) -> u8 | reader",
    "3 fragments. 1 resolution refusals, 1 open lowering gaps and 1 executable lowered products.",
    # Final per-example receipts, printed after every check ran; lowering_evidence.spec_receipts validates them.
    "OMEGA_L5_SPEC_RECEIPT\tbuild/assumed_binding_contracts.md:27\trecorded refusal\trecorded refusal\tfrontend=matched,lowering=none,semantic=none",
    "OMEGA_L5_SPEC_RECEIPT\tproofs/contracts.md:826\texecutable product inspected\tpassed\tfrontend=matched,lowering=matched,semantic=witness",
    "OMEGA_L5_SPEC_RECEIPT\tlanguage/machine_values.md:44\topen gap\tknown gap\tfrontend=matched,lowering=matched,semantic=none",
]

PREDICATE_LINES = [
    f"OMEGA_FIXTURE\t{PREDICATE_TEST}\tcontracts.omg\tTermination=1,VerifyLibraryImplementation=1\trefused: unsupported typed contract predicate record at contracts.omg"
    "\tapp\tcontracts.omg\t36\t47\t1\t37\tpredicate()",
    "OMEGA_CASE\thandoff\tcall in a contract predicate\tL5\topen gap\tThe clause needs the predicate and its selected call. Observed unsupported typed contract predicate record at contracts.omg: predicate()",
    "OMEGA_GAP_LOCATION\tcall in a contract predicate\tapp\tcontracts.omg\t36\t47\t1\t37\tpredicate()",
]
ORDERING_LINES = [f"OMEGA_FIXTURE\t{ORDERING_TEST}\tcontracts.omg\tProveGuarantee=3,VerifyLibraryImplementation=1\tproduced Psi"]


def sample(lines=SAMPLE_LINES, status="ok"):
    return libtest("tests\\sample_baseline.rs", ("directory_import_and_its_missing_import_twin", "ok", []), (SAMPLE_TEST, status, lines))


def spec(lines=SPEC_LINES, status="FAILED"):
    return libtest("tests\\spec_corpus.rs", (SPEC_TEST, status, lines))


def contracts(predicate=PREDICATE_LINES, ordering=ORDERING_LINES):
    return libtest("tests\\contracts.rs", (PREDICATE_TEST, "ok", predicate), (ORDERING_TEST, "ok", ordering))


class L5RecordTests(unittest.TestCase):
    def setUp(self):
        # The runner reads the versioned sample baseline relative to the V2 workspace.
        previous = os.getcwd()
        os.chdir(Path(__file__).absolute().parent.parent.parent)
        self.addCleanup(os.chdir, previous)
        self.report = {"cases": {}, "expected": {}, "obligations": {}, "package_fixtures": [], "measurement_errors": [],
                       "suites": {}, "cohort_sources": {"spec_tree_at_head": "f" * 40}}

    def collect(self, *blocks, stage="L5"):
        return collect("".join(blocks), stage, self.report)

    def errors_naming(self, *lines):
        for line in lines:
            self.assertTrue(any(line in error for error in self.report["measurement_errors"]), line)

    def test_records_of_a_real_run_are_bound_to_their_emitting_tests(self):
        tests, counts = self.collect(sample(), contracts(), spec())
        self.assertEqual(self.report["measurement_errors"], [])
        self.assertEqual(counts, {"passed": 4, "failed": 1, "ignored": 0})
        self.assertEqual(self.report["expected"], {"sample": 10, "spec": 3})
        self.assertEqual(self.report["cohort_sources"], {"spec_tree_at_head": "f" * 40, "historical_samples": REVISION})
        cases = self.report["cases"]
        self.assertEqual((cases["sample/cross-width"]["target"], cases["sample/cross-width"]["test"]), ("tests\\sample_baseline.rs", SAMPLE_TEST))
        self.assertEqual(cases["sample/cross-width"]["later_layers_not_reached"], ["L3", "L4", "L5"])
        # Each of the six printed refusals is attached to its case with its exact bytes and position.
        refusals = {key: case["compiler_refusal"] for key, case in cases.items() if "compiler_refusal" in case}
        self.assertEqual({key: (site["stage"], site["package"], site["file"], site["byte_start"], site["byte_end"],
                                site["line"], site["byte_column"], site["occurrence_form"], site["occurrence"], site["reason"])
                          for key, site in refusals.items()}, {
            "sample/cross-width": ("L2", "root", "main.omg", 475, 485, 20, 9, "text", "Main::main", "a data carrier cannot own a machine"),
            "sample/rational-landing": ("L2", "root", "main.omg", 720, 730, 25, 9, "text", "Main::main", "a data carrier cannot own a machine"),
            "sample/direct-imports": ("L2", "root", "m/b.omg", 4, 7, 1, 5, "text", "m\\a", "unavailable import m\\a"),
            "sample/missing-import-v2": ("L2", "root", "main.omg", 65, 66, 4, 30, "text", "A", "unresolved name A"),
            "sample/abstract-premise": ("L1", "root", "main.omg", 772, 776, 37, 27, "text", "Trap", "expected `;` (bytes 772..776)"),
            "sample/false-abstract-premise": ("L1", "root", "main.omg", 773, 777, 37, 27, "text", "Trap", "expected `;` (bytes 773..777)"),
        })
        self.assertTrue(all(site["after_static_evaluation"] is False for site in refusals.values()))
        self.assertEqual(cases["sample/abstract-premise"]["later_layers_not_reached"], ["L2", "L3", "L4", "L5"])
        self.assertNotIn("compiler_refusal", cases["sample/import-from-dependency"])
        self.assertEqual((cases["spec/proofs/contracts.md:826"]["target"], cases["spec/proofs/contracts.md:826"]["test"]), ("tests\\spec_corpus.rs", SPEC_TEST))
        self.assertEqual(cases["handoff/call in a contract predicate"]["test"], PREDICATE_TEST)
        self.assertEqual(self.report["obligations"], {
            "Borrow": {"proofs/contracts.md:826": 1}, "CallPrecondition": {"proofs/contracts.md:826": 1},
            "OperationContract": {"proofs/contracts.md:826": 1}, "MathematicalFact": {}, "Termination": {}, "ProveGuarantee": {},
            "VerifyLibraryImplementation": {"proofs/contracts.md:826": 1, "language/machine_values.md:44": 1}})
        self.assertEqual([(f["target"], f["test"], f["outcome"].split(":")[0]) for f in self.report["package_fixtures"]],
                         [("tests\\contracts.rs", PREDICATE_TEST, "refused"), ("tests\\contracts.rs", ORDERING_TEST, "produced Psi")])
        self.assertEqual(list(incomplete_cohorts(self.report)), [])
        # Receipts are evidence records, not failure text.
        spec_result = next(test for test in tests if test["test"] == SPEC_TEST)
        self.assertEqual(spec_result["status"], "FAILED")
        self.assertFalse(any(line.startswith("OMEGA_") for line in spec_result["diagnostic"]))
        # The open gaps and the failed aggregate still block readiness; binding does not change a claim.
        self.report["suites"]["L5"] = {"exit": 101, "tests": tests}
        self.assertEqual(readiness_exit(self.report), 1)

    def test_a_record_printed_by_another_test_or_outside_a_capture_is_not_credited(self):
        stray_expected = "OMEGA_EXPECTED\tspec\t3"
        borrowed_case = "OMEGA_CASE\tspec\tproofs/contracts.md:54\tL2\trecorded refusal\texample.omg:1:21: unresolved name T (bytes 20..21)"
        borrowed_count = "OMEGA_OBLIGATION\tproofs/contracts.md:826\tBorrow\t1"
        substituted = f"OMEGA_FIXTURE\t{PREDICATE_TEST}\tcontracts.omg\tProveGuarantee=3,VerifyLibraryImplementation=1\tproduced Psi"
        unnamed = "OMEGA_FIXTURE\t<unnamed>\tcontracts.omg\tVerifyLibraryImplementation=1\tproduced Psi"
        sample_elsewhere = "OMEGA_CASE\tsample\tstandalone\tL5\texecutable product inspected\twitness matched"
        self.collect(stray_expected + "\n",
                     spec(SPEC_LINES[:1]),
                     libtest("tests\\fields.rs", ("borrowed", "ok", [borrowed_case, borrowed_count, sample_elsewhere])),
                     contracts(ordering=[substituted, unnamed]))
        self.errors_naming(stray_expected, borrowed_case, borrowed_count, substituted, unnamed, sample_elsewhere)
        self.assertEqual(len(self.report["measurement_errors"]), 6)
        self.assertEqual(self.report["expected"], {})
        self.assertNotIn("spec/proofs/contracts.md:54", self.report["cases"])
        self.assertNotIn("sample/standalone", self.report["cases"])
        self.assertEqual(self.report["obligations"]["Borrow"], {})
        # The correctly attributed records from the same targets are kept.
        self.assertEqual([f["test"] for f in self.report["package_fixtures"]], [PREDICATE_TEST])
        self.assertIn("handoff/call in a contract predicate", self.report["cases"])
        self.assertIn("spec: observed 0 results, expected an inventory", list(incomplete_cohorts(self.report)))

    def test_records_from_an_earlier_suite_are_not_credited_to_l5(self):
        self.collect(sample(), stage="L4")
        self.assertEqual(self.report["cases"], {})
        self.assertEqual(self.report["expected"], {})
        self.assertTrue(all(error.startswith("L4: ") for error in self.report["measurement_errors"]))
        # Ten case results, six refusal receipts and the expected count and source identity.
        self.assertEqual(len(self.report["measurement_errors"]), 18)

    def test_duplicate_records_keep_the_first_observation(self):
        repeated = [
            "OMEGA_CASE\tspec\tproofs/contracts.md:826\tL5\tdeclaration or empty product\tL5 schema only Area",
            "OMEGA_EXPECTED\tspec\t2",
            "OMEGA_OBLIGATION\tproofs/contracts.md:826\tBorrow\t7",
            "OMEGA_INVENTORY\tMove",
        ]
        self.collect(sample(SAMPLE_LINES + SAMPLE_LINES[:4]), spec(SPEC_LINES + repeated))
        cases = self.report["cases"]
        self.assertEqual(cases["spec/proofs/contracts.md:826"]["status"], "executable product inspected")
        self.assertEqual(self.report["expected"], {"sample": 10, "spec": 3})
        self.assertEqual(self.report["obligations"]["Borrow"], {"proofs/contracts.md:826": 1})
        self.assertNotIn("Move", self.report["obligations"])
        self.assertEqual(self.report["cohort_sources"]["historical_samples"], REVISION)
        self.errors_naming("duplicate result for spec/proofs/contracts.md:826", "duplicate result for sample/standalone",
                           *repeated[1:], *SAMPLE_LINES[:2])
        self.assertEqual(len(self.report["measurement_errors"]), 7)
        self.assertEqual(sum(row["cohort"] == "sample" for row in cases.values()), 10)

    def test_unknown_or_malformed_records_are_reported_and_the_rest_preserved(self):
        bad_spec = [
            "OMEGA_EXPECTED\tfragments\t3",
            "OMEGA_EXPECTED\tspec\t03",
            "OMEGA_CASE\tspec\tbuild/configuration.md\tL2\trecorded refusal\texample.omg:1:1: unresolved name builder (bytes 0..7)",
            "OMEGA_CASE\tspec\tproofs/contracts.md:54\tL2\texecutable product inspected\tL5 lowered area",
            "OMEGA_CASE\tspec\tproofs/contracts.md:406\tL2\trecorded refusal",
            "OMEGA_CASE\tfragment\tproofs/contracts.md:406\tL2\trecorded refusal\tx",
            "OMEGA_OBLIGATION\tlanguage/machine_values.md:44\tBorrow\t0",
            "OMEGA_OBLIGATION\tlanguage/machine_values.md:44\tMove\t1",
            "OMEGA_INVENTORY\tBorrow,Borrow",
        ]
        bad_sample = [
            "OMEGA_SOURCE\tspec_tree_at_head\t" + "a" * 40,
            "OMEGA_SOURCE\thistorical_samples\t" + "0" * 40,
            "OMEGA_CASE\tsample\tstandalone-v3\tL5\texecutable product inspected\twitness matched",
            "OMEGA_CASE\tsample\tstandalone\tL5\trecorded refusal\twitness matched",
        ]
        bad_fixtures = [
            f"OMEGA_FIXTURE\t{ORDERING_TEST}\tcontracts.omg\tVerifyLibraryImplementation=1\tlowered",
            f"OMEGA_FIXTURE\t{ORDERING_TEST}\tcontracts.omg\tBorrow=1,Borrow=2\tproduced Psi",
            f"OMEGA_FIXTURE\t{ORDERING_TEST}\t\tBorrow=1\tproduced Psi",
        ]
        self.collect(sample(bad_sample[:2] + SAMPLE_LINES + bad_sample[2:]),
                     contracts(ordering=bad_fixtures + ORDERING_LINES),
                     spec(SPEC_LINES[:2] + bad_spec + SPEC_LINES[2:]))
        self.errors_naming(*bad_spec, *bad_sample, *bad_fixtures)
        self.assertEqual(len(self.report["measurement_errors"]), len(bad_spec) + len(bad_sample) + len(bad_fixtures))
        self.assertEqual(self.report["cohort_sources"], {"spec_tree_at_head": "f" * 40, "historical_samples": REVISION})
        self.assertEqual(self.report["expected"], {"sample": 10, "spec": 3})
        self.assertEqual(self.report["cases"]["sample/standalone"]["status"], "executable product inspected")
        self.assertEqual(len(self.report["package_fixtures"]), 2)
        self.assertEqual(self.report["obligations"]["VerifyLibraryImplementation"]["language/machine_values.md:44"], 1)
        self.assertEqual(list(incomplete_cohorts(self.report)), [])

    def test_a_passing_comparison_must_print_the_outcome_it_compared_with_the_baseline(self):
        # The compiler's receipt and the case result change together, as in a real changed outcome.
        changed = [line.replace("a data carrier cannot own a machine", "unresolved name Width")
                   if line.startswith(("OMEGA_CASE\tsample\tcross-width\t", "OMEGA_SAMPLE_REFUSAL\tcross-width\t")) else line
                   for line in SAMPLE_LINES]
        self.collect(sample(changed))
        self.assertEqual(len(self.report["measurement_errors"]), 1)
        self.assertIn("sample/cross-width", self.report["measurement_errors"][0])
        self.assertEqual(self.report["cases"]["sample/cross-width"]["detail"], "main.omg:20:9 unresolved name Width")
        # A failed comparison already reports the change as a failed assertion.
        self.setUp()
        self.collect(sample(changed, status="FAILED"))
        self.assertEqual(self.report["measurement_errors"], [])

    def test_obligation_counts_need_a_spec_case_that_reached_lowering(self):
        self.collect(spec(SPEC_LINES[:2] + [
            "OMEGA_OBLIGATION\tresources/carry.md:6\tVerifyLibraryImplementation\t1",
            "OMEGA_OBLIGATION\tresources/access.md:20\tBorrow\t1",
            "OMEGA_CASE\tspec\tresources/access.md:20\tL2\trecorded refusal\texample.omg:1:8: unresolved name Controller (bytes 7..29)",
        ]))
        incomplete = list(incomplete_cohorts(self.report))
        self.assertIn("spec/resources/carry.md:6: obligation counts without a case result", incomplete)
        self.assertIn("spec/resources/access.md:20: obligation counts printed after preparation, but the case stopped at L2", incomplete)
        self.assertIn("spec: observed 1 results, expected 3", incomplete)


CROSS_RECEIPT = "OMEGA_SAMPLE_REFUSAL\tcross-width\t"
CROSS_CASE = "OMEGA_CASE\tsample\tcross-width\t"
CROSS_REASON = "a data carrier cannot own a machine"


def receipt(label="cross-width", stage="L2", package="root", file="main.omg", start="475", end="485", row="20", column="9",
            occurrence="Main::main", reason=CROSS_REASON):
    return "\t".join(["OMEGA_SAMPLE_REFUSAL", label, stage, package, file, start, end, row, column, occurrence, reason])


def case(stage="L2", status="compatibility blocked", detail="main.omg:20:9 " + CROSS_REASON):
    return f"{CROSS_CASE}{stage}\t{status}\t{detail}"


class SampleRefusalTests(unittest.TestCase):
    """Faults in the fixed sample's compiler refusal receipts, each with the measurement it must produce.

    The sample test is marked failed so its changed rows are not also compared with the versioned baseline."""

    def outcome(self, replacements, extra=()):
        previous = os.getcwd()
        os.chdir(Path(__file__).absolute().parent.parent.parent)
        self.addCleanup(os.chdir, previous)
        lines = []
        for line in SAMPLE_LINES:
            prefix = next((prefix for prefix in replacements if line.startswith(prefix)), None)
            line = replacements[prefix] if prefix else line
            if line is not None:
                lines.append(line)
        self.report = {"cases": {}, "expected": {}, "obligations": {}, "package_fixtures": [], "measurement_errors": [],
                       "suites": {}, "cohort_sources": {}}
        collect(sample(lines + list(extra), status="FAILED"), "L5", self.report)
        return self.report["measurement_errors"]

    def refusal(self, label="cross-width"):
        return self.report["cases"]["sample/" + label].get("compiler_refusal")

    def test_the_unchanged_receipts_attach_without_errors(self):
        self.assertEqual(self.outcome({}), [])
        self.assertEqual(sum("compiler_refusal" in row for row in self.report["cases"].values()), 6)

    def test_a_missing_receipt_is_reported_once(self):
        self.assertEqual(self.outcome({CROSS_RECEIPT: None}), ["sample/cross-width: missing the compiler's original refusal"])

    def test_an_invalid_receipt_is_reported_without_a_second_missing_error(self):
        for line, why in [(receipt(reason="100%"), "malformed percent escape"),
                          (receipt(reason="a%0adata"), "malformed percent escape"),
                          (receipt(occurrence="Main%3A%3Amain"), "not escaped as the emitter escapes it"),
                          (receipt(occurrence="Main::mai"), "has 9 bytes but the range 475..485 has 10"),
                          (receipt(row="-"), "incomplete line and byte column"),
                          (receipt(stage="setup"), "not a compiler stage"),
                          (receipt() + "\textra", "expected 11 tab-separated fields, found 12")]:
            with self.subTest(line=line):
                errors = self.outcome({CROSS_RECEIPT: line})
                self.assertEqual(len(errors), 1, errors)
                self.assertTrue(errors[0].startswith("L5: invalid sample refusal: "), errors)
                self.assertIn(why, errors[0])
                self.assertTrue(errors[0].endswith(line))
                self.assertIsNone(self.refusal())

    def test_a_duplicate_receipt_keeps_the_first(self):
        errors = self.outcome({}, [receipt(reason="unresolved name Width")])
        self.assertEqual(len(errors), 1)
        self.assertIn("duplicate refusal for cross-width", errors[0])
        self.assertEqual(self.refusal()["reason"], CROSS_REASON)

    def test_the_receipt_stage_and_resumption_must_be_the_reported_stage(self):
        self.assertEqual(self.outcome({CROSS_RECEIPT: receipt(stage="L1")}),
                         ["sample/cross-width: compiler stopped at L1 but the case reports L2"])
        self.assertEqual(self.outcome({CROSS_RECEIPT: receipt(stage="L3"), CROSS_CASE: case(stage="L3-resumed")}),
                         ["sample/cross-width: compiler stopped at L3 but the case reports L3-resumed"])
        self.assertEqual(self.outcome({CROSS_RECEIPT: receipt(stage="L3-resumed"), CROSS_CASE: case(stage="L3")}),
                         ["sample/cross-width: compiler stopped at L3-resumed but the case reports L3"])
        self.assertEqual(self.outcome({CROSS_RECEIPT: receipt(stage="L3-resumed"), CROSS_CASE: case(stage="L3-resumed")}), [])
        self.assertEqual((self.refusal()["stage"], self.refusal()["after_static_evaluation"]), ("L3", True))
        self.assertEqual(self.report["cases"]["sample/cross-width"]["later_layers_not_reached"], ["L5"])

    def test_the_reason_and_position_must_be_the_case_detail(self):
        for line in [receipt(reason="unresolved name Width"), receipt(row="21"), receipt(column="10", start="476", end="486"),
                     receipt(file="other.omg")]:
            with self.subTest(line=line):
                errors = self.outcome({CROSS_RECEIPT: line})
                self.assertEqual(len(errors), 1, errors)
                self.assertTrue(errors[0].startswith("sample/cross-width: case detail "), errors)
                self.assertIsNone(self.refusal())

    def test_escaped_reasons_are_decoded_and_flattened_like_the_detail(self):
        self.assertEqual(self.outcome({CROSS_RECEIPT: receipt(reason="why%09100%25%0Amore"),
                                       CROSS_CASE: case(detail="main.omg:20:9 why 100% more")}), [])
        self.assertEqual(self.refusal()["reason"], "why\t100%\nmore")
        # The undecoded reason is not the compiler's reason.
        self.assertEqual(len(self.outcome({CROSS_RECEIPT: receipt(reason="why%09100%25%0Amore"),
                                           CROSS_CASE: case(detail="main.omg:20:9 why%09100%25%0Amore")})), 1)

    def test_a_same_named_file_of_another_package_keeps_its_owner(self):
        errors = self.outcome({CROSS_RECEIPT: receipt(package="dep")})
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("'<dep>/main.omg:20:9 " + CROSS_REASON, errors[0])
        self.assertEqual(self.outcome({CROSS_RECEIPT: receipt(package="dep"), CROSS_CASE: case(detail="<dep>/main.omg:20:9 " + CROSS_REASON)}), [])
        self.assertEqual((self.refusal()["package"], self.refusal()["file"], self.refusal()["line"]), ("dep", "main.omg", 20))
        # Bytes of another package are unavailable; only its byte range locates the refusal.
        unavailable = receipt(package="@omega/toolchain", row="-", column="-", occurrence="-")
        self.assertEqual(len(self.outcome({CROSS_RECEIPT: unavailable})), 1)
        self.assertEqual(self.outcome({CROSS_RECEIPT: unavailable, CROSS_CASE: case(
            detail="<@omega/toolchain>/main.omg (bytes 475..485) " + CROSS_REASON)}), [])
        self.assertEqual((self.refusal()["occurrence_form"], self.refusal()["occurrence"]), ("unavailable", None))
        self.assertNotIn("line", self.refusal())

    def test_receipts_need_a_refused_case(self):
        self.assertEqual(self.outcome({}, [receipt(label="standalone")]),
                         ["sample/standalone: compiler refusal for a case reported as executable product inspected"])
        self.assertEqual(self.outcome({}, [receipt(label="import-from-dependency")]),
                         ["sample/import-from-dependency: compiler refusal for a case reported as missing context"])
        self.assertEqual(self.outcome({}, [receipt(label="no-such-sample")]),
                         ["sample/no-such-sample: compiler refusal for a sample without a case result"])

    def test_inspection_failures_report_the_actual_refusal(self):
        got = ", got L2 main.omg:20:9 " + CROSS_REASON
        for detail in ["expected Psi" + got, "expected L2 main.omg:1:1 unresolved name B" + got]:
            with self.subTest(detail=detail):
                self.assertEqual(self.outcome({CROSS_CASE: case(status="inspection failed", detail=detail)}), [])
                self.assertEqual(self.refusal()["stage"], "L2")
        self.assertEqual(self.outcome({CROSS_RECEIPT: None, CROSS_CASE: case(status="inspection failed", detail="expected Psi" + got)}),
                         ["sample/cross-width: missing the compiler's original refusal"])
        for detail in ["expected Psi, got L2 main.omg:20:9 unresolved name Width", "expected Psi, got L1 main.omg:20:9 " + CROSS_REASON,
                       "got L2 main.omg:20:9 " + CROSS_REASON, "expected " + got]:
            with self.subTest(detail=detail):
                errors = self.outcome({CROSS_CASE: case(status="inspection failed", detail=detail)})
                self.assertEqual(len(errors), 1, errors)
                self.assertIn("does not end with the compiler refusal", errors[0])
        # A refusal by L5 itself is reported at L5 with no later layer.
        self.assertEqual(self.outcome({CROSS_RECEIPT: receipt(stage="L5"), CROSS_CASE: case(
            stage="L5", status="inspection failed", detail="expected Psi, got L5 main.omg:20:9 " + CROSS_REASON)}), [])
        self.assertEqual(self.refusal()["later_layers_not_reached"], [])

    def test_an_unexpected_product_needs_no_receipt(self):
        for detail in ["produced Psi without the required behavior", "produced Psi; expected L2 main.omg:4:30 unresolved name A"]:
            with self.subTest(detail=detail):
                product = case(stage="L5", status="unexpected Psi product", detail=detail)
                self.assertEqual(self.outcome({CROSS_RECEIPT: None, CROSS_CASE: product}), [])
                self.assertEqual(self.outcome({CROSS_CASE: product}),
                                 ["sample/cross-width: compiler refusal for a case reported as unexpected Psi product"])
        # An inspection failure is a refusal that broke its expectation, so its receipt is required.
        failed = case(stage="L5", status="inspection failed", detail="produced Psi without the required behavior")
        self.assertEqual(self.outcome({CROSS_RECEIPT: None, CROSS_CASE: failed}),
                         ["sample/cross-width: missing the compiler's original refusal"])


if __name__ == "__main__":
    unittest.main()
