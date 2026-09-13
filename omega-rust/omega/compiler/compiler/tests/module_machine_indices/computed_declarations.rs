use super::*;
use build_time_evaluation::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue};
use compiler::CheckedCompileRequest;

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
    assert_body_value(&checked, "read_match", 7);
    assert!(!selections(&checked, "SIZE", identity(1)).is_empty());
    assert!(!selections(&checked, "ENABLED", identity(1)).is_empty());
    assert!(!selections(&checked, "SIZES", identity(1)).is_empty());
}

#[test]
fn computed_match_arithmetic_lands_once_at_each_integer_destination_and_typed_peer() {
    let tree = Sources::new();
    let root = tree.package("root");
    for carrier in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "const FINAL: {carrier} = (match true {{ true -> 7 / 2, false -> 9 / 2 }}) * 2;
                 const PEER: {carrier} = ((match false {{ true -> 7 / 2, false -> 9 / 2 }}) * 2) + 0{carrier};
                 machine final_read() -> {carrier} {{ FINAL }}
                 machine peer_read() -> {carrier} {{ PEER }}"
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_body_value(&checked, "final_read", 7);
        assert_body_value(&checked, "peer_read", 9);
    }
}

#[test]
fn computed_match_arithmetic_composes_before_final_and_typed_peer_landings() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (expression, expected) in [
        ("((match true { true -> 7 / 2, false -> 9 / 2 }) * 2)", 7),
        ("(2 * (match true { true -> 7 / 2, false -> 9 / 2 }))", 7),
        (
            "((match true { true -> 7 / 2, false -> 9 / 2 }) + 1 / 2)",
            4,
        ),
        (
            "(10 - (match true { true -> 7 / 2, false -> 9 / 2 }) * 2)",
            3,
        ),
        ("((match true { true -> 14, false -> 18 }) / 2)", 7),
        ("((match true { true -> -7 / 2, false -> -9 / 2 }) * -2)", 7),
        (
            "((match true { true -> 18446744073709551616, false -> 18446744073709551618 }) - 18446744073709551615)",
            1,
        ),
    ] {
        for initializer in [expression.to_owned(), format!("{expression} + 0u64")] {
            Sources::write(
                root.join("main.omg"),
                &format!(
                    "{BUFFER} const SIZE: u64 = {initializer};
                     machine read() -> u64 {{ SIZE }} {} {}",
                    keep("keep", "SIZE"),
                    keep("oracle", &expected.to_string()),
                ),
            );
            let checked = compile(&root, root_inputs(&root));
            assert_body_value(&checked, "read", expected);
            assert_same_machine_types(&checked, "keep", "oracle");
        }
    }
}

#[test]
fn computed_match_integer_landing_handles_twenty_four_independent_terms() {
    let tree = Sources::new();
    let root = tree.package("root");
    for term in [
        "(match true { true -> 7, false -> 9 })",
        "(((match true { true -> 7 / 2, false -> 9 / 2 }) * 2) + 0u64)",
    ] {
        let terms = std::iter::repeat_n(term, 24)
            .collect::<Vec<_>>()
            .join(" + ");
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{BUFFER} const SIZE: u64 = ({terms}) + 0u64;
                 machine read() -> u64 {{ SIZE }} {} {}",
                keep("keep", "SIZE"),
                keep("oracle", "168"),
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_body_value(&checked, "read", 168);
        assert_same_machine_types(&checked, "keep", "oracle");
    }
}

#[test]
fn computed_match_fractional_branch_combinations_require_complete_warning_evidence() {
    let tree = Sources::new();
    let root = tree.package("root");
    let terms = std::iter::repeat_n("(match true { true -> 7 / 2, false -> 9 / 2 })", 24)
        .collect::<Vec<_>>()
        .join(" + ");
    for expression in [
        "((match true { true -> 1 / 2, false -> 3 / 2 }) * (match false { true -> 2, false -> 4 }))".to_owned(),
        "(((match true { true -> 1 / 2, false -> 3 / 2 }) + (match false { true -> 1 / 2, false -> 3 / 2 })) * 2)".to_owned(),
        format!("(({terms}) * 2)"),
    ] {
        for initializer in [expression.clone(), format!("{expression} + 0u64")] {
            Sources::write(
                root.join("main.omg"),
                &format!("const VALUE: u64 = {initializer};"),
            );
            let diagnostics =
                compile_to_checked(CheckedCompileRequest { package_inputs: Some(root_inputs(&root)), ..CheckedCompileRequest::new(&root.join("main.omg"), None) })
                    .expect_err("integral all-arm results still need complete fractional warnings");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("fractional")
                        && diagnostic.message.contains("warning")
                }),
                "warning evidence, not integer representability, remains unsupported for {initializer}: {diagnostics:?}"
            );
        }
    }
}

#[test]
fn computed_match_arithmetic_rejects_unselected_invalid_integer_landings() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, expression, expected) in [
        (
            "u8",
            "((match true { true -> 2, false -> 7 / 2 }) + 0)",
            "land",
        ),
        (
            "u8",
            "((match true { true -> 2, false -> 513 / 2 }) * 2)",
            "land",
        ),
        (
            "u8",
            "((match true { true -> 2, false -> -1 }) + 0)",
            "land",
        ),
        (
            "i8",
            "((match true { true -> 2, false -> -129 }) + 0)",
            "land",
        ),
        (
            "u64",
            "((match true { true -> 2, false -> 18446744073709551616 }) + 0)",
            "land",
        ),
        (
            "i64",
            "((match true { true -> 2, false -> 9223372036854775808 }) + 0)",
            "land",
        ),
        (
            "u8",
            "((match true { true -> 2, false -> 1 / 0 }) * 0)",
            "defined exact numeric",
        ),
        ("u8", "((match true { true -> 2, false -> 3 }) / 2)", "land"),
        (
            "u8",
            "((match true { true -> 2, false -> 3 }) * (match true { true -> 1 / 2, false -> 3 / 2 }))",
            "land",
        ),
    ] {
        for initializer in [expression.to_owned(), format!("{expression} + 0{carrier}")] {
            Sources::write(
                root.join("main.omg"),
                &format!("const INVALID: {carrier} = {initializer};"),
            );
            let diagnostics = compile_to_checked(CheckedCompileRequest {
                package_inputs: Some(root_inputs(&root)),
                ..CheckedCompileRequest::new(&root.join("main.omg"), None)
            })
            .expect_err("unused declarations still owe every result arm's landing");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{carrier} {initializer}: {diagnostics:?}"
            );
        }
    }
}

#[test]
fn computed_match_arithmetic_retains_typed_operand_width_and_selected_operation_obligations() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (initializer, expected) in [
        (
            "((match true { true -> 1, false -> 256 }) + 0u8) / 2",
            "land",
        ),
        (
            "((match true { true -> 1, false -> 7 / 2 }) + 0u8) * 2",
            "land",
        ),
        (
            "((match true { true -> 255, false -> 254 }) + 1u8) - 1",
            "Exact integer constant operation",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("const INVALID: u8 = {initializer};"),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("a safe final value cannot repair an earlier typed boundary");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{initializer}: {diagnostics:?}"
        );
    }
}

#[test]
fn computed_match_boolean_short_circuit_keeps_static_landing_without_executing_subjects() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "pub data Flag<const Enabled: bool> { value: u8; }
         const SKIPPED: bool = false && (((match (1u8 / 0 == 0) { true -> 7 / 2, false -> 9 / 2 }) * 2) == 0u8);
         const SELECTED: bool = match true { true -> true, false -> (((match (1u8 / 0 == 0) { true -> 7 / 2, false -> 9 / 2 }) * 2) == 0u8) };
         machine skipped(value: Flag<SKIPPED>) -> Flag<SKIPPED> { let local: Flag<SKIPPED> = value; local }
         machine selected(value: Flag<SELECTED>) -> Flag<SELECTED> { let local: Flag<SELECTED> = value; local }
         machine disabled(value: Flag<false>) -> Flag<false> { let local: Flag<false> = value; local }
         machine enabled(value: Flag<true>) -> Flag<true> { let local: Flag<true> = value; local }",
    );
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "skipped", "disabled");
    assert_same_machine_types(&checked, "selected", "enabled");

    for expression in [
        "(((match true { true -> 2, false -> 7 / 2 }) + 0) == 0u8)",
        "(((match true { true -> 2, false -> 256 }) + 0) == 0u8)",
    ] {
        for initializer in [
            format!("false && {expression}"),
            format!("true || {expression}"),
        ] {
            Sources::write(
                root.join("main.omg"),
                &format!("const INVALID: bool = {initializer};"),
            );
            let diagnostics = compile_to_checked(CheckedCompileRequest {
                package_inputs: Some(root_inputs(&root)),
                ..CheckedCompileRequest::new(&root.join("main.omg"), None)
            })
            .expect_err("short circuit skips execution, not anonymous operand landing");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("land")),
                "{initializer}: {diagnostics:?}"
            );
        }
    }
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
fn computed_module_array_constants_check_and_reject_bad_private_landings() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings;
         const SIZE: u64 = 7 / 2 * 2;
         pub const SIZES: [u64; 2] = [SIZE * 2, 3];",
    );
    Sources::write(
        root.join("main.omg"),
        "use settings;
         data Indexed<const Selected: [u64; 2]> { value: u8; }
         machine keep(value: Indexed<settings::SIZES>) -> Indexed<settings::SIZES> { value }",
    );
    let checked = compile(&root, root_inputs(&root));
    assert!(!selections(&checked, "settings::SIZES", identity(1)).is_empty());

    Sources::write(
        root.join("settings.omg"),
        "module settings;
         const SIZE: u64 = 7 / 2 * 2;
         pub const SIZES: [u64; 2] = [SIZE * 2, 3];
         const BAD: [u8; 1] = [200 + 100];",
    );
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_inputs(&root)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("unused private module array still lands its leaves");
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
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(indirect),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("loading the leaf through middle grants no initializer selection authority");

    Sources::write(
        root.join("main.omg"),
        &format!(
            "use leaf::constants; {BUFFER} const SIZE: u64 = leaf::constants::BASE + 1; {}",
            keep("keep", "SIZE")
        ),
    );
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(direct),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a direct dependency cannot name the public constant's private implementation");
}
