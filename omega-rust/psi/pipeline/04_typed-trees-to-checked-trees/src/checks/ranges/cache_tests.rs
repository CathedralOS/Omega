use checked_trees::BorrowFacts;
use typed_trees::TypedTrees;

pub(super) fn range_flow_fixture(
    program: &TypedTrees,
    borrows: &BorrowFacts,
) -> checked_trees::FlowFacts {
    let plan = proof::obligations::build_proof_plan(program);
    let proof = crate::proof::build_proof_facts(program, &plan, borrows);
    let mut semantic = crate::semantic::facts::build_semantic_facts(program, &proof);
    let domains = crate::flow::build_domain_facts(program, &semantic);
    let operational = validation::infer_operational_may(program);
    let mut flow = crate::flow::build_flow_facts(
        program,
        borrows,
        &proof,
        &mut semantic,
        &domains,
        &operational,
    );
    crate::facts::review_sources::bind_checked_body_call_source_spans(program, &mut flow)
        .expect("bind exact range fixture call identities as production does");
    flow
}
