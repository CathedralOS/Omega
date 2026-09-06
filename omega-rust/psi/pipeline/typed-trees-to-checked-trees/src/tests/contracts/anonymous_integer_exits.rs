use super::*;

fn check_expression(expression: &str, target: &str, expected: &str, accepted: bool) {
    let source =
        format!("machine value() -> {target}\nensures result == {expected}\n{{ {expression} }}");
    match lower_typed_trees(parse_typed_trees(&source)) {
        Ok(_) => assert!(accepted, "unproved anonymous return accepted: {source}"),
        Err(diagnostics) => assert!(!accepted, "{diagnostics:#?}\n{source}"),
    }
}

fn combine_return_arms(program: &mut typed_trees::TypedTrees) {
    use typed_trees::statement::StatementNode;
    let machine = program.machines()[0].clone();
    let nodes = program.machine_states(&machine)[0].statement_nodes;
    let count = nodes.count() as usize;
    let StatementNode::Transition(other) = &program.statement_table.statements(nodes)[count - 1]
    else {
        panic!("false return arm")
    };
    let continuation = other.target;
    let StatementNode::Transition(first) =
        &mut program.statement_table.statements_mut(nodes)[count - 2]
    else {
        panic!("true return arm")
    };
    first.continuation = continuation;
    program.machine_states_mut(&machine)[0].statement_nodes =
        arena::HandleSpan::from_parts(nodes.start(), nodes.count() - 1);
}

#[test]
fn guarded_return_values_land_at_the_declared_destination() {
    for (expression, target, expected, fallback) in [
        ("(255 + 1) - 1", "u8", "255", "255"),
        ("(0 - 129) + 1", "i8", "-128", "-128"),
        (
            "(18446744073709551615 + 1) - 1",
            "u64",
            "18446744073709551615",
            "18446744073709551615",
        ),
        ("((255u8 as u8 in Wrapping) + 1) as u8", "u8", "0", "0"),
    ] {
        for accepted in [true, false] {
            let comparison = if accepted { "==" } else { "!=" };
            let source = format!(
                "machine value(flag: bool) -> {target} ensures result {comparison} {expected} {{ transition flag {{ true -> ({expression}) false -> ({fallback}) }} }}"
            );
            for combined in [false, true] {
                let mut program = parse_typed_trees(&source);
                if combined {
                    combine_return_arms(&mut program);
                }
                let result = lower_typed_trees(program);
                assert_eq!(
                    result.is_ok(),
                    accepted,
                    "combined={combined}: {source}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn guarded_scalar_returns_read_current_storage_and_keep_saved_values() {
    let source = "machine value(flag: bool) -> u8 ensures result == 7 { let mut current: u8 = 7; let saved: u8 = current; current = 8; transition flag { true -> saved false -> (current - 1) } }";
    for combined in [false, true] {
        let mut program = parse_typed_trees(source);
        if combined {
            combine_return_arms(&mut program);
        }
        lower_typed_trees(program)
            .unwrap_or_else(|diagnostics| panic!("combined={combined}: {source}: {diagnostics:?}"));
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
        for body in [
            format!("let landed: {target} = {expression};"),
            format!("let mut landed: {target} = {expression};"),
            format!("let mut landed: {target} = 0; landed = {expression};"),
        ] {
            let source = format!("machine value() {{ {body} }}");
            lower_typed_trees(parse_typed_trees(&source))
                .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        }
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
        for body in [
            format!("let landed: {target} = {expression};"),
            format!("let mut landed: {target} = {expression};"),
            format!("let mut landed: {target} = 0; landed = {expression};"),
        ] {
            let source = format!("machine value() {{ {body} }}");
            let diagnostics = lower_typed_trees(parse_typed_trees(&source))
                .expect_err("a local destination cannot discard its numeric obligations");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("does not fit destination")
                        || diagnostic.message.contains("may overflow")
                        || diagnostic.message.contains("declared range")
                        || diagnostic.message.contains("satisfies bounded target")
                }),
                "{source}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn anonymous_operator_meaning_requires_intact_selection_custody() {
    let mut program = parse_typed_trees("machine value() -> u8 { 3 + 4 }");
    let root = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            matches!(node, typed_trees::expression::ExpressionNode::Binary(_)).then_some(handle)
        })
        .unwrap();
    assert!(validation::has_anonymous_operator_meaning(&program, root));
    program.retain_authored_declaration_selections(Default::default());
    assert!(!validation::has_anonymous_operator_meaning(&program, root));

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
            matches!(node, typed_trees::expression::ExpressionNode::Binary(_)).then_some(handle)
        })
        .unwrap();
    assert!(!validation::has_anonymous_operator_meaning(&program, root));
}

#[test]
fn destination_landing_preserves_each_shared_call_argument_destination() {
    for body in [
        "(18446744073709551615 + 1) - 1",
        "transition true { true -> ((18446744073709551615 + 1) - 1) false -> 0u64 }",
        "let landed: u64 = (18446744073709551615 + 1) - 1; landed",
        "let mut landed: u64 = (18446744073709551615 + 1) - 1; landed",
        "let mut landed: u64 = 0; landed = (18446744073709551615 + 1) - 1; landed",
    ] {
        for (parameter_type, supported) in [("u64", true), ("f64", false)] {
            let mut program = parse_typed_trees(&format!(
                r#"
        machine good() -> u64 {{ {body} }}
        machine consume(value: {parameter_type}) {{}}
        machine bad() {{ consume(0); }}
    "#
            ));
            let large = program
                .expression_table
                .iter_expressions()
                .find_map(|(handle, node)| match node {
                    typed_trees::expression::ExpressionNode::Integer(literal)
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
                typed_trees::statement::StatementNode::Call(call) => call.arguments,
                _ => panic!("expected statement call"),
            };
            program
                .statement_table
                .set_expression_handle_at_offset(arguments, 0, large);
            let result = validation::validate_program(&program);
            if supported {
                result.unwrap_or_else(|diagnostics| {
                    panic!("exact u64 argument destination: {diagnostics:#?}")
                });
                continue;
            }
            // Integer argument landing now handles the ordinary u64 call above.
            // Floating argument landing is a distinct, still-unhandled width
            // consumer; sharing with an integer exit cannot grant it that width.
            let diagnostics =
                result.expect_err("unhandled floating use must retain width rejection");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("exceeds the i64 range")),
                "{diagnostics:#?}"
            );
        }
    }
}
