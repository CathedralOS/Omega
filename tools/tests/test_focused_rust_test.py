"""Exercise the focused driver without launching Rust tools."""

import contextlib
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import Mock, patch


SPEC = importlib.util.spec_from_file_location(
    "focused_rust_test", Path(__file__).resolve().parents[1] / "ci/focused_rust_test.py")
driver = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(driver)


class FocusedRustTest(unittest.TestCase):
    def invoke(self, arguments, output="", status=0, **selection):
        environment = {
            "TEST_PACKAGE": "compiler", "TEST_FILTER": "windows_canonicalize_receiver",
            "TEST_TARGET": "filesystem_receiver", "GITHUB_SHA": "exact-source",
        }
        environment.update(selection)
        process = Mock(stdout=io.StringIO(output))
        process.wait.return_value = status
        captured = io.StringIO()
        with patch.dict(driver.os.environ, environment), \
                patch.object(driver.subprocess, "run") as rustc, \
                patch.object(driver.subprocess, "Popen", return_value=process) as cargo, \
                contextlib.redirect_stdout(captured):
            try:
                driver.main(arguments)
            finally:
                self.rustc = rustc
                self.cargo = cargo
                self.output = captured.getvalue()

    def test_preparation_compiles_without_claiming_test_execution(self):
        self.invoke(["--prepare"], output="Finished test profile [optimized]\n")
        command = self.cargo.call_args.args[0]
        self.assertEqual(command[1:3], ["tools/build_slot.py", "--"])
        self.assertEqual(command[3:], [
            "mbx", "test", "--locked", "-p", "compiler", "--test",
            "filesystem_receiver", "--no-run", "windows_canonicalize_receiver",
        ])
        self.assertIn("no tests were executed", self.output)

    def test_execution_requires_a_passing_test_and_keeps_runtime_options(self):
        self.invoke([], output="test result: ok. 1 passed; 0 failed;\n")
        command = self.cargo.call_args.args[0]
        self.assertNotIn("--no-run", command)
        self.assertEqual(command[-4:], [
            "windows_canonicalize_receiver", "--", "--nocapture", "--test-threads=1",
        ])
        self.assertNotIn("no tests were executed", self.output)

    def test_library_preparation_selects_the_library_target(self):
        self.invoke(["--prepare"], TEST_TARGET="")
        command = self.cargo.call_args.args[0]
        self.assertIn("--lib", command)
        self.assertNotIn("--test", command)

    def test_compile_and_execution_failures_preserve_nonzero_status(self):
        for arguments in (["--prepare"], []):
            with self.subTest(arguments=arguments), self.assertRaises(SystemExit) as failure:
                self.invoke(arguments, output="tool failed\n", status=101)
            self.assertEqual(failure.exception.code, 101)
            self.assertNotIn("Compilation completed", self.output)

    def test_zero_passing_filter_is_refused(self):
        with self.assertRaisesRegex(SystemExit, "executed no passing tests"):
            self.invoke([], output="test result: ok. 0 passed; 0 failed;\n")

    def test_incomplete_selection_refuses_before_any_tool(self):
        for field in ("TEST_PACKAGE", "TEST_FILTER"):
            for arguments in (["--validate"], ["--prepare"]):
                with self.subTest(field=field, arguments=arguments), \
                        self.assertRaises(SystemExit):
                    self.invoke(arguments, **{field: ""})
                self.rustc.assert_not_called()
                self.cargo.assert_not_called()

    def test_selection_validation_does_not_compile_or_run(self):
        self.invoke(["--validate"])
        self.rustc.assert_not_called()
        self.cargo.assert_not_called()


if __name__ == "__main__":
    unittest.main()
