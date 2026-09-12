use super::{
    Sources, assert_same_machine_types, compile, compile_to_checked, identity, root_inputs,
    selections,
};
use compiler::CheckedCompileRequest;

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
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
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
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
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
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_inputs(&root)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("authored equality cannot acquire builtin comparison meaning");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("requires exact authored selection")),
        "{diagnostics:?}"
    );
}

#[test]
fn boolean_equality_indices_preserve_module_selection_and_canonical_identity() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (operator, peer, root_result, module_result) in [
        ("==", true, true, false),
        ("!=", true, false, true),
        ("==", false, false, true),
        ("!=", false, true, false),
    ] {
        let index = format!("(ENABLED {operator} {peer})");
        Sources::write(
            root.join("settings.omg"),
            &format!(
                "module settings; const ENABLED: bool = false; {}",
                keep("keep", &index)
            ),
        );
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; {FLAG} const ENABLED: bool = true; {} {} {}",
                keep("keep", &index),
                keep("enabled", "true"),
                keep("disabled", "false")
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        for (name, result, constant) in [
            ("keep", root_result, "ENABLED"),
            ("settings::keep", module_result, "settings::ENABLED"),
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
        let operators = checked.authored_declaration_selections().iter().filter(|selection| selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Operator).collect::<Vec<_>>();
        assert_eq!(operators.len(), 6);
        for (position, selection) in operators.iter().enumerate() {
            assert!(
                operators[..position]
                    .iter()
                    .all(|prior| prior.source_span() != selection.source_span())
            );
        }
    }
}

#[test]
fn boolean_equality_indices_compose_with_comparison_results() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (expression, result) in [
        ("(ENABLED == false)", false),
        ("(false != ENABLED)", true),
        ("(ENABLED == ENABLED)", true),
        ("((LIMIT == 3) == ENABLED)", true),
        ("((LIMIT < 3) != (LIMIT > 1))", true),
        ("((ENABLED != false) == true)", true),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{FLAG} const ENABLED: bool = true; const LIMIT: u64 = 3; {} {}",
                keep("keep", expression),
                keep("oracle", if result { "true" } else { "false" })
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_same_machine_types(&checked, "keep", "oracle");
    }
}

#[test]
fn boolean_equality_indices_reject_runtime_and_authored_meaning() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (declarations, machine, expected) in [
        ("", "machine keep(ENABLED: bool, value: Flag<(ENABLED == true)>) {}".to_owned(), "original lexical scope"),
        ("", "machine keep(input: bool) { let ENABLED: bool = input; let value: Flag<(ENABLED != false)>; }".to_owned(), "original lexical scope"),
        ("operator == bool::equal(left: bool, right: bool) -> bool;", keep("keep", "(ENABLED == true)"), "requires exact authored selection"),
        ("operator != bool::different(left: bool, right: bool) -> bool;", keep("keep", "(ENABLED != false)"), "requires exact authored selection"),
    ] {
        Sources::write(root.join("main.omg"), &format!("{FLAG} const ENABLED: bool = true; {declarations} {machine}"));
        let diagnostics = compile_to_checked(CheckedCompileRequest { package_inputs: Some(root_inputs(&root)), ..CheckedCompileRequest::new(&root.join("main.omg"), None) }).expect_err("Boolean equality cannot erase selection obligations");
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.message.contains(expected)), "{diagnostics:?}");
    }
}

#[test]
fn boolean_logic_indices_select_only_the_required_value_branch() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (expression, result) in [
        ("(ENABLED && true)", true),
        ("(ENABLED && false)", false),
        ("(false && ENABLED)", false),
        ("(true || ENABLED)", true),
        ("(false || ENABLED)", true),
        ("(ENABLED || false)", true),
        ("(false && (LIMIT / 0 == 0))", false),
        ("(true || (LIMIT / 0 == 0))", true),
        ("((ENABLED && false) || (LIMIT == 3))", true),
        ("((ENABLED || false) == (LIMIT > 1))", true),
        ("(false && (LIMIT + 255 == 0))", false),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{FLAG} const ENABLED: bool = true; const LIMIT: u8 = 3; {} {}",
                keep("keep", expression),
                keep("oracle", if result { "true" } else { "false" })
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_same_machine_types(&checked, "keep", "oracle");
    }
}

#[test]
fn boolean_logic_indices_keep_unselected_custody_and_module_identity() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        &format!(
            "module settings; const ENABLED: bool = false; {}",
            keep("keep", "(false || ENABLED)")
        ),
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings; {FLAG} const ENABLED: bool = true; {} {} {} {}",
            keep("keep", "(false || ENABLED)"),
            keep("skipped", "(true || ENABLED)"),
            keep("enabled", "true"),
            keep("disabled", "false")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "keep", "enabled");
    assert_same_machine_types(&checked, "skipped", "enabled");
    assert_same_machine_types(&checked, "settings::keep", "disabled");
    assert_eq!(selections(&checked, "ENABLED", identity(1)).len(), 6);
    assert_eq!(
        selections(&checked, "settings::ENABLED", identity(1)).len(),
        3
    );
}

#[test]
fn boolean_logic_indices_do_not_skip_admission_or_selected_branch_failures() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (declarations, machine, expected) in [
        (
            "",
            keep("keep", "(true && (LIMIT / 0 == 0))"),
            "Exact integer constant operation",
        ),
        (
            "",
            keep("keep", "(false || (LIMIT / 0 == 0))"),
            "Exact integer constant operation",
        ),
        (
            "",
            "machine keep(ENABLED: bool, value: Flag<(true || ENABLED)>) {}".to_owned(),
            "original lexical scope",
        ),
        (
            "operator == u8::equal(left: u8, right: u8) -> bool;",
            keep("keep", "(true || (LIMIT == 3))"),
            "requires exact authored selection",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{FLAG} const ENABLED: bool = true; const LIMIT: u8 = 3; {declarations} {machine}"
            ),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("selective execution retains admission and evaluated arithmetic obligations");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn boolean_logic_indices_reject_ill_typed_unselected_operands() {
    let tree = Sources::new();
    let root = tree.package("root");
    for expression in [
        "(true || LIMIT)",
        "(false && LIMIT)",
        "(true || (LIMIT == 1u64))",
        "(true || (LIMIT + 1u64 == 3))",
        "(true || ((LIMIT + 1) == 4u64))",
        "(true || ((LIMIT | 0) == 1u64))",
        "(true || ((LIMIT << 0u64) == 1u64))",
        "(true || (LIMIT == 256))",
        "(true || (LIMIT + 256 == 3))",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{FLAG} const LIMIT: u8 = 3; {}", keep("keep", expression)),
        );
        assert!(
            compile_to_checked(CheckedCompileRequest {
                package_inputs: Some(root_inputs(&root)),
                ..CheckedCompileRequest::new(&root.join("main.omg"), None)
            })
            .is_err(),
            "unselected operand must be well typed: {expression}"
        );
    }
}

#[test]
fn boolean_logic_indices_check_the_complete_static_operand_roster() {
    let tree = Sources::new();
    let root = tree.package("root");
    let mut rejected = Vec::new();
    for operator in [
        "+", "-", "*", "/", "%", "&", "|", "^", "==", "!=", "<", "<=", ">", ">=",
    ] {
        let operation = format!("(LIMIT {operator} 1u64)");
        rejected.push(
            if matches!(operator, "==" | "!=" | "<" | "<=" | ">" | ">=") {
                operation
            } else {
                format!("({operation} == 3)")
            },
        );
    }
    for expression in [
        "(LIMIT + (255 + 1) == 3)",
        "((255 + 1) + LIMIT == 3)",
        "(LIMIT + (7 / 2) == 3)",
        "(LIMIT == 18446744073709551616)",
        "(LIMIT + 18446744073709551616 == 3)",
        "(LIMIT << (7 / 2) == 3)",
        "(LIMIT >> 18446744073709551616 == 3)",
        "(1 << LIMIT == 3)",
        "(LIMIT << true == 3)",
        "((LIMIT == 3) + (LIMIT == 3) == 2)",
        "((LIMIT == 3) & true)",
        "(1 % 2 == LIMIT)",
    ] {
        rejected.push(expression.to_owned());
    }
    for expression in rejected {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{FLAG} const LIMIT: u8 = 3; {}",
                keep("keep", &format!("(true || {expression})"))
            ),
        );
        assert!(
            compile_to_checked(CheckedCompileRequest {
                package_inputs: Some(root_inputs(&root)),
                ..CheckedCompileRequest::new(&root.join("main.omg"), None)
            })
            .is_err(),
            "unselected operand must retain formation and landing: {expression}"
        );
    }
    for expression in [
        "(LIMIT + 255 == 0)",
        "(LIMIT - 4 == 0)",
        "(LIMIT * 255 == 0)",
        "(LIMIT / 0 == 0)",
        "(LIMIT % 0 == 0)",
        "(LIMIT << 8 == 0)",
        "(LIMIT >> 8 == 0)",
        "(LIMIT & (1 + 1) == 2)",
        "(LIMIT | (1 + 1) == 3)",
        "(LIMIT ^ (1 + 1) == 1)",
        "(LIMIT << 1u64 == 6)",
        "(LIMIT >> 1u64 == 1)",
        "(LIMIT + (6 / 2) == 6)",
        "((1 + 1) < 3)",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{FLAG} const LIMIT: u8 = 3; {} {}",
                keep("keep", &format!("(true || {expression})")),
                keep("oracle", "true")
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_same_machine_types(&checked, "keep", "oracle");
    }
}

#[test]
fn literal_boolean_indices_keep_canonical_identity_and_operator_occurrences() {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;

    let tree = Sources::new();
    let root = tree.package("root");
    for (expression, result, operator_count) in [
        ("(true)", true, 0),
        ("(false)", false, 0),
        ("(1u64 == 1u64)", true, 1),
        ("(1u64 != 1u64)", false, 1),
        ("(1u64 < 2u64)", true, 1),
        ("(2u64 <= 1u64)", false, 1),
        ("(2u64 > 1u64)", true, 1),
        ("(1u64 >= 2u64)", false, 1),
        ("(-1i64 < 0)", true, 1),
        ("(18446744073709551615u64 > 9223372036854775807)", true, 1),
        ("(6 / 2 == 3u8)", true, 2),
        ("(true == false)", false, 1),
        ("(true != false)", true, 1),
        ("(true && false)", false, 1),
        ("(false || true)", true, 1),
        ("(false && (1u8 / 0 == 0))", false, 3),
        ("(true || (255u8 + 1 == 0))", true, 3),
        ("((1u8 < 2) == (true || false))", true, 3),
    ] {
        Sources::write(
            root.join("settings.omg"),
            &format!("module settings; {}", keep("keep", expression)),
        );
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; {FLAG} {} {}",
                keep("keep", expression),
                keep("oracle", if result { "true" } else { "false" })
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_same_machine_types(&checked, "keep", "oracle");
        assert_same_machine_types(&checked, "settings::keep", "oracle");
        let operators = checked
            .authored_declaration_selections()
            .iter()
            .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Operator)
            .collect::<Vec<_>>();
        assert_eq!(operators.len(), 6 * operator_count, "{expression}");
        for (position, selection) in operators.iter().enumerate() {
            assert!(
                operators[..position]
                    .iter()
                    .all(|prior| prior.source_span() != selection.source_span())
            );
        }
    }
}

#[test]
fn literal_boolean_indices_retain_selection_types_and_evaluated_failures() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (declarations, expression, expected) in [
        (
            "",
            "(true && (1u8 / 0 == 0))",
            "Exact integer constant operation",
        ),
        (
            "",
            "(false || (255u8 + 1 == 0))",
            "Exact integer constant operation",
        ),
        ("", "(true || 1u8)", "incompatible operand types"),
        ("", "(true || (1u8 == 1u64))", "incompatible"),
        ("", "(true || (1u8 == 256))", "land"),
        (
            "operator == u8::equal(left: u8, right: u8) -> bool;",
            "(true || (1u8 == 1))",
            "requires exact authored selection",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{FLAG} {declarations} {}", keep("keep", expression)),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("literal indices must satisfy the same admission and scalar obligations");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{expression}: {diagnostics:?}"
        );
    }
}

#[test]
fn literal_boolean_data_fields_use_the_same_canonical_instance() {
    let tree = Sources::new();
    let root = tree.package("root");
    for property in ["[copy]", ""] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "pub data Flag<const Enabled: bool> {property} {{ value: u8; }}
                 data Holder {{ value: Flag<(1u64 < 2)>; }}
                 machine read(holder: &Holder) -> Flag<true> {{ holder.value }}"
            ),
        );
        let result = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        });
        if property.is_empty() {
            let errors = result.expect_err("canonical Boolean indices do not grant copyability");
            assert!(
                errors.iter().any(|error| error
                    .message
                    .contains("cannot transfer a non-copy value out of borrowed storage")),
                "{errors:?}"
            );
        } else {
            result.expect("copyable field retains its canonical Boolean index");
        }
    }
}
