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
    let expression = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| matches!(node, ExpressionNode::Match(_)).then_some(handle))
        .expect("Match root");
    (program, expression)
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
