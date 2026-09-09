use super::{
    Sources, assert_same_machine_types, compile, compile_to_checked_with_packages, root_inputs,
};
use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;

const FLAG: &str = "pub data Flag<const Enabled: bool> { value: u8; }";

fn keep(name: &str, index: &str) -> String {
    format!(
        "machine {name}(value: Flag<{index}>) -> Flag<{index}> {{ let local: Flag<{index}> = value; local }}"
    )
}

#[test]
fn anonymous_comparisons_preserve_rational_values_and_operator_custody() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (expression, result, operator_count) in [
        ("(1 / 3 < 1 / 2)", true, 3),
        ("(7 / 2 == 3)", false, 2),
        ("(7 / 2 != 3)", true, 2),
        ("(7 / 2 <= 3)", false, 2),
        ("(7 / 2 > 3)", true, 2),
        ("(7 / 2 >= 3)", true, 2),
        ("(6 / 2 <= 3)", true, 2),
        ("(6 / 2 >= 3)", true, 2),
        ("(0.1 + 0.2 == 0.3)", true, 2),
        ("(1 / 10 == 0.1)", true, 2),
        ("(-1 / 3 < -0.3)", true, 2),
        ("(18446744073709551616 > 18446744073709551615)", true, 1),
        ("(9007199254740993 / 9007199254740992 > 1)", true, 2),
        ("((1 / 3 < 1 / 2) == (7 / 2 > 3))", true, 6),
        ("((7 / 2 < 3) || (0.1 + 0.2 == 0.3))", true, 5),
        ("(false && (1 / 3 < 1 / 2))", false, 4),
        ("(true || (1 / 3 > 1 / 2))", true, 4),
        ("((7u8 / 2 < 7 / 2 * 2) && (7 / 2 > 3))", true, 7),
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
fn anonymous_comparisons_cannot_hide_undefined_values_or_landed_operands() {
    let tree = Sources::new();
    let root = tree.package("root");
    for expression in [
        "(1 / 0 < 2)",
        "(1 < 2 / 0)",
        "(0 / 0 == 0)",
        "(false && (1 / 0 < 2))",
        "(true || (1 < 2 / 0))",
        "(1 / 2 < 1u8)",
        "(true || (1u8 > 1 / 2))",
        "(1u8 < 1u64)",
        "(true || (255u8 < 256))",
        "(1.0f64 < 2.0f64)",
        "(7 % 2 < 2)",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{FLAG} {}", keep("keep", expression)),
        );
        assert!(
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .is_err(),
            "{expression}"
        );
    }
}

#[test]
fn anonymous_comparisons_keep_exact_selection_and_runtime_shadow_rejection() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (declarations, expression) in [
        (
            "operator < u64::less(left: u64, right: u64) -> bool;",
            "(1 / 3 < 1 / 2)",
        ),
        (
            "operator == u64::equal(left: u64, right: u64) -> bool;",
            "(true || (1 / 3 == 1 / 2))",
        ),
        (
            "operator + u64::add(left: u64, right: u64) -> u64;",
            "(true || (1 + 2 < 4))",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{FLAG} {declarations} {}", keep("keep", expression)),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err(
                    "anonymous operands cannot invent builtin meaning under authored selection",
                );
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("requires exact authored selection")),
            "{diagnostics:?}"
        );
    }
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{FLAG} const LIMIT: u64 = 2; machine keep(LIMIT: u64, value: Flag<(true || (1 / 2 < LIMIT))>) {{}}"
        ),
    );
    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("runtime shadow remains a runtime operand");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("original lexical scope")),
        "{diagnostics:?}"
    );
}

#[test]
fn anonymous_comparison_data_fields_match_boolean_literal_instances() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{FLAG} data Holder {{ value: Flag<(0.1 + 0.2 == 0.3)>; }}
         machine read(holder: &Holder) -> Flag<true> {{ holder.value }}"
        ),
    );
    compile(&root, root_inputs(&root));
}
