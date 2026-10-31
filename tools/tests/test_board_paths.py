#!/usr/bin/env python3
"""Repeated task headings are a failed board merge, not two outcomes.

    python3 tools/tests/test_board_paths.py -v
"""

from pathlib import Path
import unittest

import worktree_testkit as kit


BOARD_PATHS = Path(__file__).resolve().parents[1] / "board_paths.py"
REPOSITORY = Path(__file__).resolve().parents[2]


def load_board_paths():
    return kit.load_module(BOARD_PATHS, "board_paths_under_test")


class DuplicateHeadings(unittest.TestCase):
    def setUp(self):
        self.board = load_board_paths()

    def test_one_heading_is_not_repeated(self):
        text = "- **ONE-ITEM.** Do the work.\n  Acceptance: it runs.\n"
        self.assertEqual(self.board.duplicate_headings(text), [])

    def test_the_same_heading_twice_reports_both_lines(self):
        text = (
            "- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Edited copy.\n"
            "\n"
            "- **OTHER-ITEM.** Stays once.\n"
            "\n"
            "- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** Old copy.\n"
        )
        self.assertEqual(
            self.board.duplicate_headings(text),
            [("RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY", [1, 5])],
        )

    def test_an_alias_inside_the_item_is_not_a_second_heading(self):
        text = (
            "- **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY.** "
            "(also covers **DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION**)\n"
            "  - **Psi checking, contracts, and package review**\n"
            "    retaining the witnesses in `samples/cli/facts/dynamic_suspension`\n"
        )
        self.assertEqual(
            self.board.item_headings(text),
            [("RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY", 1)],
        )
        self.assertEqual(self.board.duplicate_headings(text), [])

    def test_the_shipped_boards_have_one_heading_per_item(self):
        for name in self.board.BOARDS:
            text = (REPOSITORY / name).read_text(encoding="utf-8")
            self.assertEqual(
                self.board.duplicate_headings(text),
                [],
                name,
            )


if __name__ == "__main__":
    unittest.main()
