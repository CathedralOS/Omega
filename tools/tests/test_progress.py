#!/usr/bin/env python3
"""Guard the progress measurement: the spec-section mapping must name every
current `## ` heading under wiki/spec/language exactly once, every group it
cites must exist in the corpus, and the tool's roster reading must agree with
the umbrella suite's own constants.

Runs locally only; standard library only:

    python3 tools/tests/test_progress.py -v
"""

import importlib.util
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools" / "progress.py"


def load_tool():
    specification = importlib.util.spec_from_file_location("progress_under_test", TOOL)
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


progress = load_tool()


class SpecMapping(unittest.TestCase):
    def rows(self):
        rows = {}
        for line in progress.SPEC_GROUPS.read_text(encoding="utf-8").splitlines():
            if not line.strip() or line.startswith("#") or line.startswith("spec_file\t"):
                continue
            parts = line.split("\t")
            self.assertGreaterEqual(len(parts), 5, f"short row: {line[:60]}")
            rows[(parts[0], parts[1])] = parts
        return rows

    def test_every_spec_section_is_mapped_exactly_once(self):
        sections = progress.spec_sections()
        rows = self.rows()
        self.assertEqual(len(sections), len(set(sections)), "duplicate spec headings")
        missing = [s for s in sections if s not in rows]
        extra = [r for r in rows if r not in set(sections)]
        self.assertEqual(missing, [], "spec sections with no mapping row")
        self.assertEqual(extra, [], "mapping rows naming no current spec section")

    def test_every_cited_group_exists(self):
        groups = {m.split("/", 1)[0] for m in progress.corpus_members("pass")}
        fail_groups = {m.split("/", 1)[0] for m in progress.corpus_members("fail")}
        for (spec, heading), parts in self.rows().items():
            for name in parts[3].split(","):
                name = name.strip()
                if name and name != "NONE":
                    self.assertIn(name, groups, f"{spec}: {heading} cites pass group {name}")
            for name in parts[4].split(","):
                name = name.strip()
                if name and name != "NONE":
                    self.assertIn(name, fail_groups, f"{spec}: {heading} cites fail group {name}")

    def test_class_vocabulary_is_closed(self):
        for (spec, heading), parts in self.rows().items():
            self.assertIn(parts[2], {"core", "typical", "advanced", "meta"}, f"{spec}: {heading}")


class Rosters(unittest.TestCase):
    def test_umbrella_tiers_match_the_suite_constants(self):
        tiers = progress.umbrella_tiers()
        text = (progress.COMPILER_TESTS / "canary_suite.rs").read_text(encoding="utf-8")
        for const in ("CHECKED_ONLY_PASS_CANARIES", "ACTIVE_PASS_CANARIES"):
            block = progress.const_block(text, const)
            self.assertTrue(block, f"{const} not found")
            named = set(progress.FIXTURE.findall(block))
            self.assertTrue(named, f"{const} is empty")
            self.assertTrue(named <= set(tiers), f"{const} members missing from tier map")

    def test_dark_members_are_a_small_minority(self):
        corpus = progress.corpus_report()
        self.assertGreater(corpus["pass"], 1000)
        self.assertLess(corpus["dark"], corpus["pass"] // 10, "over a tenth of the corpus runs nowhere")


class Surfaces(unittest.TestCase):
    def test_every_surface_pattern_compiles_and_structural_is_a_subset(self):
        import re

        for name, pattern in progress.SURFACES.items():
            re.compile(pattern)
        self.assertTrue(progress.STRUCTURAL <= set(progress.SURFACES))


if __name__ == "__main__":
    unittest.main()


class CoverageLine(unittest.TestCase):
    """The umbrella's elision counts are read from its coverage line, so the
    headline can separate fixtures this run never compiled from passes."""

    def test_elided_counts_are_read_from_the_coverage_line(self):
        import tempfile
        tool = load_tool()
        text = (
            "pass-canary coverage: selected-active=10 rooted-exact-native-elided=2 "
            "direct-exact-native-elided=3 active-compiled=5 cross-target-elided=1 "
            "cross-target-compiled=4\n"
            "1 pass canary(ies) failed to compile\n"
            "test result: FAILED. 0 passed; 1 failed\n"
        )
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "pass.log"
            log.write_text(text, encoding="utf-8")
            report = tool.parse_pass_log(log, {})
        self.assertEqual(report["coverage"]["direct-exact-native-elided"], 3)
        elided = sum(v for k, v in report["coverage"].items() if k.endswith("-elided"))
        self.assertEqual(elided, 6)

    def test_a_log_without_the_line_reports_no_coverage(self):
        import tempfile
        tool = load_tool()
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "pass.log"
            log.write_text("test result: ok. 1 passed\n", encoding="utf-8")
            report = tool.parse_pass_log(log, {})
        self.assertEqual(report["coverage"], {})


class OwnerVerdicts(unittest.TestCase):
    """Elided fixtures take the verdict of their dedicated owner test."""

    def test_owner_verdicts_join_the_index_to_the_suite_log(self):
        import tempfile
        tool = load_tool()
        index = (
            "rooted\tdata/a\t\ttopic::a_canary_runs\t70\n"
            "rooted\tdata/b\t\ttopic::b_canary_runs\t70\n"
            "direct\tdata/c\t\ttopic::c_canary_runs\t0\n"
            "cross-target\tdata/d\tlinux_x86_64\ttopic::d_compiles\t\n"
        )
        log = (
            "test topic::a_canary_runs ... ok\n"
            "test topic::b_canary_runs ... FAILED\n"
            "test topic::d_compiles ... ok\n"
        )
        with tempfile.TemporaryDirectory() as directory:
            index_path = Path(directory) / "owners.tsv"
            index_path.write_text(index, encoding="utf-8")
            log_path = Path(directory) / "suite.log"
            log_path.write_text(log, encoding="utf-8")
            outcome = tool.owner_outcomes(tool.parse_owner_index(index_path), tool.parse_suite_log(log_path))
        self.assertEqual(outcome["passing"], ["data/a"])
        self.assertEqual(outcome["failing"], ["data/b"])
        self.assertEqual(outcome["no_verdict"], ["data/c", "data/d"])

    def test_a_failing_owner_counts_against_the_fixture_tier(self):
        tool = load_tool()
        pass_outcome = {"failed_members": {}, "per_tier": {"active": {"members": 2, "failed": 0}}}
        tool.merge_owner_verdicts(pass_outcome, {"passing": ["x/p"], "failing": ["x/f"], "no_verdict": []}, {"x/f": "active", "x/p": "active"})
        self.assertEqual(pass_outcome["failed_members"], {"x/f": "dedicated-owner"})
        self.assertEqual(pass_outcome["per_tier"]["active"]["failed"], 1)
        self.assertEqual(pass_outcome["owners"]["passing"], 1)
        self.assertEqual(pass_outcome["owners"]["failing"], 1)
        self.assertEqual(pass_outcome["owners"]["runs"], {"x/p"})


class VerifiedLevels(unittest.TestCase):
    """Fixtures and sections are reported at the strongest predicate a run
    verified: runs, compiles, checks, fails or unmeasured."""

    def test_failure_blocks_name_the_stage_and_family(self):
        import tempfile
        tool = load_tool()
        text = (
            "---- topic::a_canary_runs stdout ----\n"
            "thread 'topic::a_canary_runs' panicked at x.rs:1:1:\n"
            "a canary should compile: [Diagnostic { severity: Error, message: \"selected compiler intrinsic `x` has no closed native catalog identity\", source_span: None }]\n"
            "\n"
            "---- topic::b_canary_runs stdout ----\n"
            "thread 'topic::b_canary_runs' panicked at x.rs:2:2:\n"
            "b canary should exit 70: got 1\n"
        )
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "suite.log"
            log.write_text(text, encoding="utf-8")
            failures = tool.parse_suite_failures(log)
        self.assertEqual(failures["topic::a_canary_runs"]["stage"], "compile")
        self.assertEqual(failures["topic::a_canary_runs"]["family"], "selected compiler intrinsic `_` has no closed native catalog identity")
        self.assertEqual(failures["topic::b_canary_runs"]["stage"], "run")

    def test_levels_take_the_strongest_verified_predicate(self):
        tool = load_tool()
        members = ["g/runs", "g/compiles", "g/checks", "g/fails", "g/elided", "g/dark"]
        original = tool.corpus_members
        tool.corpus_members = lambda kind: members
        try:
            report = {
                "corpus": {"tier_of": {"g/runs": "active", "g/compiles": "active", "g/checks": "checked_only", "g/fails": "active", "g/elided": "active"}},
                "outcomes": {"pass": {"failed_members": {"g/fails": "active"}, "owners": {"runs": {"g/runs"}, "unjudged": {"g/elided"}}}},
                "spec": {"rows": [{"spec_file": "s", "heading": "h", "class": "core", "groups": ["g"]}]},
            }
            levels = tool.fixture_levels(report)
            self.assertEqual(levels, {"g/runs": "runs", "g/compiles": "compiles", "g/checks": "checks", "g/fails": "fails", "g/elided": "unmeasured", "g/dark": "unmeasured"})
            sections = tool.section_levels(report, levels)
            self.assertEqual(sections["rows"][0]["best"], "runs")
            self.assertEqual(sections["tally"], {"core": {"runs": 1}})
        finally:
            tool.corpus_members = original

