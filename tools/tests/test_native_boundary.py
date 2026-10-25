"""Seed forbidden dependencies, rather than only blessing today's graph."""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_native_boundary import NATIVE_ROOT, RUNTIME_ROOT, RUNTIME_OWNERS, dependency_violations


class NativeBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[2]
        self.metadata = {"workspace_root": str(self.root), "packages": []}
        self.native = self.package("native-production", NATIVE_ROOT)
        self.evidence = self.package(
            "effects", "omega-rust/omega/compilation/backend/plans/provider-evidence"
        )
        self.edge(self.native, self.evidence)

    def package(self, name, path):
        package = {
            "name": name,
            "manifest_path": str(self.root / path / "Cargo.toml"),
            "dependencies": [],
        }
        self.metadata["packages"].append(package)
        return package

    def edge(self, source, destination, **properties):
        source["dependencies"].append({
            "name": destination["name"],
            "path": str(Path(destination["manifest_path"]).parent),
            "kind": None,
            **properties,
        })

    def test_terminal_contract_is_allowed(self):
        terminal = self.package(
            "terminal-psi", "omega-rust/psi/compilation/representations/terminal-psi"
        )
        self.edge(self.evidence, terminal)
        self.assertEqual(dependency_violations(self.metadata), [])

    def test_source_tree_through_backend_evidence_is_rejected(self):
        typed = self.package(
            "typed-trees", "omega-rust/psi/compilation/representations/typed-trees"
        )
        self.edge(self.evidence, typed, rename="trees")
        violations = dependency_violations(self.metadata)
        self.assertEqual(len(violations), 1)
        self.assertIn("native-production -> effects -> typed-trees", violations[0])

    def test_optional_foreign_target_build_dependency_is_checked(self):
        interpreter = self.package(
            "terminal-interpreter", "omega-rust/psi/interpreter/terminal-interpreter"
        )
        self.edge(self.evidence, interpreter, kind="build", optional=True,
                  target='cfg(target_os = "other")')
        self.assertEqual(len(dependency_violations(self.metadata)), 1)

    def test_dev_dependencies_are_not_production_edges(self):
        compiler = self.package("compiler", "omega-rust/omega/compilation/driver")
        self.edge(self.native, compiler, kind="dev")
        self.assertEqual(dependency_violations(self.metadata), [])
        self.edge(self.native, compiler)
        self.assertEqual(len(dependency_violations(self.metadata)), 1)

    def test_neighbor_with_terminal_prefix_is_not_terminal(self):
        counterfeit = self.package(
            "counterfeit", "omega-rust/psi/compilation/representations/terminal-psi-source"
        )
        self.edge(self.native, counterfeit)
        self.assertEqual(len(dependency_violations(self.metadata)), 1)

    def test_missing_path_member_is_not_silently_skipped(self):
        self.native["dependencies"].append({"path": str(self.root / "missing")})
        with self.assertRaisesRegex(ValueError, "not a workspace member"):
            dependency_violations(self.metadata)

    def test_runtime_uses_engine_but_cannot_reach_source_products(self):
        runtime = self.package("terminal-runtime", RUNTIME_ROOT)
        engine = self.package(
            "terminal-interpreter", "omega-rust/psi/interpreter/terminal-interpreter"
        )
        self.edge(runtime, engine)
        self.assertEqual(dependency_violations(self.metadata, RUNTIME_ROOT, RUNTIME_OWNERS), [])
        products = self.package("compilation-products", "omega-rust/omega/compilation/products")
        self.edge(engine, products)
        failures = dependency_violations(self.metadata, RUNTIME_ROOT, RUNTIME_OWNERS)
        self.assertEqual(len(failures), 1)
        self.assertIn("terminal-runtime -> terminal-interpreter -> compilation-products", failures[0])

    def test_runtime_cannot_call_the_native_compiler(self):
        runtime = self.package("terminal-runtime", RUNTIME_ROOT)
        self.edge(runtime, self.native)
        self.assertEqual(len(dependency_violations(self.metadata, RUNTIME_ROOT, RUNTIME_OWNERS)), 1)


if __name__ == "__main__":
    unittest.main()
