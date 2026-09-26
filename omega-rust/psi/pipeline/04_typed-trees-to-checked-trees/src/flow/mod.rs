//! Flow facts: for every state, statement, call, and exit of every machine, the
//! semantic contexts and constraints active there, with the invalidation,
//! borrow-lifetime, boundary-edge, and control rows that record how they change.
//!
//! One entrance: `builder::build_flow_facts_with_service_reaches`, which
//! `crate::facts::build_check_facts` calls after `domain::build_domain_facts` has
//! recorded each domain's dependency paths. It builds every state through
//! `state::build_state_flow_fact`. While joins in `state_values` change a state's
//! incoming values, it rebuilds the changed states, then rebuilds every state once
//! more to publish complete output. If the incoming values do not settle, it
//! rebuilds every state once with all incoming values unknown. `reach` then
//! attaches service-reach, suspension, and blocking summaries.
//!
//! `state::build_state_flow_fact` runs one state in this order: declaration and
//! state-entry contexts (with incoming values from `state_values`) rebased by
//! `entry_origins`, each statement through `statements`, loans expired at state
//! exit (`borrow_lifetimes`), and the exit facts of a body that ends without a
//! transition (`exits`). For each statement, `statements` runs a transition's arms
//! (`exits`) or the statement's calls in execution order (`expression`, which
//! builds each call through `calls::build_call_flow_fact`), retires and activates
//! borrow loans, drops facts over the written places (`mutation`, `domain`), adds
//! operator `ensures` (`operator_calls`), and propagates the written value's facts
//! (`transfers`).
//!
//! The module list below groups the children by role. `carried_semantic_dependencies`
//! is not a flow-build step: the check pass runs it after execution finalization.

// The entrance and its sweep: the build state it carries, the incoming state
// values whose changes decide another sweep, and the reach summaries it attaches.
mod builder;
mod context;
mod reach;
mod state_values;

// One state's facts and the producers it runs for its entry, statements, and exits.
mod entry_origins;
mod exits;
mod expression;
mod operator_calls;
mod state;
mod statements;
mod transfers;

// One call's facts: the call builder, its phases, and boundary-trait edges.
mod boundaries;
mod call_phases;
pub(crate) mod calls;

// What a write drops: the places a statement or call writes, the facts over them
// and their domain dependencies, and borrow loans that expire or are overwritten.
mod borrow_lifetimes;
mod domain;
mod mutation;

// Places, ownership, and reference and value origins: queries also used outside
// `flow`.
mod ownership;
mod place;
mod reference_places;
mod value_origins;

// Shared helpers: reference-span append and filter rules, contiguous borrow
// constraint refs, and borrow and proof fact-row lookups.
mod constraints;
mod fact_rows;
mod reference_spans;

// Not a flow-build step: derived from the complete check facts.
mod carried_semantic_dependencies;

pub(crate) use reference_places::{
    call_result_sources, local_reference_candidate_storages_at_call,
    local_reference_storage_at_call, local_reference_storage_before_statement,
    reference_expression_storage_places, reference_result_candidates_before_statement,
};
pub(crate) use value_origins::{
    trace_value_origin_before_statement, value_origin_at_call, value_origin_at_call_resolving,
    value_origin_before_statement,
};

pub(crate) use borrow_lifetimes::{filter_expired_borrow_loans, filter_reassigned_borrow_loans};
use boundaries::append_call_boundary_edges;
#[cfg(test)]
pub(crate) use builder::build_flow_facts;
pub(crate) use builder::build_flow_facts_with_service_reaches;
#[cfg(test)]
pub(crate) use builder::tests::check_against_whole_pass;
use call_phases::{
    CallFlowContexts, append_call_referent_field_domain_facts, apply_call_invalidations,
    build_call_entry_contexts, build_call_exit_contexts, build_call_requires_contexts,
};
use calls::build_call_flow_fact;
pub(crate) use calls::call_parameter_qualification_identities;
pub(crate) use calls::call_result_qualification_identities;
pub(crate) use calls::call_target_return_type;
pub(crate) use calls::ensured_parameter_position;
pub(crate) use carried_semantic_dependencies::derive_checked_semantic_dependencies;
use constraints::{
    append_contiguous_borrow_access_constraints, append_contiguous_borrow_root_constraints,
};
use context::FlowBuildContext;
pub(crate) use domain::build_domain_facts;
use domain::filter_contexts_after_place_mutations;
pub(crate) use domain::relative_place_segments_from_expression;
use exits::append_state_exit_facts;
use fact_rows::borrow_state_fact;
pub(crate) use fact_rows::{enter_fact_row_scope, proof_contract_call};
pub(crate) use mutation::close_storage_places_over_aliases_with_resolver;
pub(crate) use mutation::origin_place;
pub(crate) use mutation::rebase_exact_local_place;
pub(crate) use mutation::{
    StateMutationSummaryCache, call_mutated_places, call_write_accesses,
    canonical_receiver_place_for_call_site, frame_storage_writes, place_from_origin_path,
    signature_ceiling_places, statement_mutated_place, statement_storage_writes,
};
use operator_calls::{
    append_operator_statement_ensures, operator_statement_call_mutated_places,
    resolve_operator_for_call, resolve_operator_statement_call,
};
pub(crate) use place::contextual_canonical_place_from_expression;
pub(crate) use reference_spans::append_constraint_ref;
use reference_spans::{
    append_flow_contexts, append_flow_contexts_for_points, append_place_segments,
    appended_span_since, project_constraint_refs_to_active_contexts, retained_constraint_refs,
    retained_flow_contexts,
};

/// One `CallFrameResolver` serves an entire immutable program window of the
/// check pass: construction rebuilds the top-level symbol index, so every
/// consumer down the call tree receives the shared resolver instead of
/// rebuilding it. A caller outside that context — a standalone query or a
/// test — passes `None` and the site performs the same private construction
/// it ran before the pass shared one, stored in `owned` for the call's
/// duration. A `None` result therefore means exactly what a failed private
/// construction meant: the resolver is unavailable and the site takes its
/// existing opaque/conservative outcome.
pub(crate) fn shared_call_frames_or<'program, 'a>(
    shared: Option<&'a validation::CallFrameResolver<'program>>,
    program: &'program typed_trees::TypedTrees,
    owned: &'a mut Option<validation::CallFrameResolver<'program>>,
) -> Option<&'a validation::CallFrameResolver<'program>> {
    if shared.is_some() {
        return shared;
    }
    *owned = validation::CallFrameResolver::new(program);
    owned.as_ref()
}

pub(crate) fn resolved_operator_statement_symbol(
    program: &typed_trees::TypedTrees,
    call: &typed_trees::statement::TableCall,
) -> Option<symbols::SymbolHandle> {
    operator_calls::resolve_operator_statement_call(program, call)
        .map(|resolved| resolved.operator.symbol)
}
pub(crate) use ownership::{
    DiscoveredMoveEvent, FlowOwnershipEventSource, canonical_place_type_reference,
    collection_element_type_reference, discover_state_move_events,
    expression_type_reference_in_state, normalized_event_place_root, owned_call_operand_places,
    owned_method_receiver_place, project_type_reference_from_segments,
};
#[allow(unused_imports)]
pub(crate) use place::{
    CanonicalPlace, canonical_place_from_expression, canonical_place_from_expression_in_state,
    canonical_place_from_semantic_place, canonical_place_from_symbol,
    canonical_place_joined_segments_may_overlap, canonical_place_overlaps_segments,
    canonical_place_segments_equal, canonical_place_segments_may_overlap, effective_member_symbol,
    expression_place_type_reference, expression_type_symbol, index_place_segment,
    literal_argument_access_places, literal_value_path_is_inactive, literal_value_projections,
    normalize_attached_place_root, place_case_has_value, place_cases_are_selected,
    place_segment_has_unresolved_identity, push_field_place_segments,
    resolve_member_symbol_from_type_symbol, symbol_type_symbol,
};
use reach::attach_reach_summaries;
use state::build_state_flow_fact;
use statements::append_state_statement_flow_facts;
use transfers::propagate_statement_transfers;
