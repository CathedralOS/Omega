"""Seed forbidden dependencies, rather than only blessing today's graph."""

from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_native_boundary import NATIVE_ROOT, RUNTIME_ROOT, RUNTIME_OWNERS, NATIVE_PROVIDER_RUNTIME_ROOT, NATIVE_PROVIDER_RUNTIME_OWNERS, dependency_violations
from check_native_boundary import PROVIDER_SELECTION_ROOT, PROVIDER_SELECTION_OWNERS
from check_native_boundary import TRUST_ROOT, TRUST_OWNERS, representation_spelling_violations, psi_omega_dependency_violations, realization_checker_entry_violations
from check_native_boundary import FORBIDDEN_DEPENDENCIES, PSI_FACADE, TERMINAL_PRODUCTION, forbidden_dependency_violations
from check_native_boundary import FIREWALL_ALLOWANCES, OMEGA_STAGES, Allowance, stale_allowance_violations
import json
import subprocess


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

    def test_provider_selection_cannot_reopen_source_or_drive_native_compilation(self):
        selection = self.package("provider-selection", PROVIDER_SELECTION_ROOT)
        self.edge(selection, self.evidence)
        self.assertEqual(dependency_violations(self.metadata, PROVIDER_SELECTION_ROOT, PROVIDER_SELECTION_OWNERS), [])
        typed = self.package("typed-trees", "omega-rust/psi/compilation/representations/typed-trees")
        self.edge(selection, typed)
        self.edge(selection, self.native)
        violations = dependency_violations(self.metadata, PROVIDER_SELECTION_ROOT, PROVIDER_SELECTION_OWNERS)
        self.assertEqual(len(violations), 2)
        self.assertTrue(any("provider-selection -> typed-trees" in violation for violation in violations))
        self.assertTrue(any("provider-selection -> native-production" in violation for violation in violations))

    def test_native_worker_consumes_artifacts_without_a_compiler(self):
        worker = self.package("native-provider-runtime", NATIVE_PROVIDER_RUNTIME_ROOT)
        artifacts = self.package("native-artifact", "omega-rust/omega/compilation/backend/artifacts/native-artifact")
        self.edge(worker, artifacts)
        self.assertEqual(dependency_violations(self.metadata, NATIVE_PROVIDER_RUNTIME_ROOT, NATIVE_PROVIDER_RUNTIME_OWNERS), [])
        self.edge(artifacts, self.native, optional=True, target='cfg(windows)')
        failures = dependency_violations(self.metadata, NATIVE_PROVIDER_RUNTIME_ROOT, NATIVE_PROVIDER_RUNTIME_OWNERS)
        self.assertEqual(len(failures), 1)
        self.assertIn("native-provider-runtime -> native-artifact -> native-production", failures[0])

    def test_trust_admission_consumes_declarations_without_source_or_native_compilation(self):
        trust = self.package("trust-model", TRUST_ROOT)
        declarations = self.package("language-semantics", "omega-rust/psi/foundation/language-semantics")
        self.edge(trust, declarations)
        self.assertEqual(dependency_violations(self.metadata, TRUST_ROOT, TRUST_OWNERS), [])
        typed = self.package("typed-trees", "omega-rust/psi/compilation/representations/typed-trees")
        self.edge(declarations, typed, kind="build", optional=True, target='cfg(windows)')
        failures = dependency_violations(self.metadata, TRUST_ROOT, TRUST_OWNERS)
        self.assertEqual(len(failures), 1)
        self.assertIn("trust-model -> language-semantics -> typed-trees", failures[0])

    def test_trust_admission_cannot_reopen_checked_source_or_call_native_compilation(self):
        trust = self.package("trust-model", TRUST_ROOT)
        checked = self.package("checked-trees", "omega-rust/psi/compilation/representations/checked-trees")
        self.edge(trust, checked)
        self.edge(trust, self.native)
        failures = dependency_violations(self.metadata, TRUST_ROOT, TRUST_OWNERS)
        self.assertEqual(len(failures), 2)
        self.assertTrue(any("trust-model -> checked-trees" in failure for failure in failures))
        self.assertTrue(any("trust-model -> native-production" in failure for failure in failures))

    def test_psi_cannot_reach_omega_through_inactive_production_edges(self):
        psi = self.package("psi-compiler", "omega-rust/psi/compilation/driver")
        foundation = self.package("language-semantics", "omega-rust/psi/foundation/language-semantics")
        self.edge(psi, foundation)
        self.assertEqual(psi_omega_dependency_violations(self.metadata), [])
        self.edge(foundation, self.native, kind="build", optional=True, target='cfg(windows)')
        failures = psi_omega_dependency_violations(self.metadata)
        self.assertEqual(len(failures), 1)
        self.assertIn("language-semantics -> native-production", failures[0])
        self.edge(psi, self.native, kind="dev")
        self.assertEqual(len(psi_omega_dependency_violations(self.metadata)), 2)

    def test_realization_cannot_import_checker_under_an_inactive_alias(self):
        dispatch = self.package("selected-dispatch", "omega-rust/omega/compilation/realization/selected-dispatch")
        checker = self.package("validation", "omega-rust/psi/compilation/checking")
        self.edge(dispatch, checker, kind="dev")
        self.assertEqual(realization_checker_entry_violations(self.metadata), [])
        self.edge(dispatch, checker, kind="build", rename="facts", optional=True, target='cfg(windows)')
        failures = realization_checker_entry_violations(self.metadata)
        self.assertEqual(len(failures), 1)
        self.assertIn("selected-dispatch -> validation", failures[0])

    def test_source_coordinator_keeps_its_checker_dependency(self):
        coordinator = self.package("psi-compiler", "omega-rust/psi/compilation/driver")
        checker = self.package("validation", "omega-rust/psi/compilation/checking")
        self.edge(coordinator, checker)
        self.assertEqual(realization_checker_entry_violations(self.metadata), [])


class RepresentationSpellingTests(unittest.TestCase):
    def test_named_record_cannot_be_classified_as_retired_text(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "omega-rust/psi/compilation/classification.rs"
            source.parent.mkdir(parents=True)
            for expression in (
                'name.as_str ( ) == "string";',
                'name.as_str(syntax) != "string";',
                'matches!(name.as_str(syntax), "f32" | "string");',
                'name == "string";',
            ):
                with self.subTest(expression=expression):
                    source.write_text(expression + "\n", encoding="utf-8")
                    failures = representation_spelling_violations(root)
                    self.assertEqual(len(failures), 1)
                    self.assertIn("classification.rs:1:", failures[0])
            source.write_text('matches!(node, TypeReferenceNode::Slice { .. });\n', encoding="utf-8")
            self.assertEqual(representation_spelling_violations(root), [])

    def test_reversed_representation_comparisons_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "omega-rust/omega/compilation/classification.rs"
            source.parent.mkdir(parents=True)
            for expression in (
                '"string" == name.as_str();',
                '"string" != self.name.as_str(syntax);',
                '"string" == name;',
                'name != "string";',
            ):
                with self.subTest(expression=expression):
                    source.write_text(expression + "\n", encoding="utf-8")
                    self.assertEqual(len(representation_spelling_violations(root)), 1)

    def test_formatted_comparisons_and_literal_data_stay_distinct(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "omega-rust/psi/compilation/classification.rs"
            source.parent.mkdir(parents=True)
            for expression in (
                'name\n .as_str(syntax)\n ==\n "string";',
                '"string"\n !=\n self.name.as_str(syntax);',
                'name\n ==\n "string";',
            ):
                with self.subTest(expression=expression):
                    source.write_text(expression + "\n", encoding="utf-8")
                    self.assertEqual(len(representation_spelling_violations(root)), 1)
            source.write_text('let name = "string";\nlet class = RepresentationClass::Slice;\n', encoding="utf-8")
            self.assertEqual(representation_spelling_violations(root), [])


class ForbiddenDependencyTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[2]
        self.metadata = {"workspace_root": str(self.root), "packages": []}

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

    def test_every_rule_names_existing_workspace_paths(self):
        for rule in FORBIDDEN_DEPENDENCIES:
            for path in (*rule.owners, *rule.forbidden, *rule.except_owners):
                self.assertTrue((self.root / path).is_dir(), f"{path}: {rule.reason}")

    def test_psi_cannot_depend_on_omega(self):
        checking = self.package("checking", "omega-rust/psi/compilation/checking")
        target = self.package("target", "omega-rust/omega/compilation/backend/target")
        self.edge(checking, target, optional=True, target='cfg(unix)')
        failures = forbidden_dependency_violations(self.metadata)
        self.assertEqual(len(failures), 1)
        self.assertIn("checking -> target: Psi must not depend on Omega", failures[0])

    def test_dev_dependency_on_omega_is_not_a_production_edge(self):
        checking = self.package("checking", "omega-rust/psi/compilation/checking")
        target = self.package("target", "omega-rust/omega/compilation/backend/target")
        self.edge(checking, target, kind="dev")
        self.assertEqual(forbidden_dependency_violations(self.metadata), [])

    def test_product_consumer_cannot_assemble_a_source_stage(self):
        products = self.package("compilation-products", "omega-rust/omega/compilation/products")
        checker = self.package(
            "typed-trees-to-checked-trees",
            "omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees",
        )
        facade = self.package("psi-compiler", PSI_FACADE)
        self.edge(products, facade)
        self.assertEqual(forbidden_dependency_violations(self.metadata), [])
        self.edge(products, checker)
        failures = forbidden_dependency_violations(self.metadata)
        self.assertEqual(len(failures), 1)
        self.assertIn("compilation-products -> 04_typed-trees-to-checked-trees", failures[0])

    def test_only_terminal_production_sequences_the_terminal_stages(self):
        stage = self.package(
            "lowered-psi-to-terminal-psi",
            "omega-rust/psi/compilation/pipeline/07_lowered-psi-to-terminal-psi",
        )
        production = self.package("psi-terminal-production", TERMINAL_PRODUCTION)
        self.edge(production, stage)
        self.assertEqual(forbidden_dependency_violations(self.metadata), [])
        facade = self.package("psi-compiler", PSI_FACADE)
        self.edge(facade, stage)
        failures = forbidden_dependency_violations(self.metadata)
        self.assertEqual(len(failures), 1)
        self.assertIn("psi-compiler -> 07_lowered-psi-to-terminal-psi", failures[0])

    def test_omega_crate_cannot_import_source_side_psi(self):
        dispatch = self.package("selected-dispatch", "omega-rust/omega/compilation/realization/selected-dispatch")
        typed = self.package("typed-trees", "omega-rust/psi/compilation/representations/typed-trees")
        self.edge(dispatch, typed)
        self.assertEqual(forbidden_dependency_violations(self.metadata), [])
        for owner, dependency in (
            ("omega-rust/omega/compilation/backend/target", "omega-rust/psi/compilation/representations/checked-trees"),
            ("omega-rust/omega/compilation/representations/target-operations", "omega-rust/psi/interpreter/checked-interpreter"),
            ("omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations",
             "omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi"),
            ("omega-rust/omega/compilation/backend/layout", "omega-rust/psi/compilation/checking/representation-planning"),
        ):
            with self.subTest(owner=owner):
                self.metadata["packages"] = [dispatch, typed]
                source = self.package(Path(owner).name, owner)
                self.edge(source, self.package(Path(dependency).name, dependency), kind="build", optional=True)
                failures = forbidden_dependency_violations(self.metadata)
                self.assertEqual(len(failures), 1)
                self.assertIn("Omega cannot depend on source IRs", failures[0])

    def test_omega_crate_keeps_terminal_psi_and_foundations(self):
        layout = self.package("layout", "omega-rust/omega/compilation/backend/layout")
        for dependency in ("omega-rust/psi/compilation/representations/terminal-psi",
                           "omega-rust/psi/compilation/checking/terminal-installation",
                           "omega-rust/psi/foundation/layout-plans"):
            self.edge(layout, self.package(Path(dependency).name, dependency))
        self.assertEqual(forbidden_dependency_violations(self.metadata), [])

    def test_omega_stage_cannot_import_another_stage(self):
        later = self.package("register-homes", OMEGA_STAGES[5])
        earlier = self.package("selected-instructions", OMEGA_STAGES[4])
        self.edge(later, earlier)
        self.assertEqual(forbidden_dependency_violations(self.metadata), [])
        first = self.package("abstract-operations", OMEGA_STAGES[0])
        self.edge(later, first)
        self.edge(first, later, kind="dev")
        failures = forbidden_dependency_violations(self.metadata)
        self.assertEqual(len(failures), 1)
        self.assertIn("register-homes -> 00_terminal-psi-to-abstract-operations: stages cannot call", failures[0])

    def test_removed_edge_must_leave_the_allowlist(self):
        rows = [allowed for allowed in FIREWALL_ALLOWANCES if allowed.owner == OMEGA_STAGES[1]]
        self.assertEqual(len(rows), 1)
        later = self.package("abstract-operations", rows[0].owner)
        earlier = self.package("terminal-psi-to-abstract-operations", rows[0].dependency)
        self.edge(later, earlier, kind="dev")
        failures = [failure for failure in stale_allowance_violations(self.metadata)
                    if failure.startswith(f"{rows[0].owner} ->")]
        self.assertEqual(len(failures), 1)
        self.assertIn("edge removed", failures[0])
        later["dependencies"].clear()
        self.edge(later, earlier)
        self.assertEqual([failure for failure in stale_allowance_violations(self.metadata)
                          if failure.startswith(f"{rows[0].owner} ->")], [])

    def test_allowance_outside_every_rule_fails(self):
        misplaced = Allowance("omega-rust/omega/compilation/driver",
                              "omega-rust/psi/compilation/representations/typed-trees", "Debt: test")
        rules = [rule._replace(allowed=(misplaced,)) for rule in FORBIDDEN_DEPENDENCIES if rule.allowed]
        failures = stale_allowance_violations(self.metadata, rules)
        self.assertEqual(len(failures), 1)
        self.assertIn("no rule forbids", failures[0])

    def test_allowlist_rows_name_existing_crates_and_a_reason(self):
        self.assertEqual(len(OMEGA_STAGES), 10)
        self.assertEqual(len(FIREWALL_ALLOWANCES), len(set(FIREWALL_ALLOWANCES)))
        for allowed in FIREWALL_ALLOWANCES:
            with self.subTest(edge=allowed[:2]):
                self.assertTrue((self.root / allowed.owner / "Cargo.toml").is_file())
                self.assertTrue((self.root / allowed.dependency / "Cargo.toml").is_file())
                self.assertRegex(allowed.reason, r"^(Sanctioned: pipeline\.md:\d|Debt: )")

    def test_the_workspace_matches_its_allowlist(self):
        result = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"],
                                cwd=self.root, capture_output=True, text=True, check=True)
        metadata = json.loads(result.stdout)
        self.assertEqual(forbidden_dependency_violations(metadata), [])
        self.assertEqual(stale_allowance_violations(metadata), [])

    def test_sibling_with_owner_prefix_is_not_the_owner(self):
        counterfeit = self.package("counterfeit", "omega-rust/psi-extra/driver")
        target = self.package("target", "omega-rust/omega/compilation/backend/target")
        self.edge(counterfeit, target)
        self.assertEqual(forbidden_dependency_violations(self.metadata), [])


if __name__ == "__main__":
    unittest.main()
