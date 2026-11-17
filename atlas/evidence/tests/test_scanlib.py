"""Regressions for source selection and the small set of imports Atlas observes."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))
from scanlib import Workspace, mask_non_code, references, scan, strip_cfg_test


class ScannerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.packages = []

    def package(self, name, directory, source, dependencies=()):
        path = self.root / directory
        (path / "src").mkdir(parents=True)
        (path / "src/lib.rs").write_text(source)
        package = {"name": name, "id": name, "manifest_path": str(path / "Cargo.toml"),
                   "dependencies": list(dependencies), "targets": [{"name": name.replace("-", "_"), "kind": ["lib"]}]}
        self.packages.append(package)
        return {"name": name, "path": str(path), "kind": None, "rename": None}

    def workspace(self):
        subprocess.run(["git", "add", "--all"], cwd=self.root, check=True)
        metadata = {"workspace_root": str(self.root), "workspace_members": [p["id"] for p in self.packages],
                    "packages": self.packages, "target_directory": str(self.root / "target")}
        return Workspace(metadata)

    def test_semantic_target_is_source_but_cargo_target_is_output(self):
        dep = self.package("language-core", "omega-rust/psi/foundation/language-core", "// definition\n")
        directory = "omega-rust/omega/compilation/backend/target"
        self.package("target", directory, "use language_core::float_operations::CANONICAL_FLOAT_OPERATIONS;\n", [dep])
        # A tracked output fixture proves selection uses the Cargo output path,
        # rather than either the word 'target' or Git tracking alone.
        self.package("build-output", "target/generated", "use language_core::Bogus;\n", [dep])
        workspace = self.workspace()
        result = scan(workspace, "foundation")
        self.assertEqual(result["usage"]["language-core"], {"float_operations": ["target"]})
        self.assertEqual(workspace.line_counts()["target"]["all_rs_lines"], 1)
        self.assertEqual(workspace.files["build-output"], [])
        self.assertNotIn("target", result["unobserved_dependencies"].get("language-core", []))

    def test_renamed_dependency_nested_group_glob_and_bare_import(self):
        dep = self.package("language-core", "omega-rust/psi/foundation/language-core", "")
        dep["rename"] = "lang"
        self.package("caller", "caller", "use lang::{float_operations::{A, B as C}, *};\nuse lang as short;\nshort::D();\n", [dep])
        result = scan(self.workspace(), "foundation")
        self.assertEqual(set(result["usage"]["language-core"]), {"float_operations", "*", "<crate>", "D"})
        self.assertEqual(set(result["usage2"]["language-core"]), {"float_operations::A", "float_operations::B"})
        self.assertEqual(result["unobserved_dependencies"], {})

    def test_glob_is_observed_without_claiming_its_imported_items(self):
        self.assertEqual(references("use foundation :: *;", "foundation"), {("*",)})
        self.assertEqual(references("extern crate foundation as f; f::Item();", "foundation"), {("<crate>",), ("Item",)})

    def test_comments_literals_and_test_items_do_not_manufacture_users(self):
        code = '''// foundation::LineComment
/* outer /* foundation::NestedComment */ foundation::BlockComment */
const S: &str = "foundation::String";
const R: &str = r###"foundation::Raw"###;
#[cfg(test)]
mod tests { const S: &str = "}"; use foundation::TestOnly; }
#[cfg(test)] use foundation::TestImport;
use foundation::Production;
'''
        cleaned = strip_cfg_test(mask_non_code(code))
        self.assertEqual(references(cleaned, "foundation"), {("Production",)})
        self.assertEqual(cleaned.count("\n"), code.count("\n"))

    def test_counts_include_integration_tests_and_count_nested_crate_once(self):
        self.package("outer", "outer", "// first\n// final without newline")
        self.package("inner", "outer/derive", "// nested\n")
        tests = self.root / "outer/tests"
        tests.mkdir()
        (tests / "integration.rs").write_text("// test\n// test\n// test\n")
        workspace = self.workspace()
        counts = workspace.line_counts()
        self.assertEqual(counts["outer"]["all_rs_lines"], 5)
        self.assertEqual(counts["outer"]["src_rs_lines"], 2)
        self.assertEqual(counts["inner"]["all_rs_lines"], 1)
        self.assertEqual(len(workspace.production_files("outer")), 1)

    def test_backend_cli_selects_backend_without_environment_override(self):
        backend = self.package("machine", "omega-rust/omega/compilation/backend/machine", "")
        foundation = self.package("facts", "omega-rust/psi/foundation/facts", "")
        self.package("caller", "caller", "use machine::Code; use facts::Value;", [backend, foundation])
        workspace = self.workspace()
        metadata = {"workspace_root": str(self.root), "workspace_members": list(workspace.packages),
                    "packages": self.packages, "target_directory": str(self.root / "target")}
        meta_file, out_file = self.root / "meta.json", self.root / "out.json"
        meta_file.write_text(json.dumps(metadata))
        env = dict(os.environ, SCAN_DIR="/foundation/")
        subprocess.run([sys.executable, str(SCRIPTS / "modscan_be.py"), str(meta_file), str(out_file)], env=env, check=True)
        result = json.loads(out_file.read_text())
        self.assertEqual(result["scope"], "backend")
        self.assertEqual(result["usage"], {"machine": {"Code": ["caller"]}})


class RepositoryRegression(unittest.TestCase):
    def test_real_target_crate_names_all_six_reported_missing_dependencies(self):
        root = SCRIPTS.parents[2]
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"], cwd=root))
        workspace = Workspace(metadata)
        target_files = workspace.production_files("target")
        self.assertTrue(any(p.name == "float_realization.rs" for p in target_files))
        code = "\n".join(strip_cfg_test(mask_non_code(p.read_text())) for p in target_files)
        for dependency in ("compiler_storage", "diagnostics", "language_core", "layout_plans",
                           "optimization_core", "semantic_vocabulary"):
            with self.subTest(dependency=dependency):
                self.assertTrue(references(code, dependency))
        self.assertIn(("float_operations", "CANONICAL_FLOAT_OPERATIONS"), references(code, "language_core"))


if __name__ == "__main__":
    unittest.main()
