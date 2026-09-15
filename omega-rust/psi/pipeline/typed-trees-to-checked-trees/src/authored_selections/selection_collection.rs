//! Collecting statement, transition-target and proof membership
//! selections.

use crate::authored_selections::call_targets::{
    checked_statement_call_target, checked_target_conformance_targets, declaration_target,
};
use crate::authored_selections::finalization::push_consistent_resolution;
use crate::authored_selections::intrinsic_calls::checked_statement_call_intrinsic;
use crate::authored_selections::{CheckedResolution, CheckedResolutionTarget};
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionTarget,
};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;

pub(crate) fn checked_struct_literal_type_symbol(
    program: &TypedTrees,
    literal: &typed_trees::expression::TableStructLiteral,
    source_span: source::SourceSpan,
) -> SymbolHandle {
    if literal.type_symbol.is_valid() {
        return literal.type_symbol;
    }

    // This selection was deliberately retained as late-bound. At the checked
    // boundary the complete symbol table can resolve the authored head in its
    // exact source/package visibility context. The ledger stores the resulting
    // symbol; downstream consumers never repeat this lookup.
    program
        .symbols
        .find_top_level_by_name_and_kinds_from_source(
            literal.type_name.as_str(),
            &[SymbolKind::Data],
            source_span,
        )
        .unwrap_or_else(SymbolHandle::invalid)
}

pub(crate) fn collect_checked_proof_membership_selections(
    program: &TypedTrees,
    facts: &CheckFacts,
    resolutions: &mut Vec<CheckedResolution>,
) -> Result<(), Diagnostic> {
    for (fact_handle, fact) in program.proof_facts.iter() {
        let typed_trees::domain::ProofFact::Membership(membership) = fact else {
            continue;
        };
        let Some(occurrence) = membership.authored_domain_selection else {
            continue;
        };
        let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
            return Err(Diagnostic::error(format!(
                "proof membership retains unknown authored selection occurrence {}",
                occurrence.ordinal(),
            )));
        };
        if selection.kind() != AuthoredDeclarationSelectionKind::DomainMembership {
            return Err(Diagnostic::error(
                "proof membership retains mismatched authored domain-selection evidence",
            )
            .with_source_span(selection.source_span()));
        }
        if selection.target()
            != AuthoredDeclarationSelectionTarget::LateBound(
                AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership,
            )
        {
            continue;
        }

        let target = declaration_target(membership.domain_symbol).or_else(|| {
            let mut permission = None;
            for (_, checked_fact) in facts.semantic.facts.iter() {
                let facts::FactPayload::ContractCarryPermission {
                    fact,
                    permission: candidate,
                    ..
                } = checked_fact.payload
                else {
                    continue;
                };
                if fact != fact_handle {
                    continue;
                }
                if permission.is_some_and(|retained| retained != candidate) {
                    return None;
                }
                permission = Some(candidate);
            }
            permission.map(|permission| {
                CheckedResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::CarryPermission(permission),
                )
            })
        });
        if let Some(target) = target {
            push_consistent_resolution(
                resolutions,
                CheckedResolution {
                    occurrence,
                    binding: AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership,
                    target,
                },
            )?;
        }
    }
    Ok(())
}

pub(crate) fn collect_checked_statement_selections(
    program: &TypedTrees,
    facts: &CheckFacts,
    resolutions: &mut Vec<CheckedResolution>,
    inferred_conformances: &mut Vec<(
        source::SourceSpan,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
        SymbolHandle,
    )>,
) -> Result<(), Diagnostic> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let typed_trees::statement::StatementNode::Call(call) = statement else {
                    continue;
                };
                if call.operational_acknowledgement.origin
                    != language_semantics::CallOperationalAcknowledgementOrigin::Source
                    || call.source_span.span.start >= call.source_span.span.end
                {
                    continue;
                }
                let target = checked_statement_call_target(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    call.target_symbol,
                );
                if target.is_valid() {
                    for selected_symbol in checked_target_conformance_targets(program, target) {
                        let inferred = (
                            call.source_span,
                            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                            selected_symbol,
                        );
                        if !inferred_conformances.contains(&inferred) {
                            inferred_conformances.push(inferred);
                        }
                    }
                }

                let Some(occurrence) = call.authored_call_selection else {
                    return Err(Diagnostic::error(
                        "source-authored checked statement call has no attached call selection",
                    )
                    .with_source_span(call.source_span));
                };
                let Some(selection) = program.authored_declaration_selections().get(occurrence)
                else {
                    return Err(Diagnostic::error(format!(
                        "statement call retains unknown authored selection occurrence {}",
                        occurrence.ordinal(),
                    ))
                    .with_source_span(call.source_span));
                };
                if selection.kind() != AuthoredDeclarationSelectionKind::Call
                    || selection.exposure()
                        != language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
                    || selection.source_span() != call.source_span
                {
                    return Err(Diagnostic::error(
                        "statement call retains mismatched authored call-selection evidence",
                    )
                    .with_source_span(call.source_span));
                }
                let resolution_target = checked_statement_call_intrinsic(program, state, call)
                    .map(CheckedResolutionTarget::Intrinsic)
                    .or_else(|| {
                        crate::flow::resolved_operator_statement_symbol(program, call)
                            .and_then(declaration_target)
                    })
                    .or_else(|| declaration_target(target));
                if selection.target()
                    == AuthoredDeclarationSelectionTarget::LateBound(
                        AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    )
                    && let Some(target) = resolution_target
                {
                    push_consistent_resolution(
                        resolutions,
                        CheckedResolution {
                            occurrence,
                            binding: AuthoredDeclarationSelectionLateBinding::CheckedCall,
                            target,
                        },
                    )?;
                }
            }
        }
    }
    collect_checked_transition_target_selections(program, resolutions)?;
    Ok(())
}

pub(crate) fn collect_checked_transition_target_selections(
    program: &TypedTrees,
    resolutions: &mut Vec<CheckedResolution>,
) -> Result<(), Diagnostic> {
    for machine in program.machines() {
        if program
            .machine_specializations
            .iter()
            .any(|specialization| {
                specialization.instance == machine.symbol
                    && specialization.template != specialization.instance
            })
        {
            continue;
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let typed_trees::statement::StatementNode::Transition(transition) = statement
                else {
                    continue;
                };
                for target in [transition.target, transition.continuation] {
                    if !target.is_valid() {
                        continue;
                    }
                    let typed_trees::statement::TransitionTargetNode::Named {
                        path,
                        source_span,
                        authored_call_selection: Some(occurrence),
                        ..
                    } = program.statement_table.transition_target(target)
                    else {
                        continue;
                    };
                    let Some(selection) =
                        program.authored_declaration_selections().get(*occurrence)
                    else {
                        return Err(Diagnostic::error(format!(
                            "transition target retains unknown authored selection occurrence {}",
                            occurrence.ordinal(),
                        ))
                        .with_source_span(*source_span));
                    };
                    if selection.kind() != AuthoredDeclarationSelectionKind::Call
                        || selection.source_span() != *source_span
                    {
                        return Err(Diagnostic::error(
                            "transition target retains mismatched authored call-selection evidence",
                        )
                        .with_source_span(*source_span));
                    }
                    if selection.target()
                        == AuthoredDeclarationSelectionTarget::LateBound(
                            AuthoredDeclarationSelectionLateBinding::CheckedCall,
                        )
                    {
                        let target_symbol = if path.symbol.is_valid() {
                            path.symbol
                        } else {
                            let Some(target_name) = program
                                .statement_table
                                .name_path_members(path.members)
                                .last()
                            else {
                                continue;
                            };
                            let matching = program
                                .machine_type_parameters(machine)
                                .iter()
                                .filter(|parameter| {
                                    parameter.name == *target_name
                                        && matches!(
                                            parameter.kind,
                                            typed_trees::data::TypeParameterKind::Machine { .. }
                                        )
                                })
                                .map(|parameter| parameter.symbol)
                                .collect::<Vec<_>>();
                            let [target] = matching.as_slice() else {
                                continue;
                            };
                            *target
                        };
                        if !target_symbol.is_valid() {
                            continue;
                        }
                        push_consistent_resolution(
                            resolutions,
                            CheckedResolution {
                                occurrence: *occurrence,
                                binding: AuthoredDeclarationSelectionLateBinding::CheckedCall,
                                target: CheckedResolutionTarget::Declaration(target_symbol),
                            },
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}
