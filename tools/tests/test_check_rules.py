"""Seed each admission rule's violation, rather than only blessing today's tree."""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_rules
from check_rules import (
    changed_golden_rows, count_source_reads, fact_row_violations, golden_rows,
    golden_violations, lanes, nearest_rows, program_shingles, read_baseline,
    source_read_counts, source_read_violations, subject_violations, write_baseline,
)


class SourceReadTests(unittest.TestCase):
    def test_reads_of_rust_and_manifest_source_count(self):
        text = '''
            let a = include_str!("lib.rs");
            let b = include_str!(
                "../../realization/provider-planning/src/service_declarations.rs"
            );
            let c = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
            let d = std::fs::read_to_string(root.join("src/main.rs"));
        '''
        self.assertEqual(count_source_reads(text), 4)

    def test_fixture_data_is_not_source_text(self):
        text = 'let program = include_str!("fixtures/main.omg");\n' \
               'let image = include_bytes!("golden.bin");\n' \
               'let c = include_str!("probe.c");'
        self.assertEqual(count_source_reads(text), 0)

    def test_a_new_read_fails(self):
        violations = source_read_violations({"a.rs": 3, "b.rs": 1}, {"a.rs": 3})
        self.assertEqual(len(violations), 1)
        self.assertIn("b.rs: 1 source-text reads, baseline 0", violations[0])

    def test_a_removed_read_lowers_the_baseline(self):
        violations = source_read_violations({"a.rs": 2}, {"a.rs": 3})
        self.assertIn("--update-source-reads", violations[0])
        self.assertEqual(source_read_violations({"a.rs": 2}, read_baseline(write_baseline({"a.rs": 2}))), [])

    def test_the_tree_matches_its_baseline(self):
        baseline = read_baseline((check_rules.ROOT / check_rules.READS_BASELINE).read_text())
        self.assertEqual(source_read_violations(source_read_counts(), baseline), [])


class GoldenTests(unittest.TestCase):
    GOLDEN = ("# header\n"
              "pass/facts/row_through_local_exit checked 10ms @abc\n"
              "\tdiagnostic line\n"
              "fail/facts/row/local rejected 12ms expected @abc\n")

    def setUp(self):
        self.rows = golden_rows(self.GOLDEN)
        self.dirs = {"pass/facts/row_through_local_exit", "fail/facts/row/local",
                     "pass/facts/new_fixture"}

    def test_rows_skip_diagnostics_and_comments(self):
        self.assertEqual(self.rows, {"pass/facts/row_through_local_exit", "fail/facts/row/local"})

    def test_edited_fixture_without_a_new_row_fails(self):
        changes = [("M", "tests/omega/pass/facts/row_through_local_exit/main.omg")]
        violations = golden_violations(changes, self.rows, self.rows, set(), self.dirs)
        self.assertEqual(len(violations), 1)
        self.assertIn("--filter pass/facts/row_through_local_exit", violations[0])

    def test_re_recorded_fixture_passes(self):
        diff = ("--- a/tests/omega/corpus_outcomes.txt\n+++ b/tests/omega/corpus_outcomes.txt\n"
                "-pass/facts/row_through_local_exit checked 10ms @abc\n"
                "+pass/facts/row_through_local_exit checked 11ms @def\n")
        changes = [("M", "tests/omega/pass/facts/row_through_local_exit/main.omg")]
        self.assertEqual(golden_violations(changes, self.rows, self.rows,
                                           changed_golden_rows(diff), self.dirs), [])

    def test_nested_fixture_resolves_to_its_row(self):
        changes = [("M", "tests/omega/fail/facts/row/local/build.omg")]
        violations = golden_violations(changes, self.rows, self.rows, set(), self.dirs)
        self.assertIn("--filter fail/facts/row/local", violations[0])

    def test_new_fixture_needs_a_row(self):
        changes = [("A", "tests/omega/pass/facts/new_fixture/main.omg")]
        self.assertEqual(len(golden_violations(changes, self.rows, self.rows, set(), self.dirs)), 1)

    def test_deleted_fixture_must_drop_its_row(self):
        changes = [("D", "tests/omega/fail/facts/row/local/main.omg")]
        dirs = self.dirs - {"fail/facts/row/local"}
        violations = golden_violations(changes, self.rows, self.rows, set(), dirs)
        self.assertIn("deleted fixture(s) keep a row", violations[0])
        self.assertEqual(golden_violations(changes, self.rows, self.rows - {"fail/facts/row/local"},
                                           {"fail/facts/row/local"}, dirs), [])

    def test_readme_and_non_fixture_paths_are_ignored(self):
        changes = [("M", "tests/omega/pass/facts/row_through_local_exit/README.md"),
                   ("M", "omega-rust/psi/src/lib.rs")]
        self.assertEqual(golden_violations(changes, self.rows, self.rows, set(), self.dirs), [])


class FactRowTests(unittest.TestCase):
    TWO = """
        machine step1(value: u64) -> u64 { return value + 1u64; }
        machine step2(value: u64) -> u64 { return value + 1u64; }
        machine compute(saved: u64) -> u64 requires saved >= 40u64 && saved <= 80u64 {
            let c1 = step1(10u64);
            let c2 = step2(c1);
            return saved - 40u64;
        }
    """
    THREE = TWO.replace("let c2 = step2(c1);",
                        "let c2 = step2(c1);\n            let c3 = step3(c2);").replace(
        "machine compute", "machine step3(value: u64) -> u64 { return value + 1u64; }\n        machine compute")
    OTHER = """
        data Packet { case Small(v: u8), case Large(v: u64) }
        machine extract(packet: Packet) -> u64 {
            match packet { Small(v) -> return v as u64, Large(v) -> return v }
        }
    """

    def nearest(self):
        return nearest_rows({"live_across_three": program_shingles(self.THREE)},
                            {"live_across_two": program_shingles(self.TWO),
                             "variant_payload": program_shingles(self.OTHER)})

    def test_count_variant_must_name_its_nearest_row(self):
        nearest = self.nearest()
        self.assertEqual(nearest["live_across_three"][1], "live_across_two")
        self.assertGreaterEqual(nearest["live_across_three"][0], check_rules.NEAR_DUPLICATE)
        violations = fact_row_violations(nearest, {"live_across_three": "# Three calls\n"})
        self.assertEqual(len(violations), 1)
        self.assertIn("matches live_across_two", violations[0])

    def test_naming_the_nearest_row_passes(self):
        readme = "## Nearest existing row\n[live_across_two](../live_across_two/README.md): ...\n"
        self.assertEqual(fact_row_violations(self.nearest(), {"live_across_three": readme}), [])

    def test_a_different_program_needs_no_comparison(self):
        nearest = nearest_rows({"variant_payload": program_shingles(self.OTHER)},
                               {"live_across_two": program_shingles(self.TWO)})
        self.assertLess(nearest["variant_payload"][0], check_rules.NEAR_DUPLICATE)
        self.assertEqual(fact_row_violations(nearest, {}), [])


class LaneTests(unittest.TestCase):
    def setUp(self):
        self.lanes = lanes((check_rules.ROOT / "AGENTS.md").read_text())

    def test_lanes_come_from_the_agents_table(self):
        self.assertTrue({"psi", "omega", "backend", "repo", "tools", "bootstrap-omega"} <= self.lanes)
        self.assertNotIn("facts", self.lanes)

    def test_listed_lane_passes(self):
        self.assertEqual(subject_violations("psi: carry the bound through joins", self.lanes), [])

    def test_unlisted_or_missing_lane_fails(self):
        for subject in ("fix: preserve C layout policy identity through owned values",
                        "facts(row): add 9-place row", "loc: fold walkers",
                        "Capture runtime Value data indices"):
            self.assertEqual(len(subject_violations(subject, self.lanes)), 1, subject)

    def test_long_subject_fails(self):
        self.assertEqual(len(subject_violations("psi: " + "x" * 90, self.lanes)), 1)


if __name__ == "__main__":
    unittest.main()
