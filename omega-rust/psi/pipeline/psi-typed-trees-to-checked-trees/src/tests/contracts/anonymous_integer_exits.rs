use super::*;

fn check_expression(expression: &str, target: &str, expected: &str, accepted: bool) {
    let source =
        format!("machine value() -> {target}\nensures result == {expected}\n{{ {expression} }}");
    match lower_typed_trees(parse_typed_trees(&source)) {
        Ok(_) => assert!(accepted, "unproved anonymous return accepted: {source}"),
        Err(diagnostics) => assert!(!accepted, "{diagnostics:#?}\n{source}"),
    }
}

#[test]
fn anonymous_integer_arithmetic_lands_once_at_the_return_destination() {
    for (expression, target, value) in [
        ("3 + 4", "u8", "7"),
        ("(255 + 1) - 1", "u8", "255"),
        ("(0 - 1) + 2", "u8", "1"),
        ("(127 + 1) - 1", "i8", "127"),
        ("(0 - 128) + 1", "i8", "-127"),
    ] {
        check_expression(expression, target, value, true);
        check_expression(expression, target, "0", false);
    }
}

#[test]
fn anonymous_integer_arithmetic_keeps_full_unsigned_and_intermediate_precision() {
    for expression in [
        "(9223372036854775807 * 2) + 1",
        "(18446744073709551615 + 1) - 1",
        "(18446744073709551615 * 18446744073709551615) / 18446744073709551615",
    ] {
        check_expression(expression, "u64", "18446744073709551615", true);
        check_expression(expression, "u64", "18446744073709551614", false);
    }
}

#[test]
fn anonymous_integer_arithmetic_rejects_an_unrepresentable_final_value() {
    for (expression, target) in [
        ("255 + 1", "u8"),
        ("127 + 1", "i8"),
        ("0 - 1", "u8"),
        ("18446744073709551615 + 1", "u64"),
        ("1 / 0", "u8"),
    ] {
        check_expression(expression, target, "0", false);
    }
}

#[test]
fn previously_landed_integer_operations_keep_their_selected_width() {
    check_expression("((255u8 as u8 in Wrapping) + 1) as u8", "u8", "0", true);
    check_expression("((255u8 as u8 in Wrapping) + 1) as u8", "u8", "256", false);
    check_expression("(255u8 + 1u8) - 1u8", "u8", "255", false);
}

#[test]
fn anonymous_local_initializers_land_without_widths_on_intermediate_values() {
    for (expression, target) in [
        ("(255 + 1) - 1", "u8"),
        ("(0 - 1) + 2", "u8"),
        ("(0 - 129) + 1", "i8"),
        ("(18446744073709551615 + 1) - 1", "u64"),
        (
            "(18446744073709551615 * 18446744073709551615) / 18446744073709551615",
            "u64",
        ),
        ("(255 + 1) - 129", "u8 [0..=127]"),
    ] {
        let source = format!("machine value() {{ let landed: {target} = {expression}; }}");
        lower_typed_trees(parse_typed_trees(&source))
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    }
}

#[test]
fn anonymous_local_landing_enforces_final_carrier_and_refinement_ranges() {
    for (expression, target) in [
        ("255 + 1", "u8"),
        ("127 + 1", "i8"),
        ("0 - 1", "u8"),
        ("18446744073709551615 + 1", "u64"),
        ("(255 + 1) - 1", "u8 [0..=127]"),
        ("(255u8 + 1u8) - 1u8", "u8"),
    ] {
        let source = format!("machine value() {{ let landed: {target} = {expression}; }}");
        let diagnostics = lower_typed_trees(parse_typed_trees(&source))
            .expect_err("a local destination cannot discard its numeric obligations");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("does not fit destination")
                    || diagnostic.message.contains("may overflow")
                    || diagnostic.message.contains("declared range")
            }),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn anonymous_operator_meaning_requires_intact_selection_custody() {
    let mut program = parse_typed_trees("machine value() -> u8 { 3 + 4 }");
    let root = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            matches!(node, psi_typed_trees::expression::ExpressionNode::Binary(_)).then_some(handle)
        })
        .unwrap();
    assert!(psi_validation::has_anonymous_operator_meaning(
        &program, root
    ));
    program.retain_authored_declaration_selections(Default::default());
    assert!(!psi_validation::has_anonymous_operator_meaning(
        &program, root
    ));

    let program = parse_typed_trees(
        r#"
        boundary operator + u8::custom(left: u8, right: u8) -> u8;
        machine value() -> u8 { 3 + 4 }
    "#,
    );
    let root = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            matches!(node, psi_typed_trees::expression::ExpressionNode::Binary(_)).then_some(handle)
        })
        .unwrap();
    assert!(!psi_validation::has_anonymous_operator_meaning(
        &program, root
    ));
}

#[test]
fn destination_landing_does_not_bless_a_shared_literal_at_an_unhandled_call_site() {
    for body in [
        "(18446744073709551615 + 1) - 1",
        "let landed: u64 = (18446744073709551615 + 1) - 1; landed",
    ] {
        let mut program = parse_typed_trees(&format!(
            r#"
        machine good() -> u64 {{ {body} }}
        machine consume(value: u64) {{}}
        machine bad() {{ consume(0); }}
    "#
        ));
        let large = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, node)| match node {
                psi_typed_trees::expression::ExpressionNode::Integer(literal)
                    if literal.value_u64() == Some(u64::MAX) =>
                {
                    Some(handle)
                }
                _ => None,
            })
            .unwrap();
        let bad = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "bad")
            .unwrap();
        let state = &program.machine_states(bad)[0];
        let arguments = match &program.statement_table.statements(state.statement_nodes)[0] {
            psi_typed_trees::statement::StatementNode::Call(call) => call.arguments,
            _ => panic!("expected statement call"),
        };
        program
            .statement_table
            .set_expression_handle_at_offset(arguments, 0, large);
        let diagnostics = psi_validation::validate_program(&program)
            .expect_err("unhandled use must retain width rejection");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("exceeds the i64 range")),
            "{diagnostics:#?}"
        );
    }
}
