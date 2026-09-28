use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program_result;

mod case_dispatch;
mod constant_index_projection;
mod float_patterns;
mod owned_results;
mod semantic_results;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_typed_trees(typed_program_result(source)?, &CheckingRequest::settled())
}

#[test]
fn match_default_retains_failed_pattern_premise_for_its_call() {
    check(
        "machine nonzero(value: i64) -> i64 requires value != 0 { value }
         machine choose(value: i64) -> i64 {
             match value { 0 -> 7 _ -> nonzero(value) }
         }",
    )
    .expect("the default arm is reached only when value differs from zero");
}

#[test]
fn match_arm_premises_keep_the_exact_subject() {
    let diagnostics = check(
        "machine nonzero(value: i64) -> i64 requires value != 0 { value }
         machine choose(value: i64, other: i64) -> i64 {
             match value { 0 -> 7 _ -> nonzero(other) }
         }",
    )
    .expect_err("a pattern on value cannot constrain other");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("requires")),
        "{diagnostics:#?}"
    );
}

#[test]
fn match_pattern_evidence_rejects_foreign_arm_identity() {
    let checked = check(
        "machine first(flag: bool) -> bool { match flag { true -> false _ -> true } }
         machine second(flag: bool) -> bool { match flag { true -> true _ -> false } }",
    )
    .expect("two independent dispatches");
    let program = &checked;
    let dispatches: Vec<_> = program
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            if let typed_trees::expression::ExpressionNode::Match(dispatch) = node {
                Some((expression, dispatch))
            } else {
                None
            }
        })
        .collect();
    assert!(dispatches.len() >= 2);
    let (expression, dispatch) = dispatches[0];
    let (_, foreign) = dispatches
        .iter()
        .find(|(_, other)| other.arms != dispatch.arms)
        .unwrap();
    let valid = facts::FactPayload::MatchPattern {
        expression,
        arm: dispatch.arms.start(),
        matched: true,
    };
    assert!(valid.match_pattern_comparison(program).is_some());
    let foreign = facts::FactPayload::MatchPattern {
        expression,
        arm: foreign.arms.start(),
        matched: true,
    };
    assert!(foreign.match_pattern_comparison(program).is_none());
}
