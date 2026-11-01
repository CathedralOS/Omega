"""Seed each admission rule's violation, rather than only blessing today's tree."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_rules
from check_rules import (
    board_task_ids, changed_golden_rows, citing_documents, count_source_reads,
    count_status_phrases, dangling_task_ids, dangling_task_violations, fact_row_violations,
    golden_rows, golden_violations, lanes, measurement_violations, nearest_rows,
    program_shingles, read_baseline, read_dangling_baseline, source_read_counts,
    source_read_violations, status_phrase_counts, status_phrase_violations,
    subject_violations, write_baseline, write_dangling_baseline, write_status_baseline,
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


class StatusPhraseTests(unittest.TestCase):
    # Passages deleted from omega-rust/pipeline.md and wiki/spec on 2026-11-01.
    STATUS = """
## Implementation status

**Implementation status (re-verified `52d95a9d76f`, linux x86-64):** the cyclic
### Acceptance status
Not yet closed:
it executes with its authored exit 70 on macOS ARM64.
fields, and return 70 in native and checked-interpreter execution on macOS ARM64.
callback/policy capture, and Terminal-first target orchestration are still
open.
review projection remain migration work. This is
multi-slot readiness remains open. Corpus consumers use this same executed
Resolver/typing continuation now lives in the Psi compiler.
This boundary is implemented; the common Psi
These are semantic and evidence requirements, not a claim that every described
"""

    # Rules that share words with status and must not count.
    RULES = """
## Status codes
A host exposing fewer status bits does not authorize truncation.
An acyclic join may introduce a merge token only from matching valid tokens.
Check and declaration review must still work without executable publication.
record the requirements still open on the value that layer emits
The process exits with status 70 only through a checked exit.
A detached editable document is not an instance of its described type.
"""

    def test_status_wording_counts(self):
        self.assertEqual(count_status_phrases(self.STATUS), 12)

    def test_rules_do_not_count(self):
        self.assertEqual(count_status_phrases(self.RULES), 0)

    def test_new_status_in_a_contract_fails(self):
        violations = status_phrase_violations({"wiki/spec/a.md": 2, "wiki/spec/b.md": 1},
                                              {"wiki/spec/a.md": 2})
        self.assertEqual(len(violations), 1)
        self.assertIn("wiki/spec/b.md: 1 status phrases, baseline 0", violations[0])
        self.assertIn("TASKS.md", violations[0])

    def test_removed_status_lowers_the_baseline(self):
        violations = status_phrase_violations({"omega-rust/pipeline.md": 1},
                                              {"omega-rust/pipeline.md": 31})
        self.assertIn("--update-status-phrases", violations[0])
        rewritten = read_baseline(write_status_baseline({"omega-rust/pipeline.md": 1}))
        self.assertEqual(status_phrase_violations({"omega-rust/pipeline.md": 1}, rewritten), [])

    def test_contracts_are_pipeline_and_spec_only(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "omega-rust").mkdir()
            (root / "omega-rust/pipeline.md").write_text("X now owns Y.\n")
            (root / "wiki/spec/build").mkdir(parents=True)
            (root / "wiki/spec/build/a.md").write_text("Not yet closed:\n")
            (root / "wiki/drafts").mkdir()
            (root / "wiki/drafts/b.md").write_text("Not yet closed:\n")
            (root / "TASKS.md").write_text("now passes; not yet\n")
            self.assertEqual(status_phrase_counts(root),
                             {"omega-rust/pipeline.md": 1, "wiki/spec/build/a.md": 1})

    def test_the_tree_matches_its_baseline(self):
        baseline = read_baseline((check_rules.ROOT / check_rules.STATUS_BASELINE).read_text())
        self.assertEqual(status_phrase_violations(status_phrase_counts(), baseline), [])


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


class MeasurementTests(unittest.TestCase):
    ROOT = check_rules.MEASUREMENTS_ROOT

    def violations(self, changes, lines=10):
        return measurement_violations(changes, lambda path: lines)

    def test_added_raw_data_fails(self):
        for name in ("run/summary.json", "run/transcript.txt", "run/profile.json.gz",
                     "run/candidate.patch", "run/probe.py", "run/.gitattributes", "data.csv"):
            violations = self.violations([("A", self.ROOT + name)])
            self.assertEqual(len(violations), 1, name)
            self.assertIn("raw run data stays out of git", violations[0])

    def test_a_long_summary_fails(self):
        limit = check_rules.MEASUREMENT_LINE_LIMIT
        self.assertEqual(self.violations([("A", self.ROOT + "run/README.md")], limit), [])
        violations = self.violations([("A", self.ROOT + "run/README.md")], limit + 1)
        self.assertEqual(len(violations), 1)
        self.assertIn(f"{limit + 1} lines", violations[0])

    def test_summaries_and_other_folders_pass(self):
        changes = [("A", self.ROOT + "run/README.md"), ("A", self.ROOT + "notes.md"),
                   ("M", self.ROOT + "old_run/README.md"), ("D", self.ROOT + "old_run/runs.json"),
                   ("A", "tools/benchmark/evidence/run/runs.json"),
                   ("A", "wiki/drafts/measurements_notes.json")]
        self.assertEqual(self.violations(changes, 10), [])

    def test_editing_an_existing_long_summary_passes(self):
        self.assertEqual(self.violations([("M", self.ROOT + "run/README.md")], 1278), [])

    def test_diff_check_reads_the_pull_request_from_git(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            environment = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t",
                               GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@t")

            def git(*arguments):
                return subprocess.run(["git", "-C", directory, *arguments], check=True,
                                      capture_output=True, text=True, env=environment).stdout.strip()

            def write(path, lines):
                (root / path).parent.mkdir(parents=True, exist_ok=True)
                (root / path).write_text("line\n" * lines)

            git("init", "-q")
            write(self.ROOT + "old/README.md", 5)
            git("add", "-A")
            git("commit", "-q", "-m", "base")
            base = git("rev-parse", "HEAD")
            write(self.ROOT + "old/README.md", 6)
            write(self.ROOT + "new/README.md", check_rules.MEASUREMENT_LINE_LIMIT)
            write(self.ROOT + "new/long.md", check_rules.MEASUREMENT_LINE_LIMIT + 1)
            write(self.ROOT + "new/runs.json", 3)
            write("tools/elsewhere/runs.json", 3)
            git("add", "-A")
            git("commit", "-q", "-m", "change")
            violations = check_rules.diff_checks(base, root)
            self.assertEqual(len(violations), 2, violations)
            self.assertTrue(any("new/long.md" in line for line in violations))
            self.assertTrue(any("new/runs.json" in line for line in violations))


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


class DanglingTaskTests(unittest.TestCase):
    BOARD = ("## Section\n\n"
             "- **NESTED-MACHINE-UNIFICATION.** Implement nested machines.\n"
             "  - **NOT-AN-ITEM:** a sub-bullet label.\n\n"
             "- **NORMALIZED-ABI-LOWERING.** (Also covers **`OPAQUE-BY-VALUE-BOUNDARY-ABI`**\n"
             "  and **`FOREIGN-RETAINED-ARGUMENT-BACKING`**.) Complete foreign calls.\n"
             "  Later prose naming **EXPLICIT-ENTRY-STORAGE** is not an item.\n")
    FACT = "samples/cli/facts/row/README.md"

    def setUp(self):
        self.known = board_task_ids([self.BOARD])

    def test_items_and_covered_ids_are_on_the_board(self):
        self.assertEqual(self.known, {"NESTED-MACHINE-UNIFICATION", "NORMALIZED-ABI-LOWERING",
                                      "OPAQUE-BY-VALUE-BOUNDARY-ABI",
                                      "FOREIGN-RETAINED-ARGUMENT-BACKING"})

    def test_a_closed_item_cited_by_the_spec_fails(self):
        documents = {"wiki/spec/build/entry_roots.md":
                     "Remaining code is migration debt under `EXPLICIT-ENTRY-STORAGE`.\n"
                     "See [NESTED-MACHINE-UNIFICATION](../../../TASKS.md#x) and "
                     "**OPAQUE-BY-VALUE-BOUNDARY-ABI**.\n"}
        dangling = dangling_task_ids(documents, self.known)
        self.assertEqual(dangling, {("wiki/spec/build/entry_roots.md", "EXPLICIT-ENTRY-STORAGE")})
        violations = dangling_task_violations(dangling, set())
        self.assertEqual(len(violations), 1)
        self.assertIn("cites EXPLICIT-ENTRY-STORAGE, which is on no board", violations[0])

    def test_other_upper_case_words_are_not_task_ids(self):
        documents = {"AGENTS.md": "SHA-256 of UTF-8 and UTF-16 text per IEEE-2019, R1-R9, "
                                  "`BOUNDARY-REPORT.md` and docs/X86-64."}
        self.assertEqual(dangling_task_ids(documents, self.known), set())

    def test_only_fact_readmes_may_be_baselined(self):
        dangling = {(self.FACT, "CONSERVATION-CONTRACT"), ("AGENTS.md", "CORE-INTEGER-FLOAT-APIS")}
        baseline = read_dangling_baseline(write_dangling_baseline(dangling))
        self.assertEqual(baseline, {(self.FACT, "CONSERVATION-CONTRACT")})
        violations = dangling_task_violations(dangling, dangling)
        self.assertEqual(len(violations), 1)
        self.assertIn("AGENTS.md: cites CORE-INTEGER-FLOAT-APIS", violations[0])

    def test_a_new_fact_citation_fails(self):
        dangling = {(self.FACT, "CONSERVATION-CONTRACT"), (self.FACT, "BOUNDARY-ISSUANCE")}
        violations = dangling_task_violations(dangling, {(self.FACT, "CONSERVATION-CONTRACT")})
        self.assertEqual(len(violations), 1)
        self.assertIn("cites BOUNDARY-ISSUANCE", violations[0])

    def test_a_repointed_citation_lowers_the_baseline(self):
        violations = dangling_task_violations(set(), {(self.FACT, "CONSERVATION-CONTRACT")})
        self.assertEqual(len(violations), 1)
        self.assertIn("--update-dangling-task-ids", violations[0])

    def test_deleting_a_cited_board_item_fails(self):
        root = check_rules.ROOT
        boards = [(root / board).read_text() for board in check_rules.TASK_BOARDS]
        documents = citing_documents(root)
        baseline = read_dangling_baseline((root / check_rules.DANGLING_BASELINE).read_text())
        known = board_task_ids(boards)
        self.assertEqual(dangling_task_violations(dangling_task_ids(documents, known), baseline), [])
        known.discard("NESTED-MACHINE-UNIFICATION")
        violations = dangling_task_violations(dangling_task_ids(documents, known), baseline)
        self.assertIn("AGENTS.md: cites NESTED-MACHINE-UNIFICATION", "\n".join(violations))



if __name__ == "__main__":
    unittest.main()
