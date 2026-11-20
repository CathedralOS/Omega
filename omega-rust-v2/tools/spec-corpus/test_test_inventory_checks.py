import unittest

from test_inventory import compare, list_command

HEADER = "     Running tests\\stage.rs (test.exe)\n"
EMPTY = "   Doc-tests omega_lexer\n0 tests, 0 benchmarks\n"


class TestInventory(unittest.TestCase):
    def compare(self, names, results, suffix=EMPTY):
        listing = HEADER + "".join(name + ": test\n" for name in names) + f"{len(names)} tests, 0 benchmarks\n" + suffix
        return compare(listing, HEADER + EMPTY, [{"target": "tests/stage.rs", "test": name, "status": status} for name, status in results])

    def test_same_number_can_hide_a_missing_and_an_unexpected_test(self):
        row = self.compare(["first", "second"], [("first", "ok"), ("typo", "ok")])
        self.assertEqual((row["listed"], row["reported"]), (2, 2))
        self.assertEqual(row["issues"], ["tests/stage.rs::second: listed 1, reported 0", "tests/stage.rs::typo: listed 0, reported 1"])

    def test_duplicates_and_ignored_tests_cannot_verify(self):
        row = self.compare(["first", "first"], [("first", "ok"), ("first", "ignored")])
        self.assertEqual(row["issues"], ["tests/stage.rs::first: ignored", "tests/stage.rs::first: listed 2, reported 2"])

    def test_failed_tests_have_run_and_should_panic_suffix_belongs_to_the_harness(self):
        self.assertEqual(self.compare(["a", "b"], [("a - should panic", "ok"), ("b", "FAILED")])["issues"], [])

    def test_missing_empty_target_and_truncated_list(self):
        self.assertEqual(self.compare([], [], suffix="")["issues"], ["doctests: executed target absent from listing"])
        result = compare(HEADER + "x: test\n" + EMPTY, HEADER, [])
        self.assertIn("tests/stage.rs: missing list total", result["issues"])
        self.assertIn("doctests: listed target absent from execution", result["issues"])
        self.assertIn("tests/stage.rs: named entries disagree with '0 tests, 0 benchmarks'", compare(HEADER + "x: test\n0 tests, 0 benchmarks\n", HEADER, [])["issues"])

    def test_before_l5_cli_keeps_its_own_selection(self):
        args = ["mbx", "test", "-p", "omega", "--test", "frontend_diagnostics", "--", "--show-output", "--exact", "--skip", "lower"]
        self.assertEqual(list_command(args), [*args[:7], "--list", "--exact", "--skip", "lower"])
