//! Retain authored declaration custody through specialization and final checking.
//!
//! Operator occurrences in static type endpoints may lack executable use facts.
//! Their conservative candidate query uses independently known operand carriers,
//! not runtime intervals or a guessed peer type. Validated literal suffixes can
//! exclude unrelated operators; they cannot bypass a matching authored meaning
//! or replace the final selection and package-admission checks.
//!
//! This file retains checked intrinsic-call facts. `finalization.rs` publishes
//! checked selections, and `selection_collection.rs` collects proof/statement
//! conclusions. Independent validation owns source environments, compiler
//! operation classification and consistent occurrence resolution. Preparation
//! owns early binding; this owner never rewrites the typed input.

mod call_targets;
mod contract_resolution;
mod finalization;
mod intrinsic_calls;
mod member_targets;
mod operator_targets;
mod review;
mod selection_collection;

pub(crate) use contract_resolution::{
    CheckedContractOperatorResolution, checked_named_operator_call, checked_operator_resolution,
};
pub(crate) use operator_targets::{
    typed_operator_authored_selection_candidates, typed_operator_has_no_authored_selection,
};
pub(crate) use review::derive_checked_collection_view_intrinsic;

pub(crate) use crate::authored_selections::finalization::finalize_checked_authored_selections;
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;

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

pub(crate) fn bind_checked_intrinsic_call_facts(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Result<(), Diagnostic> {
    let mut intrinsic_calls = Vec::new();
    // One exact-owner index answers every call expression's query in a
    // single pass instead of a whole-program rescan per call site.
    let owner_index = validation::expression_owners::OwnerEnvironmentIndex::new(
        program,
        &facts.proof.contract_facts,
    );
    for (expression, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        let Some(intrinsic) =
            validation::expression_owners::checked_collection_view_intrinsic_from_exact_owner(
                program,
                &facts.proof.contract_facts,
                expression,
                call,
                Some(&owner_index),
            )
        else {
            continue;
        };
        if intrinsic_calls
            .iter()
            .any(|fact: &checked_trees::CheckedIntrinsicCallFact| {
                fact.expression == expression && fact.intrinsic != intrinsic
            })
        {
            return Err(Diagnostic::error(
                "one checked call expression selected conflicting compiler intrinsics",
            )
            .with_source_span(program.expression_table.source_span(expression)));
        }
        if !intrinsic_calls
            .iter()
            .any(|fact| fact.expression == expression && fact.intrinsic == intrinsic)
        {
            intrinsic_calls.push(checked_trees::CheckedIntrinsicCallFact {
                expression,
                intrinsic,
            });
        }
    }
    facts.intrinsic_calls = intrinsic_calls;
    Ok(())
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
