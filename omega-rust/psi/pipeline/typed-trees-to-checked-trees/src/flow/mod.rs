mod borrow_lifetimes;
mod boundaries;
mod builder;
mod call_phases;
pub(crate) mod calls;
mod carried_semantic_dependencies;
mod common;
mod constraints;
mod context;
mod domain;
mod exits;
mod expression;
mod mutation;
mod operator_calls;
mod ownership;
mod place;
mod reach;
mod reference_places;
mod value_origins;
pub(crate) use reference_places::{
    call_result_sources, local_reference_candidate_storages_at_call,
    local_reference_storage_at_call, local_reference_storage_before_statement,
    reference_expression_storage_places, reference_result_candidates_before_statement,
};
pub(crate) use value_origins::{
    value_origin_at_call, value_origin_at_call_resolving, value_origin_before_statement,
};
mod state;
mod state_values;
mod statements;

mod entry_origins;
mod transfers;

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
pub(crate) use calls::call_result_qualification_identities;
pub(crate) use calls::call_target_return_type;
pub(crate) use carried_semantic_dependencies::derive_checked_semantic_dependencies;
pub(crate) use common::append_constraint_ref;
pub(crate) use common::proof_contract_call;
use common::{
    append_flow_contexts, append_flow_contexts_for_points, append_place_segments,
    appended_span_since, borrow_state_fact, project_constraint_refs_to_active_contexts,
    retained_constraint_refs, retained_flow_contexts,
};
use constraints::{
    append_contiguous_borrow_access_constraints, append_contiguous_borrow_root_constraints,
};
use context::FlowBuildContext;
pub(crate) use domain::build_domain_facts;
use domain::filter_contexts_after_place_mutations;
pub(crate) use domain::relative_place_segments_from_expression;
use exits::append_state_exit_facts;
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
    canonical_place_joined_segments_may_overlap, canonical_place_overlaps_joined_segments,
    canonical_place_overlaps_segments, canonical_place_segments_equal,
    canonical_place_segments_may_overlap, effective_member_symbol, expression_place_type_reference,
    expression_type_symbol, index_place_segment, literal_argument_access_places,
    literal_value_path_is_inactive, literal_value_projections, normalize_attached_place_root,
    place_case_has_value, place_cases_are_selected, place_segment_has_unresolved_identity,
    push_field_place_segments, resolve_member_symbol_from_type_symbol, symbol_type_symbol,
};
use reach::attach_reach_summaries;
use state::build_state_flow_fact;
use statements::append_state_statement_flow_facts;
use transfers::propagate_statement_transfers;
