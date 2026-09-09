use super::{
    Sources, assert_same_machine_types, compile, compile_to_checked_with_packages, identity,
    root_inputs, selections,
};

const FLAG: &str = "pub data Flag<const Enabled: bool> { value: u8; }";

fn keep(name: &str, index: &str) -> String {
    format!(
        "machine {name}(value: Flag<{index}>) -> Flag<{index}> {{ let local: Flag<{index}> = value; local }}"
    )
}

#[test]
fn comparisons_preserve_module_selection_and_match_boolean_literal_identity() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (operator, root_result, module_result) in [
        ("==", true, false),
        ("!=", false, true),
        ("<", false, true),
        ("<=", true, true),
        (">", false, false),
        (">=", true, false),
    ] {
        let index = format!("(LIMIT {operator} 3)");
        Sources::write(
            root.join("settings.omg"),
            &format!(
                "module settings; const LIMIT: u64 = 2; {}",
                keep("keep", &index)
            ),
        );
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; {FLAG} const LIMIT: u64 = 3; {} {} {}",
                keep("keep", &index),
                keep("enabled", "true"),
                keep("disabled", "false")
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        let operators = checked.authored_declaration_selections().iter()
            .filter(|selection| selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Operator)
            .collect::<Vec<_>>();
        assert_eq!(
            operators.len(),
            6,
            "every comparison occurrence survives canonicalization"
        );
        for (position, selection) in operators.iter().enumerate() {
            assert!(
                operators[..position]
                    .iter()
                    .all(|prior| prior.source_span() != selection.source_span())
            );
        }
        for (name, result, constant) in [
            ("keep", root_result, "LIMIT"),
            ("settings::keep", module_result, "settings::LIMIT"),
        ] {
            assert_same_machine_types(&checked, name, if result { "enabled" } else { "disabled" });
            let uses = selections(&checked, constant, identity(1));
            assert_eq!(uses.len(), 3);
            for (position, selection) in uses.iter().enumerate() {
                assert!(
                    uses[..position]
                        .iter()
                        .all(|prior| prior.source_span() != selection.source_span())
                );
            }
        }
    }
}

#[test]
fn comparisons_keep_signedness_full_unsigned_width_and_operand_landings() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, value, expression, result) in [
        ("i64", "-9223372036854775808", "(LIMIT < 0)", true),
        ("i64", "-1", "(0 > LIMIT)", true),
        (
            "u64",
            "18446744073709551615",
            "(LIMIT > 9223372036854775807)",
            true,
        ),
        (
            "u64",
            "18446744073709551615",
            "(LIMIT == 18446744073709551615)",
            true,
        ),
        ("u8", "254", "(LIMIT + 1 == 255)", true),
        ("u8", "3", "(6 / 2 == LIMIT)", true),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{FLAG} const LIMIT: {carrier} = {value}; {} {}",
                keep("keep", expression),
                keep("oracle", if result { "true" } else { "false" })
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_same_machine_types(&checked, "keep", "oracle");
    }
}

#[test]
fn comparisons_reject_runtime_operands_unsafe_arithmetic_and_authored_operators() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, value, expression, expected) in [
        (
            "u8",
            "255",
            "(LIMIT + 1 == 0)",
            "Exact integer constant operation",
        ),
        (
            "u8",
            "1",
            "(LIMIT == 1u64)",
            "incompatible landed integer carriers",
        ),
        ("u8", "1", "(LIMIT == 256)", "cannot land exactly"),
        ("u8", "3", "(7 / 2 == LIMIT)", "cannot land exactly"),
        (
            "u64",
            "1",
            "(LIMIT / 0 == 0)",
            "Exact integer constant operation",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{FLAG} const LIMIT: {carrier} = {value}; {}",
                keep("keep", expression)
            ),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("comparison cannot erase operand obligations");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:?}"
        );
    }
    for machine in [
        "machine keep(LIMIT: u64, value: Flag<(LIMIT == 3)>) {}",
        "machine keep(input: u64) { let LIMIT: u64 = input; let local: Flag<(LIMIT == 3)>; }",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{FLAG} const LIMIT: u64 = 3; {machine}"),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("runtime operand is not the same-named constant");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("original lexical scope")),
            "{diagnostics:?}"
        );
    }
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{FLAG} const LIMIT: u64 = 3; operator == u64::equal(left: u64, right: u64) -> bool; {}",
            keep("keep", "(LIMIT == 3)")
        ),
    );
    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("authored equality cannot acquire builtin comparison meaning");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("requires exact authored selection")),
        "{diagnostics:?}"
    );
}
