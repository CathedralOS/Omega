use super::*;
use crate::semantic::contract_fact_place;
mod expression;
mod receiver;
mod place_builders;
mod substitution;

pub(crate) fn instantiate_call_contract_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    contract: &ContractProofFact,
) -> FactPlace {
    match program.proof_facts.get(contract.fact) {
        omega_typed_trees::domain::ProofFact::Expression(expression) => {
            if let Some(place) = expression::instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                *expression,
            )
            {
                return FactPlace::Place(place);
            }
        }
        omega_typed_trees::domain::ProofFact::Membership(membership) => {
            if let Some(place) = expression::instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                membership.value,
            ) {
                return FactPlace::Place(place);
            }
        }
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
        .iter()
        .copied()
        .collect();

    let mut segments = substitution.segments;
    segments.extend(original_segments);
    FactPlace::Place(place_builders::append_place_with_segments(
        facts,
        substitution.root,
        &segments,
    ))
}
pub(crate) use place_builders::{
    append_place_from_name_path, append_place_segment, canonical_place_to_fact_place,
    resolve_place_member_symbol,
};
pub(crate) use receiver::receiver_place_for_call;
