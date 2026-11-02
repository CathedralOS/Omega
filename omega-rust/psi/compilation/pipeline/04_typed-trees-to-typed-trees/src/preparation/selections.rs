//! Bind authored member and call identities before specialization consumes
//! their generic parameter lists. This writes only a still-owned typed program;
//! final checking resolutions publish into CheckFacts instead.

use crate::tables::{expressions_of, expressions_of_mut};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionTarget,
};
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use validation::CheckingInput;
use validation::selection_resolution::{
    AuthoredSelectionResolution, AuthoredSelectionResolutionTarget, authored_data_member,
    collect_checked_transition_target_selections, declaration_target, exact_named_operator_call,
    finalization_diagnostic, push_consistent_resolution,
};

pub fn bind_pre_specialization_authored_selections(
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    let contract_facts = arena::Arena::default();
    let mut resolutions = Vec::new();
    // A generic bound's operation is known before any concrete implementation
    // exists. Preserve that declaration selection on the authored occurrence;
    // an unused template's contracts must not acquire primitive token meaning.
    for (expression, target) in validation::collect_pre_specialization_operator_selections(program)?
    {
        for occurrence in expressions_of(program).authored_selection_occurrences(expression) {
            let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
                continue;
            };
            let binding = AuthoredDeclarationSelectionLateBinding::CheckedOperator;
            if selection.target() == AuthoredDeclarationSelectionTarget::LateBound(binding) {
                push_consistent_resolution(
                    program,
                    &mut resolutions,
                    AuthoredSelectionResolution {
                        occurrence,
                        binding,
                        target,
                    },
                )?;
            }
        }
    }
    // One exact-owner index answers every late-bound member and call's
    // query in a single pass instead of a whole-program rescan each.
    let owner_index =
        validation::expression_owners::OwnerEnvironmentIndex::for_preparation(program);
    validation::validate_executable_domain_test_carriers(program, &contract_facts, &owner_index)?;
    for (expression, node) in expressions_of(program).iter_expressions() {
        for occurrence in expressions_of(program).authored_selection_occurrences(expression) {
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
                ) => validation::expression_owners::checked_member_target_from_exact_owner(
                    program,
                    &contract_facts,
                    expression,
                    member,
                    Some(&owner_index),
                )
                .map(|target| match target {
                    validation::expression_owners::OwnerMemberTarget::Declaration(symbol) => {
                        AuthoredSelectionResolutionTarget::Declaration(authored_data_member(
                            program, symbol,
                        ))
                    }
                    validation::expression_owners::OwnerMemberTarget::CollectionMeasure(measure) => {
                        AuthoredSelectionResolutionTarget::Intrinsic(measure.intrinsic())
                    }
                }),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    ExpressionNode::Call(call),
                ) => validation::compiler_operations::classify_expression_call_intrinsic(
                    program, call,
                )
                .map(AuthoredSelectionResolutionTarget::Intrinsic)
                .or_else(|| {
                    exact_named_operator_call(program, call)
                        .and_then(|operator| declaration_target(operator.symbol))
                        .or_else(|| {
                            validation::expression_owners::checked_machine_call_target_from_exact_owner(
                                program,
                                &contract_facts,
                                expression,
                                call,
                                Some(&owner_index),
                            )
                            .and_then(declaration_target)
                        })
                }),
                _ => None,
            };
            let Some(target) = target else {
                continue;
            };
            push_consistent_resolution(
                program,
                &mut resolutions,
                AuthoredSelectionResolution {
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
            AuthoredSelectionResolutionTarget::Declaration(symbol) => {
                selections.finalize_late_bound(resolution.occurrence, resolution.binding, symbol)
            }
            AuthoredSelectionResolutionTarget::Intrinsic(intrinsic) => {
                selections.finalize_intrinsic(resolution.occurrence, resolution.binding, intrinsic)
            }
        }
        .map_err(|error| finalization_diagnostic(resolution, error))?;
    }
    program.retain_authored_declaration_selections(selections);
    Ok(())
}
