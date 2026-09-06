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
pub(crate) use reference_places::{
    local_reference_storage_at_call, local_reference_storage_before_statement,
};
mod state;
mod state_values;
mod statements;
mod terminal_cleanup;
mod terminal_debug;
mod terminal_scalar;
mod terminal_unit;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_affine_cast_affine_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_affine_chain_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_affine_fork_join_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_cast_then_affine_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_cast_then_signed_affine_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_distinct_root_affine_fork_join_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_distinct_root_affine_product_join_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_mixed_add_subtract_chain_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_same_root_affine_divide_remainder_join_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_same_root_affine_product_join_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_shift_then_arithmetic_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_signed_affine_cast_affine_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_signed_affine_chain_cast_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::affine::exact_signed_affine_chain_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_affine_chain_cast_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_cast_chain_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_cast_then_offset_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_computed_prefix_cast_chain_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_computed_prefix_cast_chain_then_computed_suffix_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_computed_prefix_mixed_conversion_chain_then_computed_suffix_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_computed_prefix_widen_chain_then_computed_suffix_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_divide_remainder_chain_cast_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_mixed_shift_chain_cast_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_multiply_chain_cast_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_offset_chain_cast_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_shift_left_chain_cast_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::cast_chains::exact_shift_right_chain_cast_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::products::exact_cast_then_divide_remainder_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::products::exact_cast_then_multiply_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::products::exact_cast_then_signed_multiply_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::products::exact_runtime_divisor_chain_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::products::exact_signed_multiply_chain_cast_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::products::exact_signed_multiply_chain_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::shifts::exact_affine_shift_cast_sandwich_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::shifts::exact_arithmetic_then_shift_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::shifts::exact_cast_then_mixed_shift_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::shifts::exact_cast_then_shift_left_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::shifts::exact_cast_then_shift_right_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::shifts::exact_divide_remainder_cast_sandwich_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::shifts::exact_divide_remainder_cross_cast_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::shifts::exact_divide_remainder_cross_chain_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::shifts::exact_mixed_shift_chain_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::shifts::exact_shift_cast_shift_runtime_parameter_positions_for_test;
#[cfg(test)]
pub(crate) use terminal_unit::shared_convergence::shifts::exact_shift_left_chain_runtime_parameter_positions_for_test;
pub(crate) use terminal_unit::types::byte_sequence_carrier;

#[cfg(test)]
pub(crate) fn exact_two_field_record_projection_for_test(
    program: &typed_trees::TypedTrees,
    root_type: typed_trees::types::TypeReferenceHandle,
    moved_field: symbols::SymbolHandle,
    target_type: typed_trees::types::TypeReferenceHandle,
) -> Option<(String, String, String, String)> {
    terminal_unit::exact_two_field_record_projection(program, root_type, moved_field, target_type)
}
mod entry_origins;
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
pub(crate) use carried_semantic_dependencies::derive_checked_semantic_dependencies;
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
pub(crate) use domain::relative_place_segments_from_expression;
use exits::append_state_exit_facts;
use mutation::close_storage_places_over_aliases;
pub(crate) use mutation::{
    StateMutationSummaryCache, call_mutated_places, call_write_accesses,
    canonical_receiver_place_for_call_site, frame_storage_writes, statement_mutated_place,
    statement_storage_writes,
};
use operator_calls::{
    append_operator_statement_ensures, operator_statement_call_mutated_places,
    resolve_operator_for_call, resolve_operator_statement_call,
};

pub(crate) fn resolved_operator_statement_symbol(
    program: &typed_trees::TypedTrees,
    call: &typed_trees::statement::TableCall,
) -> Option<symbols::SymbolHandle> {
    operator_calls::resolve_operator_statement_call(program, call)
        .map(|resolved| resolved.operator.symbol)
}
pub(crate) use ownership::{
    DiscoveredMoveEvent, FlowOwnershipEventSource, canonical_place_type_reference,
    discover_state_move_events, expression_type_reference_in_state, normalized_event_place_root,
    owned_call_operand_places, owned_method_receiver_place, project_type_reference_from_segments,
};
#[allow(unused_imports)]
pub(crate) use place::{
    CanonicalPlace, canonical_place_from_expression, canonical_place_from_expression_in_state,
    canonical_place_from_semantic_place, canonical_place_from_symbol,
    canonical_place_joined_segments_may_overlap, canonical_place_overlaps_joined_segments,
    canonical_place_overlaps_segments, canonical_place_segments_equal,
    canonical_place_segments_may_overlap, effective_member_symbol, expression_type_symbol,
    index_place_segment, literal_argument_access_places, literal_value_projections,
    place_segment_has_unresolved_identity, push_field_place_segments,
    resolve_member_symbol_from_type_symbol, symbol_type_symbol,
};
use reach::attach_reach_summaries;
use state::build_state_flow_fact;
use statements::append_state_statement_flow_facts;
pub(crate) use terminal_cleanup::build_checked_structural_control_cleanup_plans;
pub(crate) use terminal_debug::build_checked_terminal_debug_plans;
pub(crate) use terminal_scalar::{
    build_checked_scalar_graph_plans, build_checked_terminal_machine_selections,
};
pub(crate) use terminal_unit::control::build_checked_structural_unit_control_plans;
pub(crate) use terminal_unit::returns::{
    build_checked_boundary_scalar_return_plans, build_checked_structural_call_return_plans,
    build_checked_structural_return_plans, build_checked_structural_scalar_return_plans,
};
pub(crate) use terminal_unit::{
    build_checked_nominal_affine_unit_cleanup_plans,
    build_checked_partial_affine_unit_cleanup_plans, build_checked_unit_effect_plans,
};
use transfers::propagate_statement_transfers;
