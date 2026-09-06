"""Coverage boundaries for test selection, including real Git index behavior."""

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "test_affected", Path(__file__).resolve().parents[1] / "test_affected.py")
affected = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(affected)


class SelectionTests(unittest.TestCase):
    def setUp(self):
        self.root = Path.cwd().resolve()
        packages = []
        for name, directory, dependencies in [
            ("app", "crates/app", ["core"]),
            ("core", "crates/app/core", []),
            ("unrelated", "crates/unrelated", []),
            ("terminal-verifier", "crates/verifier", []),
            ("terminal-codec", "crates/codec", []),
            ("codec-user", "crates/codec-user", ["terminal-codec"]),
        ]:
            packages.append({"id": name, "name": name,
                             "manifest_path": str(self.root / directory / "Cargo.toml"),
                             "dependencies": [{"name": dependency} for dependency in dependencies]})
        self.metadata = {"packages": packages, "workspace_members": [p["id"] for p in packages]}

    def select(self, *paths):
        return affected.selection(self.root, self.metadata, paths)

    def test_nested_owner_and_transitive_dependents_exclude_unrelated(self):
        expression, packages, reasons = self.select("crates/app/core/src/lib.rs")
        self.assertEqual(packages, ["app", "core"])
        self.assertEqual(expression, "rdeps(=app) | rdeps(=core)")
        self.assertFalse(reasons)

    def test_source_reader_edge_and_its_dependents(self):
        _, packages, _ = self.select("crates/verifier/src/new_proof.rs")
        self.assertEqual(packages, ["codec-user", "terminal-codec", "terminal-verifier"])

    def test_unknown_shared_manifest_and_non_source_inputs_are_full(self):
        for path in ["Cargo.lock", "crates/app/Cargo.toml", "crates/app/build.rs",
                     "source/library/core.omg", "new-crate/src/lib.rs",
                     "crates/app/tests/fixture.rs", "tests/fixtures/README.md",
                     "crates/app/src/fixture.txt"]:
            with self.subTest(path=path):
                expression, _, reasons = self.select(path)
                self.assertEqual(expression, "all()")
                self.assertTrue(reasons)

    def test_audited_docs_select_no_libraries_but_unknown_markdown_is_full(self):
        for path in ["README.md", "TASKS.md", "AGENTS.md", "CLAUDE.md",
                     "OWNER_QUESTIONS.md", "TASKS_BOOTSTRAP.md", "TASKS_OPTIMIZER.md",
                     "wiki/releases/optimizer_promotions/rule.md", "wiki/new.md"]:
            with self.subTest(path=path):
                self.assertEqual(self.select(path), ("none()", [], []))
        for path in ["fixtures/input.md", "wiki/input.omg", "wiki/config.toml",
                     "source/library/README.md", "crates/app/src/input.md", "new.md"]:
            with self.subTest(path=path):
                self.assertEqual(self.select(path)[0], "all()")

    def test_docs_do_not_hide_a_rust_change_or_unknown_input(self):
        self.assertEqual(self.select("README.md", "crates/app/core/src/lib.rs")[1],
                         ["app", "core"])
        self.assertEqual(self.select("README.md", "Cargo.lock")[0], "all()")

    def test_docs_plan_runs_architecture_and_exact_corpus_check_without_libraries(self):
        with patch.object(affected, "changed_paths", return_value=["README.md"]), \
                patch.object(affected, "output", return_value=json.dumps(
                    dict(self.metadata, workspace_root=str(self.root)))):
            plan = affected.make_plan(self.root, "mbx", "verified-base")
        self.assertEqual(plan["documentation_paths"], ["README.md"])
        self.assertEqual(len(plan["commands"]), 2)
        architecture, corpus = plan["commands"]
        self.assertIn("omega-architecture-test", architecture)
        self.assertIn("test(=" + affected.DOCUMENTATION_TEST + ")", corpus)
        self.assertEqual(corpus[corpus.index("--no-tests") + 1], "fail")
        self.assertFalse(any("--lib" in command for command in plan["commands"]))

    def test_no_change_selects_no_libraries(self):
        self.assertEqual(self.select(), ("none()", [], []))

    def test_invalid_package_name_fails_instead_of_injecting_filter(self):
        self.metadata["packages"][0]["name"] = "app) | all("
        with self.assertRaises(ValueError):
            self.select("crates/app/src/lib.rs")

    def test_architecture_failure_does_not_prevent_libraries_or_turn_green(self):
        with patch.object(affected.subprocess, "run", side_effect=[
            subprocess.CompletedProcess([], 100), subprocess.CompletedProcess([], 0),
        ]) as run:
            self.assertEqual(affected.run_commands(self.root, [["architecture"], ["libraries"]]), 100)
            self.assertEqual(run.call_count, 2)

    def test_library_filter_preserves_workspace_build_and_architecture_gate(self):
        with patch.object(affected, "changed_paths", return_value=["crates/app/core/src/lib.rs"]), \
                patch.object(affected, "output", return_value=json.dumps(
                    dict(self.metadata, workspace_root=str(self.root)))):
            plan = affected.make_plan(self.root, "mbx", "verified-base")
        architecture, libraries = plan["commands"]
        self.assertIn("omega-architecture-test", architecture)
        self.assertIn("--workspace", libraries)
        self.assertNotIn("-p", libraries)
        self.assertIn("--no-fail-fast", libraries)
        self.assertEqual(libraries[-2:], ["--no-tests", "pass"])

    def test_full_baseline_does_not_accept_an_unexpected_empty_suite(self):
        plan = affected.make_plan(self.root, "mbx", None, full=True)
        self.assertEqual(plan["filter"], "all()")
        self.assertNotIn("--no-tests", plan["commands"][-1])

    def test_reader_implementation_change_requires_full_recheck(self):
        expression, _, reasons = self.select(
            "omega-rust/psi/semantics/terminal-codec/src/trust_graph.rs")
        self.assertEqual(expression, "all()")
        self.assertTrue(reasons)


class GitChangesTests(unittest.TestCase):
    def test_staged_unstaged_deleted_untracked_and_both_rename_sides(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def git(*arguments):
                return subprocess.check_output(["git", *arguments], cwd=root).decode().strip()

            git("init", "-q")
            for filename in ["staged.rs", "unstaged.rs", "deleted.rs", "old name.rs"]:
                (root / filename).write_text("before", encoding="utf-8")
            git("add", ".")
            git("-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                "-c", "commit.gpgsign=false", "commit", "-qm", "base")
            base = git("rev-parse", "HEAD")
            (root / "staged.rs").write_text("staged", encoding="utf-8")
            git("add", "staged.rs")
            (root / "unstaged.rs").write_text("unstaged", encoding="utf-8")
            (root / "deleted.rs").unlink()
            git("mv", "old name.rs", "new name.rs")
            (root / "untracked.rs").write_text("new", encoding="utf-8")
            self.assertEqual(set(affected.changed_paths(root, base)), {
                "staged.rs", "unstaged.rs", "deleted.rs", "old name.rs", "new name.rs", "untracked.rs",
            })
            with self.assertRaises(subprocess.CalledProcessError):
                affected.changed_paths(root, "nonexistent-commit")


if __name__ == "__main__":
    unittest.main()
