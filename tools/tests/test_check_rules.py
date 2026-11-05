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
    CACHE_STATE_NAMES, DENSE_ID_MAPS, RATCHETS, SHAPE_GATES, SOURCE_CONTEXT_READS,
    SOURCE_READS, STAGE06_ROOT, STATUS_PHRASES,
    board_task_ids, changed_golden_rows, citing_documents, count_source_reads,
    count_status_phrases, dangling_task_ids, dangling_task_violations,
    already_pinned_fact_violations, diagnostic_fragments, diff_hygiene_violations,
    expected_pin_violations, fact_row_violations,
    fixture_owner, golden_row_quality_violations, golden_rows, golden_violations, lanes,
    measurement_violations, nearest_rows, pinned_fixture_for_fact,
    program_shingles, ratchet_counts, ratchet_violations, read_baseline,
    read_dangling_baseline, subject_violations, provenance_violations, tasks_board_violations,
    write_dangling_baseline, write_ratchet,
)


def source_read_violations(counts, baseline):
    return ratchet_violations(SOURCE_READS, counts, baseline)


def status_phrase_violations(counts, baseline):
    return ratchet_violations(STATUS_PHRASES, counts, baseline)


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
        rewritten = read_baseline(write_ratchet(SOURCE_READS, {"a.rs": 2}))
        self.assertEqual(source_read_violations({"a.rs": 2}, rewritten), [])


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
        rewritten = read_baseline(write_ratchet(STATUS_PHRASES, {"omega-rust/pipeline.md": 1}))
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
            self.assertEqual(ratchet_counts(STATUS_PHRASES, root),
                             {"omega-rust/pipeline.md": 1, "wiki/spec/build/a.md": 1})


class Stage06RatchetTests(unittest.TestCase):
    """Psi 06 lowers Psi 05's plans; it neither re-gates their shapes nor
    reads the typed source trees to rebuild a fact Psi 05 did not publish."""

    UNIT = STAGE06_ROOT + "/src/unit/lowering.rs"
    TEST = STAGE06_ROOT + "/src/unit/lowering/tests.rs"

    def tree(self, files):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        for path, text in files.items():
            (root / path).parent.mkdir(parents=True, exist_ok=True)
            (root / path).write_text(text)
        return root

    def test_shape_gates_count(self):
        text = """
            return Err(unsupported(expression, "dyn copy"));
            Err(LoweringError::Unsupported { reason })
            ContentPartitionResultRewriteUnsupported => unpinned(site),
            let supported = plan.supported(); // a lowering, not a gate
        """
        self.assertEqual(SHAPE_GATES.count(text), 4)

    def test_source_context_reads_count(self):
        text = """
            use typed_trees::expression::ExpressionHandle;
            fn reaches(typed: &TypedTrees, frozen: &FrozenTypedTrees) {}
            let machine = checked.typed.machine_by_symbol(symbol);
            let identity = typed_trees::type_identity::TypeIdentity::of(plan);
            let typed_value = plan.typed_value; // a plan field, not the source trees
        """
        self.assertEqual(SOURCE_CONTEXT_READS.count(text), 4)

    def test_only_stage_05_counts(self):
        root = self.tree({
            self.UNIT: "Err(unsupported(site)); checked.typed.machines",
            "omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/a.rs":
                "Err(unsupported(site)); checked.typed.machines",
        })
        self.assertEqual(ratchet_counts(SHAPE_GATES, root), {self.UNIT: 1})
        self.assertEqual(ratchet_counts(SOURCE_CONTEXT_READS, root), {self.UNIT: 1})

    def test_a_planted_gate_fails(self):
        before = self.tree({self.UNIT: "Err(unsupported(site))"})
        baseline = read_baseline(write_ratchet(SHAPE_GATES, ratchet_counts(SHAPE_GATES, before)))
        after = self.tree({self.UNIT: "Err(unsupported(site)); Err(unpinned(other))",
                           self.TEST: "LoweringError::Unsupported"})
        violations = ratchet_violations(SHAPE_GATES, ratchet_counts(SHAPE_GATES, after), baseline)
        self.assertEqual(len(violations), 2)
        self.assertIn("lowering.rs: 2 Psi 06 shape gates, baseline 1", violations[0])
        self.assertIn("Psi 05 plan", violations[0])
        self.assertIn("tests.rs: 1 Psi 06 shape gates, baseline 0", violations[1])

    def test_a_planted_source_read_fails(self):
        violations = ratchet_violations(SOURCE_CONTEXT_READS, {self.UNIT: 3}, {self.UNIT: 2})
        self.assertEqual(len(violations), 1)
        self.assertIn("3 Psi 06 source-context reads, baseline 2", violations[0])
        self.assertIn("Public contracts", violations[0])

    def test_a_retired_site_lowers_the_baseline(self):
        for ratchet in (SHAPE_GATES, SOURCE_CONTEXT_READS):
            with self.subTest(ratchet=ratchet.noun):
                violations = ratchet_violations(ratchet, {self.UNIT: 1}, {self.UNIT: 2})
                self.assertIn(f"--{ratchet.flag}", violations[0])
                rewritten = read_baseline(write_ratchet(ratchet, {self.UNIT: 1}))
                self.assertEqual(ratchet_violations(ratchet, {self.UNIT: 1}, rewritten), [])


class DenseStateTests(unittest.TestCase):
    """Dense IDs index flat tables, and derived data is not cached."""

    tree = Stage06RatchetTests.tree

    def test_maps_keyed_by_dense_ids_count(self):
        text = """
            values: BTreeMap<ValueId, u32>,
            blocks: &mut std::collections::BTreeSet<semantic_vocabulary::BlockId>,
            symbols: FxHashMap<SymbolHandle, Row>,
            rows: HashMap<Handle<Row>, Row>,
            names: BTreeMap<String, ValueId>, // keyed by a name, not an ID
            pairs: BTreeMap<(ValueId, BlockId), u32>,
            // BTreeMap<PlaceId, u32> in a comment
        """
        self.assertEqual(DENSE_ID_MAPS.count(text), 4)

    def test_cache_state_counts(self):
        text = """
            struct Checker { shape_cache: Vec<u8>, summaries: StateMutationSummaryCache }
            fn check(memos: &mut CheckingMemos, memory: &Memory, remember: bool) {
                let mut cache: LayoutCache = LayoutCache::default();
                let digest_memo = 0;
                let index: OwnedIndex<Row> = KeyedRows::default();
            }
        """
        self.assertEqual(CACHE_STATE_NAMES.count(text), 7)

    def test_test_code_does_not_count(self):
        root = self.tree({
            "omega-rust/a/src/lib.rs": "struct A { values: BTreeMap<ValueId, u32> }\n"
                                       "#[cfg(test)]\nmod tests { fn f(cache: BTreeMap<ValueId, u32>) {} }\n",
            "omega-rust/a/src/tests.rs": "struct T { values: BTreeMap<ValueId, u32> }",
            "omega-rust/a/tests/t.rs": "struct T { values: BTreeMap<ValueId, u32> }",
        })
        self.assertEqual(ratchet_counts(DENSE_ID_MAPS, root), {"omega-rust/a/src/lib.rs": 1})
        self.assertEqual(ratchet_counts(CACHE_STATE_NAMES, root), {})

    def test_a_new_map_fails_and_a_removed_one_lowers_the_baseline(self):
        path = "omega-rust/a/src/lib.rs"
        violations = ratchet_violations(DENSE_ID_MAPS, {path: 3}, {path: 2})
        self.assertIn("3 maps keyed by dense IDs, baseline 2", violations[0])
        self.assertIn("IdTable", violations[0])
        violations = ratchet_violations(CACHE_STATE_NAMES, {path: 1}, {path: 2})
        self.assertIn("--update-cache-state", violations[0])


class RatchetBaselineTests(unittest.TestCase):
    def test_the_tree_matches_every_baseline(self):
        for ratchet in RATCHETS:
            with self.subTest(baseline=ratchet.baseline):
                baseline = read_baseline((check_rules.ROOT / ratchet.baseline).read_text())
                self.assertEqual(ratchet_violations(ratchet, ratchet_counts(ratchet), baseline), [])

    def test_update_flags_rewrite_their_baselines(self):
        for ratchet in RATCHETS:
            with self.subTest(flag=ratchet.flag):
                text = write_ratchet(ratchet, {"a.rs": 1})
                self.assertIn(f"--{ratchet.flag}", text)
                self.assertEqual(read_baseline(text), {"a.rs": 1})


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

    def test_dirty_wt_rejected_pass_and_unexpected_fail_rows_are_rejected(self):
        diff = (
            "--- a/tests/omega/corpus_outcomes.txt\n"
            "+++ b/tests/omega/corpus_outcomes.txt\n"
            "+pass/facts/good_row checked 10ms @abc123456789\n"
            "+fail/facts/good_fail rejected 10ms expected @abc123456789\n"
            "+pass/facts/dirty_row checked 10ms @abc123456789+wt\n"
            "+pass/facts/broken_pass rejected 10ms @abc123456789\n"
            "+fail/facts/wrong_fail rejected 10ms unexpected @abc123456789\n"
            "+fail/facts/lost_fail checked 10ms @abc123456789\n"
        )
        violations = golden_row_quality_violations(diff)
        self.assertEqual(len(violations), 4, violations)
        self.assertIn("dirty_row carries a dirty worktree stamp", violations[0])
        self.assertIn("broken_pass is a pass/run fixture recorded as rejected", violations[1])
        self.assertIn("wrong_fail is a fail fixture recorded as unexpected", violations[2])
        self.assertIn("lost_fail is a fail fixture recorded as checked", violations[3])

    def test_diff_hygiene_blocks_fact_additions_excessive_omg_new_corpus_and_wave_dumps(self):
        self.assertEqual(diff_hygiene_violations([
            ("A", "tests/omega/pass/borrows/focused_ok/main.omg"),
            ("A", "tests/omega/fail/borrows/focused_bad/main.omg"),
            ("M", "samples/cli/facts/length/local/main.omg"),
            ("D", "tools/swarm/waves/old.json"),
        ]), [])
        violations = diff_hygiene_violations([
            ("A", "samples/cli/facts/length/sub/main.omg"),
            ("A", "tests/omega/pass/facts/length_sub_exit/main.omg"),
            ("A", "tests/omega/corpus_extra.txt"),
            ("A", "tools/swarm/waves/w99.json"),
            *[("A", f"tests/omega/pass/borrows/f{i}/main.omg") for i in range(6)],
        ])
        self.assertEqual(len(violations), 5, violations)
        self.assertTrue(any("samples/cli/facts/length/sub/main.omg" in v for v in violations))
        self.assertTrue(any("tests/omega/pass/facts/length_sub_exit/main.omg" in v for v in violations))
        self.assertTrue(any("tests/omega/corpus_extra.txt" in v for v in violations))
        self.assertTrue(any("tools/swarm/waves/w99.json" in v for v in violations))
        self.assertTrue(any("PR adds 8 .omg files (limit is 6)" in v for v in violations))


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


class ChecklistRefusalTests(unittest.TestCase):
    """Sessions were scoring the checklist: rewriting a pinned diagnostic, or
    adding a samples/cli/facts program for a fact the corpus already pins."""

    OLD = "tests/omega/fail/facts/length_through_local_rejected/expected.txt"
    LENGTH = {"fail/facts/length_through_local_rejected",
              "pass/facts/length_through_local_exit"}
    FACTS = {"length", "may_write_codec", "may_write_codec_argument"}

    def test_rewriting_an_existing_pin_names_the_file_and_rule(self):
        changes = [("M", self.OLD)]
        violations = expected_pin_violations(
            changes,
            lambda path: "may overflow `u8`\n",
            lambda path: "may overflow `u64`\n")
        self.assertEqual(len(violations), 1)
        self.assertIn(self.OLD, violations[0])
        self.assertIn("existing-diagnostic-pin", violations[0])
        self.assertIn("dropped 'may overflow `u8`'", violations[0])
        self.assertIn("added 'may overflow `u64`'", violations[0])

    def test_whitespace_and_order_do_not_change_the_pin(self):
        changes = [("M", self.OLD)]
        self.assertEqual(expected_pin_violations(
            changes,
            lambda path: "alpha\n\nbeta\n",
            lambda path: "  beta  \nalpha\n"), [])
        self.assertEqual(diagnostic_fragments("alpha\n\nbeta\n"),
                         diagnostic_fragments("  beta  \nalpha\n"))

    def test_a_new_expected_file_and_other_oracles_are_allowed(self):
        changes = [
            ("A", "tests/omega/fail/text/fresh_refusal/expected.txt"),
            ("M", "tests/omega/fail/text/fresh_refusal/expected_error.txt"),
            ("M", "samples/cli/facts/length/local/expected.txt"),
            ("M", "tests/omega/pending/note.txt"),
        ]
        self.assertEqual(expected_pin_violations(
            changes, lambda path: "old\n", lambda path: "new\n"), [])

    def test_deleting_an_existing_pin_is_refused(self):
        violations = expected_pin_violations(
            [("D", self.OLD)], lambda path: "holds no value\n", lambda path: "")
        self.assertEqual(len(violations), 1)
        self.assertIn("existing-diagnostic-pin", violations[0])
        self.assertIn("removed the file", violations[0])

    def test_a_longer_fact_keeps_its_own_fixtures(self):
        fixtures = {"pass/facts/may_write_codec_argument_through_field_exit",
                    "pass/facts/may_write_codec_through_local_exit"}
        self.assertEqual(
            fixture_owner("may_write_codec_argument_through_field_exit", self.FACTS),
            "may_write_codec_argument")
        self.assertEqual(
            pinned_fixture_for_fact("may_write_codec", fixtures, self.FACTS),
            "pass/facts/may_write_codec_through_local_exit")
        self.assertEqual(
            pinned_fixture_for_fact("may_write_codec_argument", fixtures, self.FACTS),
            "pass/facts/may_write_codec_argument_through_field_exit")

    def test_a_new_program_for_a_pinned_fact_names_the_file_and_rule(self):
        path = "samples/cli/facts/length/field/main.omg"
        violations = already_pinned_fact_violations(
            [("A", path),
             ("M", "samples/cli/facts/length/local/main.omg"),
             ("A", "samples/cli/facts/brand_new/local/main.omg")],
            self.LENGTH, self.FACTS)
        self.assertEqual(len(violations), 1, violations)
        self.assertIn(path, violations[0])
        self.assertIn("already-pinned-fact", violations[0])
        self.assertIn("fail/facts/length_through_local_rejected", violations[0])

    def test_a_nested_fixture_directory_pins_its_fact_name(self):
        witness = pinned_fixture_for_fact(
            "length", {"pass/facts/length"}, {"length"})
        self.assertEqual(witness, "pass/facts/length")

    def test_diff_checks_wires_both_refusals(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            environment = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t",
                               GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@t")

            def git(*arguments):
                return subprocess.run(["git", "-C", directory, *arguments], check=True,
                                      capture_output=True, text=True, env=environment).stdout.strip()

            def write(path, text):
                (root / path).parent.mkdir(parents=True, exist_ok=True)
                (root / path).write_text(text)

            git("init", "-q")
            write("tests/omega/fail/facts/length_through_local_rejected/expected.txt",
                  "may overflow `u8`\n")
            write("tests/omega/fail/facts/length_through_local_rejected/main.omg",
                  "machine main() -> u64 { return 0u64; }\n")
            write("samples/cli/facts/length/local/main.omg",
                  "machine main() -> u64 { return 0u64; }\n")
            write("tests/omega/pass/facts/may_write_codec_argument_through_field_exit/main.omg",
                  "machine main() -> u64 { return 0u64; }\n")
            write("samples/cli/facts/may_write_codec/local/main.omg",
                  "machine main() -> u64 { return 0u64; }\n")
            write("samples/cli/facts/may_write_codec_argument/local/main.omg",
                  "machine main() -> u64 { return 0u64; }\n")
            git("add", "-A")
            git("commit", "-q", "-m", "base")
            base = git("rev-parse", "HEAD")
            write("tests/omega/fail/facts/length_through_local_rejected/expected.txt",
                  "may overflow `u64`\n")
            write("tests/omega/fail/text/fresh_refusal/expected.txt", "a new pin\n")
            write("samples/cli/facts/length/field/main.omg",
                  "machine main() -> u64 { return 1u64; }\n")
            write("samples/cli/facts/may_write_codec/field/main.omg",
                  "machine main() -> u64 { return 1u64; }\n")
            git("add", "-A")
            git("commit", "-q", "-m", "change")
            violations = check_rules.diff_checks(base, root)
            text = "\n".join(violations)
            pins = [line for line in violations if "existing-diagnostic-pin" in line]
            duplicates = [line for line in violations if "already-pinned-fact" in line]
            self.assertEqual(len(pins), 1, violations)
            self.assertIn("length_through_local_rejected/expected.txt", pins[0])
            self.assertEqual(duplicates, [
                line for line in duplicates
                if "samples/cli/facts/length/field/main.omg" in line], violations)
            self.assertEqual(len(duplicates), 1, violations)
            self.assertNotIn("fresh_refusal/expected.txt", text)
            self.assertNotIn("may_write_codec/field/main.omg", "\n".join(duplicates))


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


class ProvenanceTests(unittest.TestCase):
    def test_agent_line_with_session_passes(self):
        body = "Fixes the bound.\n\nAgent: Claude Code, https://claude.ai/code/session_01abc\n"
        self.assertEqual(provenance_violations(body), [])

    def test_agent_line_without_detail_passes(self):
        for name in ("Codex", "Cursor", "Human"):
            self.assertEqual(provenance_violations(f"Body.\nAgent: {name}\n"), [], name)

    def test_missing_or_unknown_agent_fails(self):
        for body in ("No provenance here.", "Implemented by Codex.",
                     "Agent: a helpful robot", "  Agent: Codex indented",
                     "Agents: Codex"):
            self.assertEqual(len(provenance_violations(body)), 1, body)


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


class TasksBoardTests(unittest.TestCase):
    BASE = (
        "# Tasks\n\n"
        "## Priority (human-set only)\n\n"
        "- **NESTED-MACHINE-UNIFICATION.** Replace `state` with nested machines.\n"
        "  - Resolve recursive lexical scopes and forward references.\n"
        "  - Complete cyclic private arrivals and normal-return obligations.\n"
        "  Acceptance: three-level nesting passes `omega run --both`.\n"
    )

    def test_the_current_tasks_board_is_clean(self):
        text = (check_rules.ROOT / check_rules.TASKS_BOARD).read_text(encoding="utf-8")
        self.assertEqual(tasks_board_violations(text, text), [])

    def test_deleting_a_completed_subbullet_passes(self):
        shrunk = self.BASE.replace(
            "  - Resolve recursive lexical scopes and forward references.\n", ""
        )
        self.assertEqual(tasks_board_violations(shrunk, self.BASE), [])

    def test_growing_lines_or_subbullets_fails(self):
        grown = self.BASE + "  - Defer another newly discovered edge case.\n"
        violations = tasks_board_violations(grown, self.BASE)
        self.assertTrue(any("grew from 8 to 9 lines" in v for v in violations), violations)
        self.assertTrue(any("sub-bullets grew from 2 to 3" in v for v in violations), violations)

    def test_wrapped_continuation_and_status_history_fail(self):
        bad = (
            "# Tasks\n\n"
            "## Priority (human-set only)\n\n"
            "- **NESTED-MACHINE-UNIFICATION.** Replace `state` with nested machines.\n"
            "  - The qualified-case row now preserves imported alias applications\n"
            "    across wrapped continuation lines that remain open.\n"
            "  Acceptance: three-level nesting passes `omega run --both`.\n"
        )
        violations = tasks_board_violations(bad)
        self.assertTrue(any("wrapped continuation" in v for v in violations), violations)
        self.assertTrue(any("now preserves" in v for v in violations), violations)
        self.assertTrue(any("remain open" in v for v in violations), violations)


if __name__ == "__main__":
    unittest.main()
