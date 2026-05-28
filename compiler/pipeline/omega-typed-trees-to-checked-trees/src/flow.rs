use crate::context::*;
use crate::lookup::statement_call_receiver_path;
use crate::{
    CallSite, call_site_argument_expressions, contract_target_from_state_symbol, find_call_site,
    find_state, find_state_in_machine,
};

mod borrow_lifetimes;
mod builder;
mod calls;
mod common;
mod constraints;
mod context;
mod domain;
mod exits;
mod mutation;
mod place;
mod transfers;

use borrow_lifetimes::{filter_expired_borrow_loans, filter_reassigned_borrow_loans};
pub(crate) use builder::build_flow_facts;
use calls::build_call_flow_fact;
use common::{
    append_constraint_ref, append_flow_contexts_for_points, append_place_segments,
    append_semantic_constraints_for_points, appended_span_since, borrow_state_fact,
    clone_constraint_refs, clone_flow_contexts, effects_call, effects_machine, effects_state,
    project_constraint_refs_to_active_contexts, proof_contract_call,
};
use constraints::{
    append_contiguous_borrow_access_constraints, append_contiguous_borrow_root_constraints,
};
use context::FlowBuildContext;
pub(crate) use domain::build_domain_facts;
use domain::filter_contexts_after_place_mutations;
use exits::append_state_exit_facts;
use mutation::call_may_mutate_contract_state;
pub(crate) use mutation::{
    StateMutationSummaryCache, call_mutated_places, statement_mutated_place,
};
#[allow(unused_imports)]
pub(crate) use place::{
    CanonicalPlace, canonical_place_from_expression, canonical_place_from_expression_in_state,
    canonical_place_from_semantic_place, canonical_place_from_symbol,
    canonical_place_joined_segments_may_overlap, canonical_place_overlaps_joined_segments,
    canonical_place_overlaps_segments, canonical_place_segments_equal,
    canonical_place_segments_may_overlap, effective_member_symbol, expression_type_symbol,
    resolve_member_symbol_from_type_symbol, symbol_type_symbol,
};
use transfers::propagate_statement_transfers;
