//! Contract fact places: the storage a callee's contract fact names,
//! rewritten into the caller's terms at one call, and the place builders that
//! fact construction and flow facts share.
//!
//! `instantiate_call_contract_place` is the entry;
//! `semantic::facts::contracts` calls it for each contract fact of a call.
//! For an expression or membership fact it first tries `expression`, which
//! builds the place from the contract expression with the call's argument,
//! receiver or result in place of each callee name. Otherwise it takes the
//! fact's declared place (`semantic::facts::contract_fact_place`) and, when
//! that place is rooted at a callee parameter, replaces the root with the
//! place of the matching argument or receiver (`substitution`), keeping the
//! remaining segments.
//!
//! `receiver` finds the place of a call's receiver for `expression` and
//! `substitution`. `place_builders` appends places and segments to a
//! `facts::FactPlan` and converts canonical flow places into fact places;
//! flow facts and contract facts call it directly. `expression` also exports
//! `instantiate_outcome_contract_expression_place`, which binds `result` to a
//! given result expression, and `call_contract_argument_projection`, which
//! the contract prover uses.

use crate::checked_trees::{ContractCallFact, ContractProofFact};
use crate::fact_plan::{FactPlace, FactPlan};
use crate::semantic::calls::CallSite;
use crate::semantic::calls::call_site_argument_expressions;
use crate::semantic::calls::call_target_parameters;
use crate::semantic::calls::find_call_site;
use crate::semantic::calls::find_state_in_machine;
use crate::semantic::facts::contract_fact_place;
mod expression;
mod place_builders;
mod receiver;
mod substitution;

pub(crate) use expression::{
    call_contract_argument_projection, instantiate_call_contract_expression_place,
    instantiate_outcome_contract_expression_place,
};

pub(crate) fn instantiate_call_contract_place(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    contract: &ContractProofFact,
) -> FactPlace {
    match program.proof_facts.get(contract.fact) {
        symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact::Expression(
            expression,
        ) => {
            if let Some(place) = expression::instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                *expression,
            ) {
                return FactPlace::Place(place);
            }
        }
        symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact::Membership(
            membership,
        ) => {
            if let Some(place) = expression::instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                membership.value,
            ) {
                return FactPlace::Place(place);
            }
        }
        symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact::Proposition(_) => {}
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
