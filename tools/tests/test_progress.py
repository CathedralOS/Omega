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
