"""Runtime Value cohort refuses missing twins and source-only runtime claims."""
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ci"))
import compiler_runtime_value_generics as cohort


class RuntimeValueCohortTests(unittest.TestCase):
    def prepare(self, root, *, twin=True, authored_exit=True):
        prefix = "runtime_index_application_through_local_"
        positive = root / "tests/omega/pass/facts" / (prefix + "exit")
        positive.mkdir(parents=True)
        positive.joinpath("main.omg").write_text(
            ("// Expected exit: 0\n" if authored_exit else "") + "machine main() {}\n")
        positive.joinpath("build.omg").write_text("authored entry\n")
        if twin:
            negative = root / "tests/omega/fail/facts" / (prefix + "rejected")
            negative.mkdir(parents=True)
            negative.joinpath("main.omg").write_text("invalid source\n")
            negative.joinpath("expected.txt").write_text("intended rejection\n")
        library = root / "tests/omega/pass/generics/library_control"
        library.mkdir(parents=True)
        library.joinpath("main.omg").write_text("data Value {}\n")

    def discover(self, root, runtime_named=()):
        with patch.object(cohort, "ROWS", ("runtime_index_application",)), \
             patch.object(cohort, "PLACES", ("local",)), \
             patch.object(cohort, "NAMED", ()), \
             patch.object(cohort, "RUNTIME_NAMED", runtime_named), \
             patch.object(cohort, "GENERIC_PREFIXES", ("library_",)):
            return cohort.fixtures(root)

    def test_source_only_control_never_counts_as_runtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.prepare(root)
            runtime, rejected, checking_only = self.discover(root)
            self.assertEqual(len(runtime), 1)
            self.assertEqual(len(rejected), 1)
            self.assertEqual(checking_only, ("pass/generics/library_control",))

    def test_required_named_runtime_witness_cannot_become_source_only(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.prepare(root)
            with self.assertRaisesRegex(ValueError, "required runtime witness lacks"):
                self.discover(root, runtime_named=("library_control",))

    def test_missing_twin_refuses_before_build(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.prepare(root, twin=False)
            with self.assertRaisesRegex(ValueError, "place/twin is missing"):
                self.discover(root)

    def test_fact_without_runtime_oracle_refuses(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.prepare(root, authored_exit=False)
            with self.assertRaisesRegex(ValueError, "lacks native/Terminal"):
                self.discover(root)


if __name__ == "__main__":
    unittest.main()
