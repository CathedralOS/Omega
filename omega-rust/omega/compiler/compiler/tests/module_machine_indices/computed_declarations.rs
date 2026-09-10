use super::*;
use build_time_evaluation::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue};

fn assert_body_value(checked: &CheckedCompilation, path: &str, expected: i64) {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| checked.symbols.display_path(machine.symbol, "::") == path)
        .expect("constant body consumer");
    let result = BuildTimeAdmissionPlan::infer(&checked.typed)
        .evaluate_machine_symbol_for_invocation_measured(
            &checked.typed,
            machine.symbol,
            vec![],
            BuildTimeInvocationCustody::Symbol(machine.symbol),
        )
        .expect("checked constant body evaluates");
    assert_eq!(result.value(), &BuildTimeValue::Int(expected), "{path}");
}

#[test]
fn computed_constant_customer_checks_integer_boolean_indices_and_body() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../tests/omega/pass/modules/computed_constant_initializers/main.omg"
        )),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_body_value(&checked, "read", 7);
    assert!(!selections(&checked, "SIZE", identity(1)).is_empty());
    assert!(!selections(&checked, "ENABLED", identity(1)).is_empty());
}

#[test]
fn computed_module_constants_keep_same_leaf_forward_dependencies_and_literal_types() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (module, numerator) in [("combat", 7), ("rooms", 9)] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module};
             const SIZE: u64 = BASE / 2;
             const BASE: u64 = {numerator} / 2 * 2;
             machine read() -> u64 {{ SIZE }} {}",
                keep("keep", "SIZE")
            ),
        );
    }
    for imports in ["use combat; use rooms;", "use rooms; use combat;"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{imports} {BUFFER} {} {} {} {}
             machine combat_read() -> u64 {{ combat::SIZE }}
             machine rooms_read() -> u64 {{ rooms::SIZE }}",
                keep("combat_keep", "combat::SIZE"),
                keep("rooms_keep", "rooms::SIZE"),
                keep("three", "3"),
                keep("four", "4")
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        for (path, oracle) in [
            ("combat::keep", "three"),
            ("combat_keep", "three"),
            ("rooms::keep", "four"),
            ("rooms_keep", "four"),
        ] {
            assert_same_machine_types(&checked, path, oracle);
        }
        for (path, value) in [
            ("combat::read", 3),
            ("combat_read", 3),
            ("rooms::read", 4),
            ("rooms_read", 4),
        ] {
            assert_body_value(&checked, path, value);
        }
        for module in ["combat", "rooms"] {
            for name in ["SIZE", "BASE"] {
                let uses = selections(&checked, &format!("{module}::{name}"), identity(1));
                assert!(!uses.is_empty(), "{module}::{name}");
                assert!(uses.iter().all(|selection| selection.exposure()
                    == AuthoredDeclarationSelectionExposure::PrivateImplementation));
            }
        }
    }
}

#[test]
fn public_computed_constant_keeps_private_initializer_dependencies_out_of_interface_exposure() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const BASE: u64 = 7 / 2 * 2; pub const SIZE: u64 = BASE / 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use constants; {BUFFER} pub {} {}",
            keep("published", "constants::SIZE"),
            keep("oracle", "3")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "published", "oracle");
    let public_uses = selections(&checked, "constants::SIZE", identity(1));
    assert_eq!(
        public_uses
            .iter()
            .filter(|selection| selection.exposure()
                == AuthoredDeclarationSelectionExposure::PublicInterface)
            .count(),
        2
    );
    let private_uses = selections(&checked, "constants::BASE", identity(1));
    assert!(
        !private_uses.is_empty(),
        "initializer dependency custody survives folding"
    );
    assert!(private_uses.iter().all(|selection| selection.exposure()
        == AuthoredDeclarationSelectionExposure::PrivateImplementation));
}

#[test]
fn computed_initializer_requires_direct_dependency_and_foreign_visibility() {
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
        "module constants; const BASE: u64 = 7 / 2 * 2; pub const SIZE: u64 = BASE / 2;",
    );
    let sources = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle),
        PackageSourceBinding::new(identity(3), "leaf", leaf),
    ];
    let dependencies = vec![
        PackageDependencyBinding::new(identity(1), "middle", identity(2)),
        PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
    ];
    let indirect =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .expect("indirect package graph");
    let mut direct_dependencies = dependencies;
    direct_dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let direct = PackageCompilationInputs::new_package(identity(1), sources, direct_dependencies)
        .expect("direct package graph");
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use middle::bridge; use leaf::constants; {BUFFER}
         const SIZE: u64 = leaf::constants::SIZE + 1; {} {}",
            keep("keep", "SIZE"),
            keep("oracle", "4")
        ),
    );
    let checked = compile(&root, direct.clone());
    assert_same_machine_types(&checked, "keep", "oracle");
    let private_uses = selections(&checked, "constants::BASE", identity(3));
    assert!(!private_uses.is_empty());
    assert!(private_uses.iter().all(|selection| selection.exposure()
        == AuthoredDeclarationSelectionExposure::PrivateImplementation));

    Sources::write(
        root.join("main.omg"),
        &format!(
            "use middle::bridge; {BUFFER} const SIZE: u64 = leaf::constants::SIZE + 1; {}",
            keep("keep", "SIZE")
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, indirect)
        .expect_err("loading the leaf through middle grants no initializer selection authority");

    Sources::write(
        root.join("main.omg"),
        &format!(
            "use leaf::constants; {BUFFER} const SIZE: u64 = leaf::constants::BASE + 1; {}",
            keep("keep", "SIZE")
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, direct)
        .expect_err("a direct dependency cannot name the public constant's private implementation");
}
