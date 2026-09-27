use super::{LARGE_ARGUMENT, typed, width_grants};
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
};

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
        crate::proof_contracts::arithmetic_domains::validate_anonymous_integer_range(
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
