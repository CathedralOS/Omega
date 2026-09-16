//! Fixtures shared by the borrow check tests.

mod lifetime_results;
mod mutable_overlaps;
mod persistent_storage;
mod view_returns;

use crate::borrow::build_borrow_facts;
use crate::checks::check_unretained_borrow_fixture_facts as check_checked_facts;
use crate::flow::build_domain_facts;
use crate::flow::build_flow_facts;
use crate::proof::build_proof_facts;
use crate::semantic::build_semantic_facts;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

/// Run a source program through the full frontend check, returning the borrow
/// checker's verdict.
pub(super) fn check_program(source: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
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
