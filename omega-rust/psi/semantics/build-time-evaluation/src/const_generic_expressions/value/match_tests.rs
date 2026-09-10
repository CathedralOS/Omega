use super::evaluate;
use source_files_to_tokens::Lexer;
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    types::PrimitiveType,
};

fn program(body: &str) -> (TypedTrees, ExpressionHandle) {
    let source = format!("machine choose() -> u8 {{ {body} }}");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(
        &Lexer::new(&source).tokenize().expect("tokens"),
    )
    .expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
    let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    let state = &program.machine_states(&program.machines()[0])[0];
    let [typed_trees::statement::StatementNode::Expression(expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        panic!("one source expression");
    };
    let expression = *expression;
    (program, expression)
}

#[test]
fn surrounding_arithmetic_warns_for_each_exact_fractional_result_path() {
    for (body, destination, expected) in [
        (
            "((match true { true -> 7 / 2, false -> 9 / 2 }) * 2) + 0u8",
            PrimitiveType::U8,
            "7",
        ),
        (
            "false && (((match (1u8 / 0 == 0) { true -> 7 / 2, false -> 9 / 2 }) * 2) == 0u8)",
            PrimitiveType::Bool,
            "false",
        ),
        (
            "true || (((match (1u8 / 0 == 0) { true -> 7 / 2, false -> 9 / 2 }) * 2) == 0u8)",
            PrimitiveType::Bool,
            "true",
        ),
    ] {
        let (program, expression) = program(body);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let (value, warnings) = evaluate(&program, machine, state, expression, destination)
            .expect("all-arm landing, no skipped subject evaluation");
        assert_eq!(value.display, expected);
        assert_eq!(warnings.len(), 2, "{body}: {warnings:?}");
        for (origin, result) in [("7/2", "7"), ("9/2", "9")] {
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.message.contains(&format!("`{origin}`"))
                        && warning.message.contains(&format!("integer `{result}`"))),
                "{warnings:?}"
            );
        }
        assert_ne!(warnings[0].source_span, warnings[1].source_span);
    }
}

#[test]
fn nested_fractional_result_edges_restore_each_exact_diagnostic_context() {
    let (program, expression) = program(
        "(match true { true -> (match false { true -> 7 / 2, false -> 9 / 2 }), false -> (match true { true -> 11 / 2, false -> 13 / 2 }) }) * 2",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (value, warnings) = evaluate(&program, machine, state, expression, PrimitiveType::U8)
        .expect("nested alternatives share no stale arm selection");
    assert_eq!(value.display, "9");
    assert_eq!(warnings.len(), 4, "{warnings:?}");
    for expected in [7, 9, 11, 13] {
        assert!(
            warnings.iter().any(
                |warning| warning.message.contains(&format!("`{expected}/2`"))
                    && warning.message.contains(&format!("integer `{expected}`"))
            ),
            "{warnings:?}"
        );
    }
}

#[test]
fn match_static_landing_retains_each_fractional_warning_once() {
    let (program, expression) = program("match true { true -> 7 / 2 * 2, false -> 0.1 * 90 }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let (value, warnings) =
        evaluate(&program, machine, state, expression, PrimitiveType::U8).expect("exact landings");
    assert_eq!(value.display, "7");
    assert_eq!(warnings.len(), 2);
    assert_ne!(warnings[0].source_span, warnings[1].source_span);
}

#[test]
fn match_scalar_probe_rejects_stale_children_and_cycles_before_type_readers() {
    let (original, expression) = program("match true { true -> 1, false -> 2 }");
    for replacement in [
        expression,
        ExpressionHandle::invalid(),
        ExpressionHandle::from_parts(
            expression.arena_index(),
            expression.generation().wrapping_add(1),
        ),
    ] {
        let mut program = original.clone();
        let ExpressionNode::Match(mut dispatch) =
            program.expression_table.expression(expression).clone()
        else {
            panic!("Match fixture");
        };
        dispatch.subject = replacement;
        *program.expression_table.expression_mut(expression) = ExpressionNode::Match(dispatch);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let error = evaluate(&program, machine, state, expression, PrimitiveType::U8)
            .expect_err("malformed subject");
        assert!(error.contains("invalid or cyclic"), "{error}");
    }
}

#[test]
fn match_wildcard_does_not_erase_undefined_anonymous_subject() {
    let (program, expression) = program("match (1 / 0) { _ -> 1 }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    assert!(evaluate(&program, machine, state, expression, PrimitiveType::U8).is_err());
}

#[test]
fn anonymous_match_subject_is_evaluated_once_across_ordered_patterns() {
    use super::{Value, match_dispatch::MatchSubject};
    use typed_trees::expression::MatchPattern;

    let (program, expression) =
        program("match 7 / 2 { (1 / 2) -> 0, (3 / 2) -> 0, (7 / 2) -> 1, _ -> 0 }");
    let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) else {
        panic!("Match fixture");
    };
    let patterns = program
        .expression_table
        .match_arms(dispatch.arms)
        .iter()
        .filter_map(|arm| {
            if let MatchPattern::Value(pattern) = arm.pattern {
                Some(pattern)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let operations = [dispatch.subject, patterns[0], patterns[1], patterns[2]];
    let mut evaluations = [0; 4];
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    {
        let mut builtin = |operand| {
            if let Some(ordinal) = operations
                .iter()
                .position(|operation| *operation == operand)
            {
                evaluations[ordinal] += 1;
            }
            validation::has_builtin_binary_expression_meaning(
                &program,
                machine,
                Some(state),
                operand,
            )
        };
        let subject = MatchSubject::new(
            &program,
            Value::Anonymous(dispatch.subject),
            &[],
            &mut builtin,
        )
        .expect("saved rational subject");
        for (pattern, expected) in patterns.into_iter().zip([false, false, true]) {
            assert_eq!(
                subject
                    .matches(&program, Value::Anonymous(pattern), &[], &mut builtin)
                    .expect("ordered pattern equality"),
                expected
            );
        }
    }
    assert_eq!(
        evaluations, [1; 4],
        "one subject evaluation and one per demanded pattern"
    );
}
