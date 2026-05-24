use super::*;
use crate::lookup::statement_call_receiver_path;

mod builder;
mod common;
mod domain;
mod mutation;
mod place;

use common::{
    append_constraint_ref, append_flow_contexts_for_points, append_place_segments,
    append_semantic_constraints_for_points, appended_span_since, borrow_state_fact,
    clone_constraint_refs, clone_flow_contexts, effects_call, effects_machine, effects_state,
    project_constraint_refs_to_active_contexts, proof_contract_call,
};
use domain::filter_contexts_after_place_mutations;
pub(crate) use domain::build_domain_facts;
pub(crate) use mutation::{call_mutated_places, StateMutationSummaryCache};
pub(crate) use place::{
    canonical_place_from_expression, canonical_place_from_symbol,
    canonical_place_from_semantic_place, canonical_place_overlaps_joined_segments,
    canonical_place_overlaps_segments, canonical_place_segments_equal,
    effective_member_symbol, expression_type_symbol, symbol_type_symbol, CanonicalPlace,
};
use mutation::{
    call_may_mutate_contract_state, statement_mutated_place,
};
pub(crate) use builder::build_flow_facts;
