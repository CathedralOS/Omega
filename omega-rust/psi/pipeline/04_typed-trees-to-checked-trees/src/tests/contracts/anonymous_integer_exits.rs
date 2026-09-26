use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

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
                let result = lower_typed_trees(program, &CheckingRequest::settled());
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
