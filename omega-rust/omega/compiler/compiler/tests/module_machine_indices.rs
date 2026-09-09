use compiler::{CheckedCompilation, compile_to_checked_with_packages};
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelection, AuthoredDeclarationSelectionExposure,
    AuthoredDeclarationSelectionTarget,
};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use typed_trees::{statement::StatementNode, types::TypeReferenceHandle};

#[path = "module_machine_indices/comparisons.rs"]
mod comparisons;

#[path = "module_machine_indices/noninteger.rs"]
mod noninteger;

static NEXT_TREE: AtomicU64 = AtomicU64::new(0);
const BUFFER: &str = "pub data Buffer<const N: u64> { value: [u8; N]; }";

#[test]
fn machine_parameter_result_and_local_indices_select_exact_module_constants() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (module, value) in [("combat", 2), ("rooms", 3)] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module}; const SIZE: u64 = {value}; {}",
                keep("keep", "SIZE + 1")
            ),
        );
    }
    for imports in ["use combat; use rooms;", "use rooms; use combat;"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{imports} {BUFFER} const SIZE: u64 = 1; {} {} {} {}",
                keep("keep", "SIZE + 1"),
                keep("two", "2"),
                keep("three", "3"),
                keep("four", "4")
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        for (path, oracle, constant) in [
            ("keep", "two", "SIZE"),
            ("combat::keep", "three", "combat::SIZE"),
            ("rooms::keep", "four", "rooms::SIZE"),
        ] {
            assert_same_machine_types(&checked, path, oracle);
            let uses = selections(&checked, constant, identity(1));
            assert_eq!(
                uses.len(),
                3,
                "parameter, result, and local independently select {constant}"
            );
            for (position, selection) in uses.iter().enumerate() {
                assert_eq!(
                    selection.exposure(),
                    AuthoredDeclarationSelectionExposure::PrivateImplementation
                );
                assert!(
                    uses[..position]
                        .iter()
                        .all(|prior| prior.source_span() != selection.source_span())
                );
            }
        }
        let two = machine_types(&checked, "two")[0];
        let three = machine_types(&checked, "three")[0];
        let four = machine_types(&checked, "four")[0];
        assert_ne!(
            checked.typed.normalized_type_identity(two),
            checked.typed.normalized_type_identity(three)
        );
        assert_ne!(
            checked.typed.normalized_type_identity(three),
            checked.typed.normalized_type_identity(four)
        );
    }
}

#[test]
fn runtime_machine_bindings_are_not_global_constant_arguments() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    let declarations = format!("use constants::SIZE; {BUFFER}");
    for (literal, shadowed) in [
        (
            "machine keep(SIZE: u64, value: Buffer<3>) -> Buffer<3> { value }",
            "machine keep(SIZE: u64, value: Buffer<SIZE + 1>) -> Buffer<SIZE + 1> { value }",
        ),
        (
            "machine keep(input: u64, value: Buffer<3>) -> Buffer<3> { let SIZE: u64 = input; let local: Buffer<3> = value; local }",
            "machine keep(input: u64, value: Buffer<3>) -> Buffer<3> { let SIZE: u64 = input; let local: Buffer<SIZE + 1> = value; local }",
        ),
    ] {
        Sources::write(root.join("main.omg"), &format!("{declarations} {literal}"));
        compile(&root, root_inputs(&root));
        Sources::write(root.join("main.omg"), &format!("{declarations} {shadowed}"));
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("runtime SIZE must not become the imported constant's canonical value");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "machine index operand must select a constant in its original lexical scope"
                )),
            "runtime shadow fixture must reject its lexical operand: {diagnostics:?}"
        );
    }
}

#[test]
fn later_local_declarations_do_not_shadow_earlier_index_occurrences() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    let source = format!(
        "use constants::SIZE; {BUFFER}
         machine keep(input: u64, value: Buffer<3>) -> Buffer<3> {{ let local: Buffer<SIZE + 1> = value; let SIZE: u64 = input; local }} {}", keep("oracle", "3")
    );
    Sources::write(root.join("main.omg"), &source);
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "keep", "oracle");
    let uses = selections(&checked, "constants::SIZE", identity(1));
    assert_eq!(
        uses.len(),
        2,
        "the explicit import and earlier annotation retain independent selections"
    );
    let annotation_start = source.find("SIZE + 1").unwrap();
    assert!(
        uses.iter()
            .any(|selection| selection.source_span().span.start == annotation_start)
    );
}

#[test]
fn public_machine_signatures_and_private_body_indices_keep_distinct_exposure() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use constants; {BUFFER} pub machine keep(value: Buffer<3>) -> Buffer<3> {{ let local: Buffer<constants::SIZE + 1> = value; local }}"
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    let uses = selections(&checked, "constants::SIZE", identity(1));
    assert_eq!(uses.len(), 1);
    assert_eq!(
        uses[0].exposure(),
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use constants; {BUFFER} pub {}",
            keep("keep", "constants::SIZE + 1")
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("a private constant cannot appear in a public machine's normalized signature");
    Sources::write(
        root.join("constants.omg"),
        "module constants; pub const SIZE: u64 = 2;",
    );
    let checked = compile(&root, root_inputs(&root));
    let uses = selections(&checked, "constants::SIZE", identity(1));
    assert_eq!(uses.len(), 3);
    assert_eq!(
        uses.iter()
            .filter(|selection| selection.exposure()
                == AuthoredDeclarationSelectionExposure::PublicInterface)
            .count(),
        2
    );
    assert_eq!(
        uses.iter()
            .filter(|selection| selection.exposure()
                == AuthoredDeclarationSelectionExposure::PrivateImplementation)
            .count(),
        1
    );
}

#[test]
fn machine_index_evaluation_preserves_declared_carriers_and_intermediate_overflow() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, value, expression, expected) in [
        (
            "u32",
            "2",
            "SIZE + 1",
            "landed `u32` result cannot initialize `u64`",
        ),
        (
            "u8",
            "255",
            "(SIZE + 1) - 1",
            "Exact integer constant operation",
        ),
    ] {
        Sources::write(
            root.join("constants.omg"),
            &format!("module constants; const SIZE: {carrier} = {value};"),
        );
        Sources::write(
            root.join("main.omg"),
            &format!("use constants::SIZE; {BUFFER} {}", keep("keep", "3")),
        );
        compile(&root, root_inputs(&root));
        Sources::write(
            root.join("main.omg"),
            &format!("use constants::SIZE; {BUFFER} {}", keep("keep", expression)),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("machine type index cannot erase the selected carrier");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn machine_indices_require_direct_public_package_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    Sources::write(
        middle.join("bridge.omg"),
        "use leaf::constants; pub machine bridge() -> u64 { leaf::constants::SIZE }",
    );
    Sources::write(
        leaf.join("constants.omg"),
        "module constants; pub const SIZE: u64 = 2;",
    );
    let sources = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle),
        PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
    ];
    let mut dependencies = vec![
        PackageDependencyBinding::new(identity(1), "middle", identity(2)),
        PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
    ];
    let indirect =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap();
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use middle::bridge; {BUFFER} {}",
            keep("keep", "constants::SIZE + 1")
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, indirect)
        .expect_err("loaded transitive constants cannot enter machine indices");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let direct = PackageCompilationInputs::new_package(identity(1), sources, dependencies).unwrap();
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use leaf::constants; {BUFFER} {} {}",
            keep("keep", "leaf::constants::SIZE + 1"),
            keep("oracle", "3")
        ),
    );
    let checked = compile(&root, direct.clone());
    assert_same_machine_types(&checked, "keep", "oracle");
    assert_eq!(
        selections(&checked, "constants::SIZE", identity(3)).len(),
        3
    );
    Sources::write(
        leaf.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, direct)
        .expect_err("a direct dependency does not expose private machine index constants");
}

#[test]
fn root_only_constant_indices_respect_runtime_parameter_and_prior_local_shadows() {
    let tree = Sources::new();
    let root = tree.package("root");
    for index in ["SIZE", "SIZE + 0"] {
        for (literal, shadowed) in [
            (
                "machine keep(SIZE: u64, value: Buffer<2>) -> Buffer<2> { value }".to_owned(),
                format!("machine keep(SIZE: u64, value: Buffer<{index}>) -> Buffer<{index}> {{ value }}"),
            ),
            (
                "machine keep(input: u64, value: Buffer<2>) -> Buffer<2> { let SIZE: u64 = input; let local: Buffer<2> = value; local }".to_owned(),
                format!("machine keep(input: u64, value: Buffer<2>) -> Buffer<2> {{ let SIZE: u64 = input; let local: Buffer<{index}> = value; local }}"),
            ),
        ] {
            Sources::write(root.join("main.omg"), &format!("{BUFFER} const SIZE: u64 = 2; {literal}"));
            compile(&root, root_inputs(&root));
            Sources::write(root.join("main.omg"), &format!("{BUFFER} const SIZE: u64 = 2; {shadowed}"));
            let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("root-only evaluation must retain runtime lexical bindings");
            assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains("machine index operand must select a constant in its original lexical scope")), "{diagnostics:?}");
        }
    }
}

#[test]
fn machine_index_selection_coexists_with_root_aggregate_constant_materialization() {
    let tree = Sources::new();
    let root = tree.package("root");
    let declarations = format!(
        "{BUFFER} data Pair {{ value: u64; }} const PAIR: Pair = Pair {{ value: 11 }}; const SIZE: u64 = 2;"
    );
    let machine = |name: &str, index: &str| {
        format!(
            "machine {name}(value: Buffer<{index}>) -> Buffer<{index}> {{ let aggregate: Pair = PAIR; let local: Buffer<{index}> = value; local }}"
        )
    };
    Sources::write(
        root.join("main.omg"),
        &format!("{declarations} {}", machine("oracle", "3")),
    );
    compile(&root, root_inputs(&root));
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{declarations} {} {}",
            machine("keep", "SIZE + 1"),
            machine("oracle", "3")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "keep", "oracle");
    assert_eq!(selections(&checked, "SIZE", identity(1)).len(), 3);
    assert_eq!(
        selections(&checked, "PAIR", identity(1)).len(),
        2,
        "both real aggregate occurrences retain declaration custody"
    );
    for machine in checked.typed.machines() {
        for state in checked.typed.machine_states(machine) {
            let aggregate = checked
                .typed
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.name.as_str() == "aggregate" => {
                        Some(local.initial_value)
                    }
                    _ => None,
                })
                .expect("aggregate local initializer");
            assert!(
                matches!(
                    checked.typed.expression_table.expression(aggregate),
                    typed_trees::expression::ExpressionNode::StructLiteral(_)
                ),
                "aggregate materialization remains available beside scalar index selection"
            );
        }
    }
}

#[test]
fn named_and_compound_machine_indices_have_the_same_canonical_types() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use constants; {BUFFER} {} {} {}",
            keep("named", "constants::SIZE"),
            keep("compound", "constants::SIZE + 0"),
            keep("oracle", "2")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "named", "oracle");
    assert_same_machine_types(&checked, "compound", "oracle");
    let uses = selections(&checked, "constants::SIZE", identity(1));
    assert_eq!(uses.len(), 6);
    for (position, selection) in uses.iter().enumerate() {
        assert!(
            uses[..position]
                .iter()
                .all(|prior| prior.source_span() != selection.source_span())
        );
    }
}

#[test]
fn qualified_named_indices_preserve_runtime_qualifier_root_shadowing() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    for index in ["constants::SIZE", "constants::SIZE + 0"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use constants; {BUFFER} {} {}",
                keep("keep", index),
                keep("oracle", "2")
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_same_machine_types(&checked, "keep", "oracle");
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use constants; {BUFFER} machine keep(constants: u64, value: Buffer<2>) -> Buffer<2> {{ value }}"
            ),
        );
        compile(&root, root_inputs(&root));
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use constants; {BUFFER} machine keep(constants: u64, value: Buffer<{index}>) -> Buffer<{index}> {{ value }}"
            ),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("a qualified Named argument must not flatten away its lexical root");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "machine index operand must select a constant in its original lexical scope"
                )),
            "{index}: {diagnostics:?}"
        );
    }
}

#[test]
fn machine_domain_parameter_result_and_local_indices_match_literal_identity() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    let machine = |name: &str, index: &str| {
        format!(
            "machine {name}(value: u64 in Indexed<{index}>) -> u64 in Indexed<{index}> {{ let local: u64 in Indexed<{index}> = value; local }}"
        )
    };
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use constants; domain<T, const N: u64> T::Indexed<N>; {} {}",
            machine("keep", "constants::SIZE + 1"),
            machine("oracle", "3")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "keep", "oracle");
    assert_eq!(
        selections(&checked, "constants::SIZE", identity(1)).len(),
        3
    );
}

#[test]
fn domain_cast_indices_use_the_original_machine_lexical_scope() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    let declarations = "use constants::SIZE; domain<T, const N: u64> T::Indexed<N>;";
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{declarations} machine keep(value: u64) -> u64 in Indexed<3> {{ let local: u64 in Indexed<3> = (value as u64 in Indexed<SIZE + 1>); local }}"
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    let uses = selections(&checked, "constants::SIZE", identity(1));
    assert_eq!(
        uses.len(),
        2,
        "import and actual cast index both retain selection custody"
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{declarations} machine keep(SIZE: u64, value: u64) -> u64 in Indexed<3> {{ let local: u64 in Indexed<3> = (value as u64 in Indexed<3>); local }}"
        ),
    );
    compile(&root, root_inputs(&root));
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{declarations} machine keep(SIZE: u64, value: u64) -> u64 in Indexed<3> {{ let local: u64 in Indexed<3> = (value as u64 in Indexed<SIZE + 1>); local }}"
        ),
    );
    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("cast index must not fold a runtime parameter as the imported constant");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "machine index operand must select a constant in its original lexical scope"
            )),
        "{diagnostics:?}"
    );
}

fn keep(name: &str, index: &str) -> String {
    format!(
        "machine {name}(value: Buffer<{index}>) -> Buffer<{index}> {{ let local: Buffer<{index}> = value; local }}"
    )
}

fn machine_types(checked: &CheckedCompilation, path: &str) -> [TypeReferenceHandle; 3] {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| checked.symbols.display_path(machine.symbol, "::") == path)
        .expect("machine declaration");
    let [state] = checked.typed.machine_states(machine) else {
        panic!("one machine state")
    };
    let parameter = checked
        .typed
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "value")
        .expect("value parameter");
    let local = checked
        .typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "local" => {
                Some(local.type_reference)
            }
            _ => None,
        })
        .expect("authored local annotation");
    [parameter.type_reference, state.return_type, local]
}

fn assert_same_machine_types(checked: &CheckedCompilation, path: &str, oracle: &str) {
    let actual = machine_types(checked, path);
    let expected = machine_types(checked, oracle);
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert_eq!(
            checked.typed.normalized_type_identity(actual),
            checked.typed.normalized_type_identity(expected),
            "{path} must match literal oracle {oracle}"
        );
    }
}

fn selections(
    checked: &CheckedCompilation,
    path: &str,
    owner: PackageKeyIdentity,
) -> Vec<AuthoredDeclarationSelection> {
    checked.authored_declaration_selections().iter().filter(|selection| matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(target)
        if checked.symbols.display_path(target.selected_symbol(), "::") == path && checked.symbols.symbol_package_identity(target.selected_symbol()) == Some(owner))).copied().collect()
}

fn identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).unwrap()
}
fn root_inputs(root: &Path) -> PackageCompilationInputs {
    PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(
            identity(1),
            "root",
            root.to_path_buf(),
        )],
        Vec::new(),
    )
    .unwrap()
}
fn compile(root: &Path, inputs: PackageCompilationInputs) -> CheckedCompilation {
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("machine index fixture checks")
}
struct Sources(PathBuf);
impl Sources {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-module-machine-indices-{}-{}",
            std::process::id(),
            NEXT_TREE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn package(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).unwrap();
        path
    }
    fn write(path: impl AsRef<Path>, source: &str) {
        fs::write(path, source).unwrap();
    }
}
impl Drop for Sources {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
