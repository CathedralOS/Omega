use crate::semantic::contract_fact_place;
use crate::semantic_calls::CallSite;
use crate::semantic_calls::call_site_argument_expressions;
use crate::semantic_calls::call_target_parameters;
use crate::semantic_calls::find_call_site;
use crate::semantic_calls::find_state_in_machine;
use checked_trees::{ContractCallFact, ContractProofFact};
use facts::{FactPlace, FactPlan};
mod expression;
mod place_builders;
mod receiver;
mod substitution;

pub(crate) use expression::{
    call_contract_argument_projection, instantiate_call_contract_expression_place,
    instantiate_outcome_contract_expression_place,
};

pub(crate) fn instantiate_call_contract_place(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    contract: &ContractProofFact,
) -> FactPlace {
    match program.proof_facts.get(contract.fact) {
        typed_trees::domain::ProofFact::Expression(expression) => {
            if let Some(place) = expression::instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                *expression,
            ) {
                return FactPlace::Place(place);
            }
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            if let Some(place) = expression::instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                membership.value,
            ) {
                return FactPlace::Place(place);
            }
        }
        typed_trees::domain::ProofFact::Proposition(_) => {}
    }

    let original_place = contract_fact_place(program, facts, contract);
    let FactPlace::Place(original_place_handle) = original_place else {
        return original_place;
    };

    let Some(substitution) =
        substitution::call_contract_place_substitution(program, facts, call, original_place_handle)
    else {
        return original_place;
    };

    let original_place = *facts.places.get(original_place_handle);
    let original_segments: Vec<_> = facts
        .place_segments
        .span_or_empty(original_place.segments)
        .to_vec();

    let mut segments = substitution.segments;
    segments.extend(original_segments);
    FactPlace::Place(place_builders::append_place_with_segments(
        facts,
        substitution.root,
        &segments,
    ))
}
pub(crate) use place_builders::{
    append_place_from_name_path, append_place_segment, append_place_with_segments,
    canonical_place_to_fact_place_in_state, resolve_place_member_symbol,
};
pub(crate) use receiver::receiver_place_for_call;
