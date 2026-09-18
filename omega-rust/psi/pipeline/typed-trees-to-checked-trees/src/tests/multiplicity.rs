//! Multiplicity tests: permission events and case guards, obligations and
//! state call results, producers and conditional payloads, and records,
//! nominal drops and fixed arrays.

mod borrowed_case_payloads;
mod borrowed_observations;
mod borrowed_restoration;
mod obligations_and_state_call_results;
mod owned_selection;
mod permission_events_and_case_guards;
mod producers_and_conditional_payloads;
mod records_nominal_drops_and_fixed_arrays;

use crate::lower_typed_trees;
use crate::tests::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve,
};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}
