use crate::context::*;
use crate::semantic::contract_fact_place;
pub(crate) use crate::{
    CallSite, call_site_argument_expressions, call_target_parameters, find_call_site,
    find_state_in_machine,
};
mod expression;
mod place_builders;
mod receiver;
mod substitution;

pub(crate) use expression::{
    instantiate_call_contract_expression_place, instantiate_outcome_contract_expression_place,
};

pub(crate) fn instantiate_call_contract_place(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    contract: &ContractProofFact,
) -> FactPlace {
    match program.proof_facts.get(contract.fact) {
        psi_typed_trees::domain::ProofFact::Expression(expression) => {
            if let Some(place) = expression::instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                *expression,
            ) {
                return FactPlace::Place(place);
            }
        }
        psi_typed_trees::domain::ProofFact::Membership(membership) => {
            if let Some(place) = expression::instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                membership.value,
            ) {
                return FactPlace::Place(place);
            }
        }
        psi_typed_trees::domain::ProofFact::Proposition(_) => {}
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
