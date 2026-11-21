import tempfile
import unittest
from pathlib import Path

from source_fingerprint import source_identity


class ReadinessSourceIdentityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        checkout = Path(self.temporary.name) / "checkout"
        self.first = checkout / "v2" / "target" / "first" / "v2"
        self.second = checkout / "v2" / "target" / "second" / "v2"
        for workspace in (self.first, self.second):
            self.write(workspace / "Cargo.toml", b"[workspace]\n")
            self.write(workspace / "contracts/target/src/lib.rs", b"one")
            self.write(workspace.parent / "wiki/spec/evaluation.md", b"rule\n")

    @staticmethod
    def write(path, contents):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(contents)

    def test_private_compilers_identified_by_actual_inputs(self):
        first = source_identity(self.first)
        self.assertEqual(first, source_identity(self.second))
        self.assertEqual(first["source_trees"]["v2"]["files"], 2)
        self.write(self.second / "contracts/target/src/lib.rs", b"two")
        changed = source_identity(self.second)
        self.assertNotEqual(first["fingerprint"], changed["fingerprint"])
        self.assertNotEqual(first["source_trees"]["v2"], changed["source_trees"]["v2"])
        self.assertEqual(first["source_trees"]["spec"], changed["source_trees"]["spec"])

    def test_adjacent_spec_names_and_raw_contents_are_inputs(self):
        baseline = source_identity(self.first)
        spec = self.first.parent / "wiki/spec"
        old = spec / "evaluation.md"
        old.write_bytes(b"rule\r\n")
        changed = source_identity(self.first)
        self.assertNotEqual(baseline["source_trees"]["spec"], changed["source_trees"]["spec"])
        self.assertEqual(baseline["source_trees"]["v2"], changed["source_trees"]["v2"])
        old.rename(spec / "domains.md")
        renamed = source_identity(self.first)
        self.assertNotEqual(changed["fingerprint"], renamed["fingerprint"])
        self.write(old, b"rule\n")
        self.assertEqual(source_identity(self.first)["source_trees"]["spec"]["files"], 2)
        (spec / "domains.md").unlink()
        self.assertEqual(baseline, source_identity(self.first))

    def test_generated_artifacts_and_nested_checkouts_do_not_change_the_measurement(self):
        baseline = source_identity(self.first)
        for path in ("target/layer-readiness/report.md", ".agents/worktree/nested/Cargo.toml",
                     "tools/__pycache__/module.pyc", ".git/config"):
            self.write(self.first / path, b"generated")
        self.assertEqual(baseline, source_identity(self.first))
        self.write(self.first / "layers/new/src/lib.rs", b"new")
        self.assertNotEqual(baseline["fingerprint"], source_identity(self.first)["fingerprint"])

    def test_missing_companion_spec_reports_the_missing_path(self):
        spec = self.first.parent / "wiki/spec"
        spec.rename(spec.with_name("absent"))
        with self.assertRaises(FileNotFoundError):
            source_identity(self.first)
