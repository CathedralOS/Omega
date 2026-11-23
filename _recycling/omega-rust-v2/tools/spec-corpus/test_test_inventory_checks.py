import unittest

from test_inventory import abnormal_exits, compare, launch_failures, list_command

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

    def test_cargo_never_executed(self):
        reason = "An Application Control policy has blocked this file. (os error 4551)"
        diagnostic = ("error: test failed, to rerun pass `-p omega --test stage`\n\nCaused by:\n"
                      "  could not execute process `test.exe --include-ignored` (never executed)\n\n"
                      f"Caused by:\n  {reason}\n")
        log = HEADER + diagnostic + EMPTY
        for action in ("test", "list"):
            self.assertEqual(launch_failures(log, action, 101),
                             [{"action": action, "target": "tests/stage.rs", "reason": reason}])
            self.assertEqual(launch_failures(log, action, 0), [])
        captured = HEADER + "\nrunning 1 test\ntest example ... FAILED\n\nfailures:\n\n---- example stdout ----\n"
        for unrelated in (reason, diagnostic, HEADER + reason, captured + log,
                          log.replace("never executed", "exit code: 1")):
            self.assertEqual(launch_failures(unrelated, "test", 101), [])

    def test_rustdoc_launch_diagnostic_belongs_to_the_named_failed_example(self):
        name = r"src\entry_points.rs - parsing::entry_points::parse_fragment (line 52)"
        reason = "An Application Control policy has blocked this file. (os error 4551)"
        log = (f"   Doc-tests omega_parser\n\nrunning 2 tests\ntest {name} ... FAILED\ntest other ... ok\n\n"
               f"failures:\n\n---- {name} stdout ----\nCouldn't run the test: {reason}\n\n"
               f"failures:\n    {name}\n\ntest result: FAILED. 1 passed; 1 failed\n\n"
               "error: doctest failed, to rerun pass `-p omega-parser --doc`\n")
        self.assertEqual(launch_failures(log, "test", 101),
                         [{"action": "test", "target": "doctests", "test": name, "reason": reason}])
        for unrelated in (log.replace("   Doc-tests omega_parser", HEADER.rstrip()),
                          log.replace(f"---- {name}", "---- other"),
                          log.replace("Couldn't run the test:", "thread panicked: Couldn't run the test:"),
                          log.replace(reason, reason + "\nthread panicked")):
            self.assertEqual(launch_failures(unrelated, "test", 101), [])

    def test_an_assertion_in_one_target_does_not_hide_another_targets_process_failure(self):
        earlier = HEADER + "running 1 test\ntest first ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored\n"
        other = HEADER.replace("stage.rs", "second.rs") + "running 1 test\ntest second ... ok\n"
        summary = "test result: ok. 1 passed; 0 failed; 0 ignored\n"
        crash = ("error: test failed, to rerun pass `--test second`\n\nCaused by:\n"
                 "  process didn't exit successfully: `second.exe` (exit code: 3)\n")
        for end in ("", summary):
            self.assertEqual(abnormal_exits(earlier + other + end + crash, 101),
                             ["tests/second.rs: harness exited abnormally (exit code: 3)"])
        printed = other + "successes:\n\n---- second stdout ----\n" + crash + summary
        self.assertEqual(abnormal_exits(earlier + printed, 101), [])
        self.assertEqual(abnormal_exits(earlier + other + summary + crash, 0), [])

    def test_before_l5_cli_keeps_its_own_selection(self):
        args = ["mbx", "test", "-p", "omega", "--test", "frontend_diagnostics", "--", "--show-output", "--exact", "--skip", "lower"]
        self.assertEqual(list_command(args), [*args[:7], "--list", "--exact", "--skip", "lower"])
