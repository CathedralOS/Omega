//! Fixtures shared by the borrow check tests.

mod lifetime_results;
mod mutable_overlaps;
mod persistent_storage;
mod premised_disjoint_writes;
mod view_returns;

use crate::borrow::build_borrow_facts;
use crate::checks::check_unretained_borrow_fixture_facts as check_checked_facts;
use crate::flow::build_domain_facts;
use crate::flow::build_flow_facts;
use crate::proof::build_proof_facts;
use crate::semantic::build_semantic_facts;
use crate::tests::front_end::typed_program;

/// Run a source program through the full frontend check, returning the borrow
/// checker's verdict.
pub(super) fn check_program(source: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };
    check_checked_facts(&typed, &facts)
}
