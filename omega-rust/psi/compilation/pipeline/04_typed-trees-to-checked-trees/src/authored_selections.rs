//! Retain authored declaration custody through specialization and final checking.
//!
//! Operator occurrences in static type endpoints may lack executable use facts.
//! Their conservative candidate query uses independently known operand carriers,
//! not runtime intervals or a guessed peer type. Validated literal suffixes can
//! exclude unrelated operators; they cannot bypass a matching authored meaning
//! or replace the final selection and package-admission checks.
//!
//! `finalization.rs` publishes checked selections, and
//! `selection_collection.rs` collects proof/statement conclusions. Independent
//! validation owns source environments, compiler operation classification and
//! consistent occurrence resolution. Preparation owns early binding; this
//! owner never rewrites the typed input.

mod call_targets;
mod contract_resolution;
mod finalization;
mod member_targets;
mod operator_targets;
mod selection_collection;

pub(crate) use contract_resolution::{
    CheckedContractOperatorResolution, checked_named_operator_call, checked_operator_resolution,
};
pub(crate) use operator_targets::{
    typed_operator_authored_selection_candidates, typed_operator_has_no_authored_selection,
};

pub(crate) use crate::authored_selections::finalization::finalize_checked_authored_selections;
use checked_trees::CheckFacts;
use symbols::SymbolHandle;

pub(crate) fn derive_checked_nominal_call_target(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    let call = program.expression_table.call_node(expression)?;
    validation::expression_owners::checked_machine_call_target_from_exact_owner(
        program,
        &facts.proof.contract_facts,
        expression,
        call,
        None,
    )
}

/// Exact declaration for one late-bound `CheckedMember` occurrence, derived
/// from the receiver's owner type in every checked environment that contains
/// the expression, before checked binding has run. Case-payload projections
/// synthesized from destructure patterns stay unresolved through typing; this
/// is the same owner-type resolution checked binding applies to them. `None`
/// means no exact owner is derivable, never that the spelling is free.
pub(crate) fn exact_owner_member_declaration(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    let member = program.expression_table.member_node(expression)?;
    if member.member_symbol.is_valid() {
        return Some(member.member_symbol);
    }
    match validation::expression_owners::checked_member_target_from_exact_owner(
        program,
        &arena::Arena::default(),
        expression,
        member,
        None,
    )? {
        validation::expression_owners::OwnerMemberTarget::Declaration(symbol) => Some(symbol),
        validation::expression_owners::OwnerMemberTarget::CollectionMeasure(_) => None,
    }
}
