use crate::context::*;
use crate::{
    CallSite, call_site_argument_expressions, contract_target_from_state_symbol, find_call_site,
    find_state, find_state_in_machine,
};

mod borrow_lifetimes;
mod boundaries;
mod builder;
mod call_phases;
mod calls;
mod common;
mod constraints;
mod context;
mod domain;
mod exits;
mod mutation;
mod operator_calls;
mod ownership;
mod place;
mod reach;
mod state;
mod statements;
mod terminal_debug;
mod terminal_scalar;
mod transfers;

use borrow_lifetimes::{filter_expired_borrow_loans, filter_reassigned_borrow_loans};
use boundaries::append_call_boundary_edges;
#[cfg(test)]
pub(crate) use builder::build_flow_facts;
pub(crate) use builder::build_flow_facts_with_service_reaches;
use call_phases::{
    CallFlowContexts, apply_call_invalidations, build_call_entry_contexts,
    build_call_exit_contexts, build_call_requires_contexts,
};
use calls::build_call_flow_fact;
use common::{
    append_constraint_ref, append_flow_contexts_for_points, append_place_segments,
    append_semantic_constraints_for_points, appended_span_since, borrow_state_fact,
    clone_constraint_refs, clone_flow_contexts, project_constraint_refs_to_active_contexts,
    proof_contract_call,
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
use operator_calls::{
    append_operator_statement_ensures, operator_statement_call_mutated_places,
    resolve_operator_for_call, resolve_operator_statement_call,
};
pub(crate) use ownership::{
    DiscoveredMoveEvent, FlowOwnershipEventSource, discover_state_move_events,
    expression_type_reference_in_state, owned_method_receiver_place,
    project_type_reference_from_segments,
};
#[allow(unused_imports)]
pub(crate) use place::{
    CanonicalPlace, canonical_place_from_expression, canonical_place_from_expression_in_state,
    canonical_place_from_semantic_place, canonical_place_from_symbol,
    canonical_place_joined_segments_may_overlap, canonical_place_overlaps_joined_segments,
    canonical_place_overlaps_segments, canonical_place_segments_equal,
    canonical_place_segments_may_overlap, effective_member_symbol, expression_type_symbol,
    index_place_segment, push_field_place_segments, resolve_member_symbol_from_type_symbol,
    symbol_type_symbol,
};
use reach::attach_reach_summaries;
use state::build_state_flow_fact;
use statements::append_state_statement_flow_facts;
pub(crate) use terminal_debug::build_checked_terminal_debug_plans;
pub(crate) use terminal_scalar::{
    build_checked_scalar_graph_plans, build_checked_terminal_machine_selections,
};
use transfers::propagate_statement_transfers;
