use super::{LARGE_ARGUMENT, anonymous_integer_landing_warnings, typed, width_grants};
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
};
use typed_trees::statement::StatementNode;

fn first_match(program: &TypedTrees) -> (ExpressionHandle, TableMatchExpression) {
    program
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| match node {
            ExpressionNode::Match(dispatch) => Some((handle, *dispatch)),
            _ => None,
        })
        .expect("fixture dispatch")
}

#[test]
fn match_result_arms_receive_the_actual_integer_destination() {
    for body in [
        format!("match subject {{ 0 -> {LARGE_ARGUMENT}, _ -> {LARGE_ARGUMENT} }}"),
        format!(
            "let result: u64 = match subject {{ 0 -> {LARGE_ARGUMENT}, _ -> {LARGE_ARGUMENT} }}; result"
        ),
        format!("accept(match subject {{ 0 -> {LARGE_ARGUMENT}, _ -> {LARGE_ARGUMENT} }})"),
    ] {
        let source = format!(
            "machine accept(value: u64) -> u64 {{ value }} machine choose(subject: u64) -> u64 {{ {body} }}"
        );
        assert_eq!(width_grants(&typed(&source)).len(), 4, "{source}");
    }
}

#[test]
fn nested_match_and_array_result_destinations_compose() {
    for (result_type, expression) in [
        (
            "u64",
            format!(
                "match subject {{ 0 -> match subject {{ 0 -> {LARGE_ARGUMENT}, _ -> {LARGE_ARGUMENT} }}, _ -> {LARGE_ARGUMENT} }}"
            ),
        ),
        (
            "[u64; 1]",
            format!(
                "match subject {{ 0 -> [match subject {{ 0 -> {LARGE_ARGUMENT}, _ -> {LARGE_ARGUMENT} }}], _ -> [{LARGE_ARGUMENT}] }}"
            ),
        ),
        (
            "[u64; 1]",
            format!(
                "[match subject {{ 0 -> match subject {{ 0 -> {LARGE_ARGUMENT}, _ -> {LARGE_ARGUMENT} }}, _ -> {LARGE_ARGUMENT} }}]"
            ),
        ),
    ] {
        let source = format!("machine choose(subject: u64) -> {result_type} {{ {expression} }}");
        assert_eq!(width_grants(&typed(&source)).len(), 6, "{source}");
    }
}

#[test]
fn every_match_result_arm_reports_its_fractional_landing_origin() {
    let source = "machine choose(subject: bool) -> u64 { match subject { true -> 7 / 2 * 2, false -> 0.1 * 90 } }";
    let warnings = anonymous_integer_landing_warnings(&typed(source));
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(
        warnings
            .iter()
            .any(|warning| warning.message.contains("7/2")
                && warning.message.contains("integer `7`"))
    );
    let decimal = warnings
        .iter()
        .find(|warning| warning.message.contains("1/10") && warning.message.contains("integer `9`"))
        .expect("unselected arm warning remains authored");
    let offset = source.find("0.1").unwrap();
    assert_eq!(
        decimal.source_span.unwrap().span,
        source::Span::new(offset, offset + 3)
    );
}

#[test]
fn a_shared_whole_match_cannot_borrow_an_integer_destination_for_float_use() {
    let mut program = typed(&format!(
        "machine choose(subject: u64) {{ let integers: u64 = match subject {{ 0 -> {LARGE_ARGUMENT}, _ -> 0 }}; let floating: f64 = 0.0; }}"
    ));
    assert_eq!(width_grants(&program).len(), 2);
    let (dispatch, _) = first_match(&program);
    let state = &program.machine_states(&program.machines()[0])[0];
    let statement = program
        .statement_table
        .iter_statements(state.statement_nodes)
        .find_map(|(handle, statement)| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "floating" => Some(handle),
            _ => None,
        })
        .expect("float destination");
    let StatementNode::LocalData(local) = program.statement_table.statement_mut(statement) else {
        panic!("local");
    };
    local.initial_value = dispatch;
    assert!(
        width_grants(&program).is_empty(),
        "one shared result cannot erase its independent float use"
    );
}

#[test]
fn match_inputs_do_not_inherit_width_grants_from_shared_result_roots_or_leaves() {
    let source = format!(
        "machine choose(subject: u64) -> u64 {{ match subject {{ 0 -> {LARGE_ARGUMENT}, _ -> 0 }} }}"
    );
    let program = typed(&source);
    assert_eq!(width_grants(&program).len(), 2);
    let (root, dispatch) = first_match(&program);
    let result = program.expression_table.match_arms(dispatch.arms)[0].value;
    let ExpressionNode::Binary(quotient) = program.expression_table.expression(result) else {
        panic!("exact quotient");
    };
    for (shared, expected_grants) in [(result, 0), (quotient.left, 1)] {
        for use_as_subject in [true, false] {
            let mut changed = program.clone();
            let mut dispatch = dispatch;
            if use_as_subject {
                dispatch.subject = shared;
            } else {
                let mut arms = changed.expression_table.match_arms(dispatch.arms).to_vec();
                arms[0].pattern = MatchPattern::Value(shared);
                dispatch.arms = changed.expression_table.insert_match_arms(arms);
            }
            *changed.expression_table.expression_mut(root) = ExpressionNode::Match(dispatch);
            let grants = width_grants(&changed);
            assert_eq!(
                grants.len(),
                expected_grants,
                "shared={shared:?}, subject={use_as_subject}"
            );
            assert!(!grants.contains(&shared));
        }
    }
}

#[test]
fn typed_call_inside_match_subject_keeps_its_independent_argument_destination() {
    let source = format!(
        "machine accept(value: u64) -> u64 {{ value }} machine choose() -> u64 {{ match accept({LARGE_ARGUMENT}) {{ 0 -> {LARGE_ARGUMENT}, _ -> 0 }} }}"
    );
    let program = typed(&source);
    let (_, dispatch) = first_match(&program);
    let ExpressionNode::Call(call) = program.expression_table.expression(dispatch.subject) else {
        panic!("subject call");
    };
    let argument = program.expression_table.expression_handles(call.arguments)[0];
    let ExpressionNode::Binary(quotient) = program.expression_table.expression(argument) else {
        panic!("call argument quotient");
    };
    let grants = width_grants(&program);
    assert_eq!(grants.len(), 4);
    assert!(grants.contains(&quotient.left));
    assert!(grants.contains(&quotient.right));
}

#[test]
fn unsupported_shared_match_result_does_not_erase_subject_call_argument_custody() {
    let mut program = typed(&format!(
        "machine accept(value: u64) -> u64 {{ value }} machine choose() {{ let integer: u64 = match accept({LARGE_ARGUMENT}) {{ 0 -> {LARGE_ARGUMENT}, _ -> 0 }}; let floating: f64 = 0.0; }}"
    ));
    assert_eq!(width_grants(&program).len(), 4);
    let (root, dispatch) = first_match(&program);
    let ExpressionNode::Call(call) = program.expression_table.expression(dispatch.subject) else {
        panic!("subject call");
    };
    let argument = program.expression_table.expression_handles(call.arguments)[0];
    let ExpressionNode::Binary(quotient) = program.expression_table.expression(argument) else {
        panic!("call argument quotient");
    };
    let call_leaves = [quotient.left, quotient.right];
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statement = program
        .statement_table
        .iter_statements(state.statement_nodes)
        .find_map(|(handle, statement)| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "floating" => Some(handle),
            _ => None,
        })
        .expect("float destination");
    let StatementNode::LocalData(local) = program.statement_table.statement_mut(statement) else {
        panic!("local");
    };
    local.initial_value = root;
    let grants = width_grants(&program);
    assert_eq!(
        grants.len(),
        2,
        "the independent call argument keeps its own destination"
    );
    for leaf in call_leaves {
        assert!(grants.contains(&leaf));
    }
}

#[test]
fn match_arm_call_keeps_its_own_argument_destination() {
    let source = format!(
        "machine accept(value: u64) -> u64 {{ value }} machine choose(subject: u64) -> u64 {{ match subject {{ 0 -> accept({LARGE_ARGUMENT}), _ -> 0 }} }}"
    );
    let program = typed(&source);
    let (_, dispatch) = first_match(&program);
    let result = program.expression_table.match_arms(dispatch.arms)[0].value;
    let ExpressionNode::Call(call) = program.expression_table.expression(result) else {
        panic!("arm call");
    };
    let argument = program.expression_table.expression_handles(call.arguments)[0];
    let ExpressionNode::Binary(quotient) = program.expression_table.expression(argument) else {
        panic!("call argument quotient");
    };
    let grants = width_grants(&program);
    assert_eq!(grants.len(), 2);
    assert!(grants.contains(&quotient.left));
    assert!(grants.contains(&quotient.right));
}

#[test]
fn stale_and_cyclic_match_result_trees_supply_no_width_grants() {
    let source = format!(
        "machine choose(subject: u64) -> u64 {{ match subject {{ _ -> {LARGE_ARGUMENT} }} }}"
    );
    let program = typed(&source);
    assert_eq!(width_grants(&program).len(), 2);
    let (root, dispatch) = first_match(&program);
    let result = program.expression_table.match_arms(dispatch.arms)[0].value;
    for replacement in [
        root,
        ExpressionHandle::from_parts(result.arena_index(), result.generation() + 1),
    ] {
        let mut changed = program.clone();
        let mut dispatch = dispatch;
        let mut arms = changed.expression_table.match_arms(dispatch.arms).to_vec();
        arms[0].value = replacement;
        dispatch.arms = changed.expression_table.insert_match_arms(arms);
        *changed.expression_table.expression_mut(root) = ExpressionNode::Match(dispatch);
        assert!(
            width_grants(&changed).is_empty(),
            "invalid result graph {replacement:?}"
        );
    }
}

#[test]
fn cyclic_result_diagnostic_query_retains_leaf_errors_without_an_interval_claim() {
    let mut program =
        typed("machine choose(flag: bool) -> u64 { match flag { true -> 7 / 2, _ -> 0 } }");
    let (root, mut dispatch) = first_match(&program);
    let mut arms = program.expression_table.match_arms(dispatch.arms).to_vec();
    arms[1].value = root;
    dispatch.arms = program.expression_table.insert_match_arms(arms);
    *program.expression_table.expression_mut(root) = ExpressionNode::Match(dispatch);
    let destination = program.machine_states(&program.machines()[0])[0].return_type;
    let mut diagnostics = Vec::new();
    assert!(
        crate::arithmetic_domains::validate_anonymous_integer_range(
            &program,
            destination,
            root,
            "dispatch result",
            &mut diagnostics,
        )
        .is_none()
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("`7/2` is not an integer"));
}
