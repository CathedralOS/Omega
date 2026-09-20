//! Retain authored declaration custody through specialization and final checking.
//!
//! Operator occurrences in static type endpoints may lack executable use facts.
//! Their conservative candidate query uses independently known operand carriers,
//! not runtime intervals or a guessed peer type. Validated literal suffixes can
//! exclude unrelated operators; they cannot bypass a matching authored meaning
//! or replace the final selection and package-admission checks.
//!
//! This file binds and finalizes authored selections. `finalization.rs`
//! finalizes selections under a policy, `selection_collection.rs` collects
//! the statement, transition and proof selections, `intrinsic_calls.rs`
//! recognizes intrinsic calls and build receivers, `call_targets.rs` and
//! `operator_targets.rs` resolve call and operator targets and
//! `member_targets.rs` resolves contextual member targets.

mod call_targets;
mod contexts;
mod contract_resolution;
mod finalization;
mod intrinsic_calls;
mod member_targets;
mod operator_targets;
mod review;
mod selection_collection;
#[cfg(test)]
mod symbol_types_tests;

pub(crate) use operator_targets::{
    typed_operator_authored_selection_candidates, typed_operator_has_no_authored_selection,
};
pub(crate) use review::derive_checked_collection_view_intrinsic;

use crate::authored_selections::call_targets::{declaration_target, exact_named_operator_call};
use crate::authored_selections::finalization::{
    finalization_diagnostic, finalize_checked_authored_selections_with_policy,
    push_consistent_resolution,
};
use crate::authored_selections::selection_collection::collect_checked_transition_target_selections;
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionTarget,
};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, expression::ExpressionNode};

pub(crate) fn derive_checked_nominal_call_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    contexts::checked_machine_call_target_from_exact_owner(program, facts, expression, call)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CheckedResolution {
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
    binding: AuthoredDeclarationSelectionLateBinding,
    target: CheckedResolutionTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckedResolutionTarget {
    Declaration(SymbolHandle),
    Intrinsic(AuthoredDeclarationSelectionIntrinsic),
}

pub(crate) fn bind_checked_intrinsic_call_facts(
    program: &TypedTrees,
    facts: &mut CheckFacts,
) -> Result<(), Diagnostic> {
    let mut intrinsic_calls = Vec::new();
    for (expression, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        let Some(intrinsic) = contexts::checked_collection_view_intrinsic_from_exact_owner(
            program, facts, expression, call,
        ) else {
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
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    if member.member_symbol.is_valid() {
        return Some(member.member_symbol);
    }
    match contexts::checked_member_target_from_exact_owner(
        program,
        &CheckFacts::default(),
        expression,
        member,
    )? {
        contexts::OwnerMemberTarget::Declaration(symbol) => Some(symbol),
        contexts::OwnerMemberTarget::CollectionLength
        | contexts::OwnerMemberTarget::CollectionCapacity => None,
    }
}

pub(crate) fn bind_pre_specialization_authored_selections(
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    let facts = CheckFacts::default();
    let mut resolutions = Vec::new();
    for (expression, node) in program.expression_table.iter_expressions() {
        for occurrence in program
            .expression_table
            .authored_selection_occurrences(expression)
        {
            let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
                return Err(Diagnostic::error(format!(
                    "expression retains unknown authored declaration selection occurrence {}",
                    occurrence.ordinal(),
                )));
            };
            let AuthoredDeclarationSelectionTarget::LateBound(binding) = selection.target() else {
                continue;
            };
            let target = match (binding, node) {
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedMember,
                    ExpressionNode::Member(member),
                ) => contexts::checked_member_target_from_exact_owner(
                    program, &facts, expression, member,
                )
                .map(|target| match target {
                    contexts::OwnerMemberTarget::Declaration(symbol) => {
                        CheckedResolutionTarget::Declaration(symbol)
                    }
                    contexts::OwnerMemberTarget::CollectionLength => {
                        CheckedResolutionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::CollectionLength,
                        )
                    }
                    contexts::OwnerMemberTarget::CollectionCapacity => {
                        CheckedResolutionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::CollectionCapacity,
                        )
                    }
                }),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    ExpressionNode::Call(call),
                ) => exact_named_operator_call(program, call)
                    .and_then(|operator| declaration_target(operator.symbol))
                    .or_else(|| {
                        contexts::checked_machine_call_target_from_exact_owner(
                            program, &facts, expression, call,
                        )
                        .and_then(declaration_target)
                    }),
                _ => None,
            };
            let Some(target) = target else {
                continue;
            };
            push_consistent_resolution(
                &mut resolutions,
                CheckedResolution {
                    occurrence,
                    binding,
                    target,
                },
            )?;
        }
    }
    collect_checked_transition_target_selections(program, &mut resolutions)?;

    let mut selections = program.authored_declaration_selections().clone();
    for resolution in resolutions {
        match resolution.target {
            CheckedResolutionTarget::Declaration(symbol) => {
                selections.finalize_late_bound(resolution.occurrence, resolution.binding, symbol)
            }
            CheckedResolutionTarget::Intrinsic(intrinsic) => {
                selections.finalize_intrinsic(resolution.occurrence, resolution.binding, intrinsic)
            }
        }
        .map_err(|error| finalization_diagnostic(resolution, error))?;
    }
    program.retain_authored_declaration_selections(selections);
    Ok(())
}

pub(crate) fn finalize_checked_authored_selections(
    program: &mut TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Diagnostic> {
    finalize_checked_authored_selections_with_policy(program, facts, false)
}

pub(crate) fn finalize_preliminary_checked_authored_selections(
    program: &mut TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Diagnostic> {
    finalize_checked_authored_selections_with_policy(program, facts, true)
}
