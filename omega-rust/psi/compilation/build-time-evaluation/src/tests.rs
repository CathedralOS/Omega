use crate::{
    BuildTimeEvaluationRequest, BuildTimeSources, PureEvaluationRecord, evaluate_pre_resolution,
};

#[test]
fn an_empty_frontier_preserves_its_input_without_execution_receipts() {
    let original = syntax_trees::SyntaxTrees::new(source::SourceId(17));
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: original.clone(),
        source_context: BuildTimeSources::default(),
    })
    .expect("no source normalization work");
    let (returned, pending) = evaluated.into_syntax_and_pre_check();
    assert!(returned == original);
    let limits = compiler_storage::RecordLimits::default();
    let bytes = compiler_storage::encode(&pending, limits).expect("actual empty continuation");
    let mut restored: crate::PreCheckEvaluation =
        compiler_storage::decode(&bytes, limits).expect("current source-input schema");
    let mut typed = typed_trees::TypedTrees::default();
    restored
        .rejoin_source_authority(&typed, None)
        .expect("known absent authority needs no invented source map");
    let pending = restored;
    let before = typed.clone();
    let record = pending
        .evaluate_or_defer_recorded(&mut typed)
        .expect("no typed work");
    assert!(typed == before);
    assert!(record.pending.is_none());
    assert!(record.accounting.receipts().is_empty());
}

#[test]
fn a_nonempty_source_roster_is_refused_without_discarding_input() {
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax_trees::SyntaxTrees::new(source::SourceId(17)),
        source_context: BuildTimeSources::default(),
    })
    .expect("empty syntax");
    let (_, pending) = evaluated.into_syntax_and_pre_check();
    let mut typed = typed_trees::TypedTrees::default();
    typed.push_data_definition(Default::default());
    let before = typed.clone();
    let errors = pending
        .evaluate(&mut typed)
        .expect_err("source checking is not an empty job");
    assert!(errors[0].message.contains("no product route yet"));
    assert!(typed == before);
}

#[test]
fn retired_tree_receipts_cannot_be_decoded_as_canonical_scalar_receipts() {
    let limits = compiler_storage::RecordLimits::default();
    // The old enum reserved wire tag zero for a source/checked-tree receipt.
    // No old tree or supposed proof is constructed to test that boundary.
    let bytes = compiler_storage::encode(&0_u32, limits).expect("retired tag");
    let error = compiler_storage::decode::<PureEvaluationRecord>(&bytes, limits)
        .expect_err("retired source evaluation must not become canonical evidence");
    assert!(
        error
            .to_string()
            .contains("retired tree-evaluation receipt")
    );
}
