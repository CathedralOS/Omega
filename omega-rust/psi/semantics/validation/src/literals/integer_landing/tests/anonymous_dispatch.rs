use super::*;
use typed_trees::expression::TableMatchExpression;

fn parsed_dispatch(subject: &str, arms: &str) -> (TypedTrees, TableMatchExpression) {
    parsed_dispatch_result(subject, arms, "u64")
}

fn parsed_dispatch_result(
    subject: &str,
    arms: &str,
    result: &str,
) -> (TypedTrees, TableMatchExpression) {
    let source = format!(
        "machine call() -> u64 {{ 1 }} machine choose() -> {result} {{ match {subject} {{ {arms} }} }}"
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolution");
    let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typing");
    let dispatch = program
        .expression_table
        .expression_entries()
        .find_map(|(_, node)| {
            if let ExpressionNode::Match(dispatch) = node {
                Some(*dispatch)
            } else {
                None
            }
        })
        .expect("source Match");
    (program, dispatch)
}

#[test]
fn width_gate_does_not_assign_a_carrier_to_anonymous_comparison_inputs() {
    for (subject, arms) in [
        (
            "340282366920938463463374607431768211456",
            "340282366920938463463374607431768211456 -> 7, _ -> 9",
        ),
        (
            "18446744073709551616 / 18446744073709551616",
            "1 -> 7, _ -> 9",
        ),
        (
            "1",
            "1 -> 7, 340282366920938463463374607431768211456 -> 9, _ -> 0",
        ),
    ] {
        let (program, _) = parsed_dispatch(subject, arms);
        let mut diagnostics = Vec::new();
        crate::literals::validate_literal_widths(&program, &mut diagnostics);
        assert!(diagnostics.is_empty(), "{subject}: {arms}: {diagnostics:?}");
    }
}

#[test]
fn width_gate_keeps_all_result_arms_including_unselected_large_values() {
    for result in ["u8", "u64"] {
        for arms in [
            "1 -> 18446744073709551616, _ -> 7",
            "1 -> 7, _ -> 18446744073709551616",
        ] {
            let (program, _) = parsed_dispatch_result("1", arms, result);
            let mut diagnostics = Vec::new();
            crate::literals::validate_literal_widths(&program, &mut diagnostics);
            assert!(!diagnostics.is_empty(), "{result}: {arms}");
        }
    }
}

#[test]
fn width_gate_does_not_skip_a_previously_typed_subject_or_pattern() {
    for (subject, arms) in [
        ("18446744073709551616u64", "_ -> 7"),
        ("1", "1 -> 7, 18446744073709551616u64 -> 9, _ -> 0"),
    ] {
        let (program, _) = parsed_dispatch(subject, arms);
        let mut diagnostics = Vec::new();
        crate::literals::validate_literal_widths(&program, &mut diagnostics);
        assert!(!diagnostics.is_empty(), "{subject}: {arms}");
    }
}

#[test]
fn comparison_input_custody_does_not_bless_a_shared_runtime_root() {
    let (mut program, dispatch) =
        parsed_dispatch("340282366920938463463374607431768211456", "_ -> 7");
    let mut diagnostics = Vec::new();
    crate::literals::validate_literal_widths(&program, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "call")
        .expect("ordinary runtime machine");
    let state = &program.machine_states(machine)[0];
    let (statement, _) = program
        .statement_table
        .iter_statements(state.statement_nodes)
        .next()
        .expect("runtime result");
    *program.statement_table.statement_mut(statement) =
        typed_trees::statement::StatementNode::Expression(dispatch.subject);
    crate::literals::validate_literal_widths(&program, &mut diagnostics);
    assert!(
        !diagnostics.is_empty(),
        "shared handle remains a checked runtime result"
    );
}

#[test]
fn wildcard_does_not_erase_subject_evaluation_or_prior_landing() {
    for subject in ["1u64", "1.0f64", "call()", "1 / 0"] {
        let (program, dispatch) = parsed_dispatch(subject, "_ -> 7");
        assert!(
            select_anonymous_numeric_match_arm(&program, &dispatch, |_| true).is_none(),
            "{subject}"
        );
    }
}

#[test]
fn exact_numeric_selection_is_ordered_and_does_not_claim_coverage() {
    for (subject, arms, ordinal) in [
        ("7 / 2", "3 -> 99, 14 / 4 -> 7, 7 / 2 -> 9", Some(1)),
        ("0.1 * 35", "7 / 2 -> 7, _ -> 9", Some(0)),
        ("7 / 2", "3 -> 99", None),
        ("7 / 2", "_ -> 7, 7 / 2 -> 9", Some(0)),
    ] {
        let (program, dispatch) = parsed_dispatch(subject, arms);
        let expected = ordinal
            .map(|ordinal| program.expression_table.match_arms(dispatch.arms)[ordinal].value);
        assert_eq!(
            select_anonymous_numeric_match_arm(&program, &dispatch, |_| true),
            expected,
            "{subject}: {arms}"
        );
    }
}

#[test]
fn selection_rejects_denied_subject_or_tested_pattern_arithmetic() {
    for (subject, arms) in [("7 / 2", "_ -> 7"), ("3", "6 / 2 -> 7, _ -> 9")] {
        let (program, dispatch) = parsed_dispatch(subject, arms);
        let mut checked = Vec::new();
        assert!(
            select_anonymous_numeric_match_arm(&program, &dispatch, |expression| {
                checked.push(expression);
                false
            })
            .is_none()
        );
        assert!(
            !checked.is_empty(),
            "arithmetic must consult selected meaning"
        );
    }
}

#[test]
fn selection_does_not_evaluate_result_bodies_or_later_patterns() {
    let (program, dispatch) = parsed_dispatch("1", "1 -> call(), call() -> 1 / 0, _ -> call()");
    let selected = program.expression_table.match_arms(dispatch.arms)[0].value;
    assert!(matches!(
        program.expression_table.expression(selected),
        ExpressionNode::Call(_)
    ));
    assert_eq!(
        select_anonymous_numeric_match_arm(&program, &dispatch, |_| panic!("no tested arithmetic")),
        Some(selected)
    );
}

#[test]
fn stale_arm_span_cannot_select_a_wildcard() {
    let (program, mut dispatch) = parsed_dispatch("1", "_ -> 7");
    let start = dispatch.arms.start();
    dispatch.arms = arena::HandleSpan::from_parts(
        arena::Handle::from_parts(start.arena_index(), start.generation() + 1),
        dispatch.arms.count(),
    );
    assert!(select_anonymous_numeric_match_arm(&program, &dispatch, |_| true).is_none());
}
