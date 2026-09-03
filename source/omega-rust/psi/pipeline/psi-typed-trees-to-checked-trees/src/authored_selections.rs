use psi_checked_trees::{CheckFacts, CheckedOperatorResolutionStatus};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionFinalizationError, AuthoredDeclarationSelectionIntrinsic,
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionTarget,
};
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::{TypedTrees, expression::ExpressionNode};

mod contexts;
mod contract_resolution;
mod review;

pub(crate) use review::derive_checked_collection_view_intrinsic;

pub(crate) fn derive_checked_nominal_call_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
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
            .any(|fact: &psi_checked_trees::CheckedIntrinsicCallFact| {
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
            intrinsic_calls.push(psi_checked_trees::CheckedIntrinsicCallFact {
                expression,
                intrinsic,
            });
        }
    }
    facts.intrinsic_calls = intrinsic_calls;
    Ok(())
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

fn finalize_checked_authored_selections_with_policy(
    program: &mut TypedTrees,
    facts: &CheckFacts,
    allow_unresolved_toolchain: bool,
) -> Result<(), Diagnostic> {
    let mut resolutions = Vec::new();
    let mut inferred_conformances = Vec::new();
    let expressions = &program.tables.expression_table;

    for (expression, node) in expressions.iter_expressions() {
        let occurrences = expressions
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        for (occurrence_offset, occurrence) in occurrences.iter().copied().enumerate() {
            let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
                return Err(Diagnostic::error(format!(
                    "expression retains unknown authored declaration selection occurrence {}",
                    occurrence.ordinal()
                )));
            };

            if selection.kind() == AuthoredDeclarationSelectionKind::Call
                && let ExpressionNode::Call(call) = node
            {
                for selected_symbol in checked_call_conformance_targets(
                    program,
                    facts,
                    expression,
                    call.target_symbol,
                    selection.source_span(),
                ) {
                    let inferred = (
                        selection.source_span(),
                        selection.exposure(),
                        selected_symbol,
                    );
                    if !inferred_conformances.contains(&inferred) {
                        inferred_conformances.push(inferred);
                    }
                }
            }
            let AuthoredDeclarationSelectionTarget::LateBound(binding) = selection.target() else {
                continue;
            };

            if binding == AuthoredDeclarationSelectionLateBinding::CheckedOperator {
                for selected_symbol in checked_operator_conformance_targets(facts, expression) {
                    let inferred = (
                        selection.source_span(),
                        selection.exposure(),
                        selected_symbol,
                    );
                    if !inferred_conformances.contains(&inferred) {
                        inferred_conformances.push(inferred);
                    }
                }
            }
            let target = match (binding, node) {
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    ExpressionNode::Call(call),
                ) => checked_intrinsic_call_target(facts, expression)
                    .or_else(|| {
                        checked_call_intrinsic(
                            program,
                            call.target.as_str(),
                            call.target_symbol,
                            call.receiver,
                        )
                    })
                    .map(CheckedResolutionTarget::Intrinsic)
                    .or_else(|| {
                        declaration_target(checked_call_target(
                            program,
                            facts,
                            expression,
                            call.target_symbol,
                            selection.source_span(),
                        ))
                    }),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedMember,
                    ExpressionNode::Member(member),
                ) => checked_member_target(program, facts, expression, member),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStaticPathSegment,
                    ExpressionNode::Name(path),
                ) => declaration_target(checked_name_path_segment_target(
                    program,
                    expression,
                    path,
                    late_binding_ordinal(program, &occurrences[..occurrence_offset], binding),
                )),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStructLiteralType,
                    ExpressionNode::StructLiteral(literal),
                ) => declaration_target(checked_struct_literal_type_symbol(
                    program,
                    literal,
                    selection.source_span(),
                )),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStructLiteralCase,
                    ExpressionNode::StructLiteral(literal),
                ) => declaration_target(literal.case_symbol.unwrap_or_else(SymbolHandle::invalid)),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStructLiteralField,
                    ExpressionNode::StructLiteral(literal),
                ) => declaration_target(
                    expressions
                        .struct_fields(literal.fields)
                        .get(late_binding_ordinal(
                            program,
                            &occurrences[..occurrence_offset],
                            binding,
                        ))
                        .map(|field| {
                            if field.field_symbol.is_valid() {
                                field.field_symbol
                            } else {
                                crate::flow::resolve_member_symbol_from_type_symbol(
                                    program,
                                    checked_struct_literal_type_symbol(
                                        program,
                                        literal,
                                        selection.source_span(),
                                    ),
                                    field.name.as_str(),
                                )
                                .unwrap_or_else(SymbolHandle::invalid)
                            }
                        })
                        .unwrap_or_else(SymbolHandle::invalid),
                ),
                (AuthoredDeclarationSelectionLateBinding::CheckedOperator, _)
                    if matches!(
                        node,
                        ExpressionNode::Binary(_)
                            | ExpressionNode::Indexed(_)
                            | ExpressionNode::Unary(_)
                    ) =>
                {
                    checked_operator_target_for_occurrence(
                        program,
                        facts,
                        expression,
                        node,
                        occurrence,
                    )
                    .or_else(|| {
                        typed_operator_has_no_authored_selection(program, expression).then_some(
                            CheckedResolutionTarget::Intrinsic(
                                AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                            ),
                        )
                    })
                }
                // Primitive constant folding can replace a successfully checked
                // builtin operator with its literal result while retaining the
                // source operator's custody occurrence on that result.
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedOperator,
                    ExpressionNode::Boolean(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::Integer(_),
                ) => Some(CheckedResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                )),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedOperator,
                    ExpressionNode::Call(call),
                ) if call.operational_acknowledgement.origin
                    == psi_language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized =>
                {
                    checked_structural_equality_call(program, facts, expression, call).then_some(
                        CheckedResolutionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                        ),
                    )
                }
                _ => None,
            };
            if let Some(target) = target {
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
    }

    collect_checked_statement_selections(
        program,
        facts,
        &mut resolutions,
        &mut inferred_conformances,
    )?;
    collect_checked_proof_membership_selections(program, facts, &mut resolutions)?;

    let mut selections = program.authored_declaration_selections().clone();
    for resolution in resolutions {
        let result = match resolution.target {
            CheckedResolutionTarget::Declaration(selected) => {
                selections.finalize_late_bound(resolution.occurrence, resolution.binding, selected)
            }
            CheckedResolutionTarget::Intrinsic(intrinsic) => {
                selections.finalize_intrinsic(resolution.occurrence, resolution.binding, intrinsic)
            }
        };
        result.map_err(|error| finalization_diagnostic(resolution, error))?;
    }
    for (source_span, exposure, selected_symbol) in inferred_conformances {
        let already_retained = selections.iter().any(|selection| {
            selection.source_span() == source_span
                && selection.exposure() == exposure
                && selection.kind()
                    == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance
                && matches!(
                    selection.target(),
                    AuthoredDeclarationSelectionTarget::Resolved(target)
                        if target.selected_symbol() == selected_symbol
                )
        });
        if !already_retained {
            selections
                .record_resolved(
                    source_span,
                    exposure,
                    psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance,
                    selected_symbol,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "failed to retain checked authored conformance selection: {error:?}"
                    ))
                    .with_source_span(source_span)
            })?;
        }
    }
    if let Some(selection) = selections.iter().find(|selection| {
        if !matches!(
            selection.target(),
            AuthoredDeclarationSelectionTarget::LateBound(_)
        ) {
            return false;
        }
        !allow_unresolved_toolchain
            || program
                .symbols
                .source_file(selection.source_span())
                .is_none_or(|source| source.origin != psi_source::SourceOrigin::Toolchain)
    }) {
        let AuthoredDeclarationSelectionTarget::LateBound(binding) = selection.target() else {
            unreachable!("guarded late-bound authored selection")
        };
        return Err(Diagnostic::error(format!(
            "authored {:?} declaration selection occurrence {} remained unresolved after successful checking ({binding:?})",
            selection.kind(),
            selection.occurrence_id().ordinal(),
        ))
        .with_source_span(selection.source_span()));
    }
    program.retain_authored_declaration_selections(selections);
    Ok(())
}

fn checked_struct_literal_type_symbol(
    program: &TypedTrees,
    literal: &psi_typed_trees::expression::TableStructLiteral,
    source_span: psi_source::SourceSpan,
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

fn collect_checked_proof_membership_selections(
    program: &TypedTrees,
    facts: &CheckFacts,
    resolutions: &mut Vec<CheckedResolution>,
) -> Result<(), Diagnostic> {
    for (fact_handle, fact) in program.proof_facts.iter() {
        let psi_typed_trees::domain::ProofFact::Membership(membership) = fact else {
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
                let psi_facts::FactPayload::ContractCarryPermission {
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

fn collect_checked_statement_selections(
    program: &TypedTrees,
    facts: &CheckFacts,
    resolutions: &mut Vec<CheckedResolution>,
    inferred_conformances: &mut Vec<(
        psi_source::SourceSpan,
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
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
                let psi_typed_trees::statement::StatementNode::Call(call) = statement else {
                    continue;
                };
                if call.operational_acknowledgement.origin
                    != psi_language_semantics::CallOperationalAcknowledgementOrigin::Source
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
                            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
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
                        != psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
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

fn collect_checked_transition_target_selections(
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
                let psi_typed_trees::statement::StatementNode::Transition(transition) = statement
                else {
                    continue;
                };
                for target in [transition.target, transition.continuation] {
                    if !target.is_valid() {
                        continue;
                    }
                    let psi_typed_trees::statement::TransitionTargetNode::Named {
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
                                            psi_typed_trees::data::TypeParameterKind::Machine { .. }
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

fn checked_statement_call_intrinsic(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    call: &psi_typed_trees::statement::TableCall,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    if call.target_symbol.is_valid() {
        return None;
    }
    if program.wire_encode_call_schema(call).is_some() {
        return Some(Intrinsic::WireEncode);
    }
    if program.wire_decode_call_schema(call).is_some() {
        return Some(Intrinsic::WireDecode);
    }
    if exact_statement_build_output_receiver(program, state, call) {
        return Some(Intrinsic::BuildIncludedSourceHandoff);
    }
    if exact_statement_build_log_receiver(program, state, call) {
        return Some(Intrinsic::BuildLogWriteLine);
    }
    if exact_statement_build_optimization_receiver(program, state, call) {
        return Some(Intrinsic::BuildOptimizationSelection);
    }
    if exact_statement_build_optimization_report_receiver(program, state, call) {
        return Some(Intrinsic::BuildOptimizationReportRequest);
    }
    checked_call_intrinsic(
        program,
        call.target.as_str(),
        call.target_symbol,
        psi_typed_trees::expression::ExpressionHandle::invalid(),
    )
}

fn checked_intrinsic_call_target(
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<AuthoredDeclarationSelectionIntrinsic> {
    let mut matching = facts
        .intrinsic_calls
        .iter()
        .filter(|fact| fact.expression == expression)
        .map(|fact| fact.intrinsic);
    let selected = matching.next()?;
    matching
        .all(|candidate| candidate == selected)
        .then_some(selected)
}

fn checked_call_intrinsic(
    program: &TypedTrees,
    target: &str,
    target_symbol: SymbolHandle,
    receiver: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic> {
    use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;

    // A resolved declaration always wins over compiler vocabulary with the
    // same spelling. Receiver calls likewise cannot select these free-call
    // intrinsics. This keeps intrinsic recognition from becoming a
    // source-text fallback for an ordinary package declaration.
    if target_symbol.is_valid() {
        None
    } else if receiver.is_valid() {
        if exact_build_output_receiver(program, receiver, target) {
            Some(Intrinsic::BuildIncludedSourceHandoff)
        } else if exact_build_log_receiver(program, receiver, target) {
            Some(Intrinsic::BuildLogWriteLine)
        } else if exact_build_optimization_receiver(program, receiver, target) {
            Some(Intrinsic::BuildOptimizationSelection)
        } else if exact_build_optimization_report_receiver(program, receiver, target) {
            Some(Intrinsic::BuildOptimizationReportRequest)
        } else {
            None
        }
    } else if let Some(predicate) =
        psi_language_semantics::byte_predicates::ByteSequencePredicate::from_name(target)
    {
        Some(Intrinsic::ByteSequencePredicate(predicate))
    } else if target == "select_provider" {
        Some(Intrinsic::BuildProviderSelection)
    } else if target == "select_representation" {
        Some(Intrinsic::BuildRepresentationSelection)
    } else if target.starts_with("accept_boundary#") {
        Some(Intrinsic::BuildBoundaryAcceptance)
    } else if target.starts_with("wire_compatibility#") {
        Some(Intrinsic::BuildWireCompatibilityRequest)
    } else if target.starts_with("bind_root#") {
        Some(Intrinsic::BuildRootBinding)
    } else if target.starts_with("asm#") {
        Some(Intrinsic::InlineAssemblyOperation)
    } else {
        None
    }
}

fn exact_build_output_receiver(
    program: &TypedTrees,
    receiver: psi_typed_trees::expression::ExpressionHandle,
    target: &str,
) -> bool {
    if target != "include_source" {
        return false;
    }
    crate::flow::expression_type_symbol(program, receiver)
        .is_some_and(|type_symbol| exact_build_prelude_data(program, type_symbol, "BuildOutput"))
}

fn exact_build_log_receiver(
    program: &TypedTrees,
    receiver: psi_typed_trees::expression::ExpressionHandle,
    target: &str,
) -> bool {
    if target != "write_line" {
        return false;
    }
    crate::flow::expression_type_symbol(program, receiver)
        .is_some_and(|type_symbol| exact_build_prelude_data(program, type_symbol, "BuildLog"))
}

fn exact_build_optimization_receiver(
    program: &TypedTrees,
    receiver: psi_typed_trees::expression::ExpressionHandle,
    target: &str,
) -> bool {
    if target != "enable" {
        return false;
    }
    crate::flow::expression_type_symbol(program, receiver)
        .is_some_and(|type_symbol| exact_build_prelude_data(program, type_symbol, "Optimizations"))
}

fn exact_build_optimization_report_receiver(
    program: &TypedTrees,
    receiver: psi_typed_trees::expression::ExpressionHandle,
    target: &str,
) -> bool {
    if target != "emit_report" {
        return false;
    }
    crate::flow::expression_type_symbol(program, receiver)
        .is_some_and(|type_symbol| exact_build_prelude_data(program, type_symbol, "Optimizations"))
}

fn exact_statement_build_output_receiver(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    call: &psi_typed_trees::statement::TableCall,
) -> bool {
    call.target.as_str() == "include_source"
        && exact_statement_build_member_receiver(program, state, call, "BuildOutput")
}

fn exact_statement_build_log_receiver(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    call: &psi_typed_trees::statement::TableCall,
) -> bool {
    call.target.as_str() == "write_line"
        && exact_statement_build_member_receiver(program, state, call, "BuildLog")
}

fn exact_statement_build_optimization_receiver(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    call: &psi_typed_trees::statement::TableCall,
) -> bool {
    call.target.as_str() == "enable"
        && exact_statement_build_member_receiver(program, state, call, "Optimizations")
}

fn exact_statement_build_optimization_report_receiver(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    call: &psi_typed_trees::statement::TableCall,
) -> bool {
    call.target.as_str() == "emit_report"
        && exact_statement_build_member_receiver(program, state, call, "Optimizations")
}

fn exact_statement_build_member_receiver(
    program: &TypedTrees,
    state: &psi_typed_trees::state::State,
    call: &psi_typed_trees::statement::TableCall,
    expected_receiver: &str,
) -> bool {
    let [root, members @ ..] = program.statement_table.name_path_members(call.receiver) else {
        return false;
    };
    let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == root.as_str())
    else {
        return false;
    };
    let mut type_symbol = program
        .type_reference_table
        .type_symbol(parameter.type_reference);
    if !exact_build_prelude_data(program, type_symbol, "Build") {
        return false;
    }
    for member in members {
        let Some(selected) = crate::flow::resolve_member_symbol_from_type_symbol(
            program,
            type_symbol,
            member.as_str(),
        ) else {
            return false;
        };
        let Some(selected_type) = crate::flow::symbol_type_symbol(program, selected) else {
            return false;
        };
        type_symbol = selected_type;
    }
    exact_build_prelude_data(program, type_symbol, expected_receiver)
}

fn exact_build_prelude_data(program: &TypedTrees, type_symbol: SymbolHandle, name: &str) -> bool {
    program
        .symbols
        .symbol_source_span(type_symbol)
        .and_then(|span| program.symbols.source_file(span))
        .is_some_and(|source| {
            source.origin == psi_source::SourceOrigin::Toolchain
                && source.path == std::path::Path::new("<build-prelude>")
        })
        && program
            .data_definitions()
            .iter()
            .any(|data| data.symbol == type_symbol && data.name.as_str() == name)
}

fn checked_operator_conformance_targets(
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Vec<SymbolHandle> {
    let mut targets = Vec::new();
    for (_, operator_use) in facts.operators.uses.iter() {
        if operator_use.expression != expression
            || operator_use.status != CheckedOperatorResolutionStatus::Resolved
        {
            continue;
        }
        let Some(candidate) = facts.operators.selected_candidate(operator_use) else {
            continue;
        };
        if candidate.is_trait_backed() && !targets.contains(&candidate.conformance_symbol) {
            targets.push(candidate.conformance_symbol);
        }
    }
    targets
}

fn checked_call_conformance_targets(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    authored_target: SymbolHandle,
    authored_source_span: psi_source::SourceSpan,
) -> Vec<SymbolHandle> {
    let target = checked_call_target(
        program,
        facts,
        expression,
        authored_target,
        authored_source_span,
    );
    checked_target_conformance_targets(program, target)
}

fn checked_target_conformance_targets(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Vec<SymbolHandle> {
    let Some(machine_symbol) = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target)
            .then_some(machine.symbol)
    }) else {
        return Vec::new();
    };

    let mut targets = Vec::new();
    for specialization in &program.machine_specializations {
        if specialization.instance != machine_symbol {
            continue;
        }
        for selected in &specialization.inferred_conformance_arguments {
            if !targets.contains(selected) {
                targets.push(*selected);
            }
        }
    }
    targets
}

fn checked_statement_call_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    authored_target: SymbolHandle,
) -> SymbolHandle {
    if authored_target.is_valid() {
        return authored_target;
    }
    let Some(state) = facts.flow.control.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
    }) else {
        return SymbolHandle::invalid();
    };
    for call in facts.flow.control.calls.span_or_empty(state.calls) {
        if call.statement_index != statement_index || !call.target_symbol.is_valid() {
            continue;
        }
        if matches!(
            crate::semantic_calls::find_call_site(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                call.call_ordinal,
            ),
            Some(crate::semantic_calls::CallSite::Statement(_))
        ) {
            return call.target_symbol;
        }
    }
    SymbolHandle::invalid()
}

fn checked_call_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    authored_target: SymbolHandle,
    authored_source_span: psi_source::SourceSpan,
) -> SymbolHandle {
    if authored_target.is_valid() {
        return authored_target;
    }
    let mut checked_named_target = None;
    for use_fact in facts
        .operators
        .named_uses()
        .filter(|use_fact| use_fact.expression == expression)
    {
        if !use_fact.selected_operator_symbol.is_valid()
            || checked_named_target
                .is_some_and(|target| target != use_fact.selected_operator_symbol)
        {
            return SymbolHandle::invalid();
        }
        checked_named_target = Some(use_fact.selected_operator_symbol);
    }
    if let Some(target) = checked_named_target {
        return target;
    }
    if let ExpressionNode::Call(call) = program.expression_table.expression(expression)
        && let Some(operator) = exact_named_operator_call(program, call)
    {
        return operator.symbol;
    }
    if let ExpressionNode::Call(call) = program.expression_table.expression(expression)
        && let Some(operator) = contract_resolution::checked_named_operator_call(
            program,
            facts,
            expression,
            call,
            authored_source_span,
        )
    {
        return operator.symbol;
    }
    if let ExpressionNode::Call(call) = program.expression_table.expression(expression)
        && let Some(target) =
            contexts::checked_machine_call_target_from_exact_owner(program, facts, expression, call)
    {
        return target;
    }
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            if let Some(crate::semantic_calls::CallSite::Expression {
                expression: candidate,
                ..
            }) = crate::semantic_calls::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            ) && candidate == expression
                && call.target_symbol.is_valid()
            {
                return call.target_symbol;
            }
        }
    }
    let mut checked_source_target = None;
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            if !call.authored_source_custody_valid
                || call.authored_source_span != Some(authored_source_span)
                || !call.target_symbol.is_valid()
            {
                continue;
            }
            if checked_source_target.is_some_and(|target| target != call.target_symbol) {
                return SymbolHandle::invalid();
            }
            checked_source_target = Some(call.target_symbol);
        }
    }
    if let Some(target) = checked_source_target {
        return target;
    }
    let mut checked_fact_target = None;
    for projection in &facts.fact_call_projections {
        if projection.call_expression != expression || !projection.target_state.is_valid() {
            continue;
        }
        if checked_fact_target.is_some_and(|target| target != projection.target_state) {
            return SymbolHandle::invalid();
        }
        checked_fact_target = Some(projection.target_state);
    }
    if let Some(target) = checked_fact_target {
        return target;
    }
    SymbolHandle::invalid()
}

fn exact_named_operator_call<'program>(
    program: &'program TypedTrees,
    call: &psi_typed_trees::expression::TableCallExpression,
) -> Option<&'program psi_typed_trees::operator::OperatorDefinition> {
    psi_typed_trees::operator::resolve_named_expression_call(program, call).or_else(|| {
        let ExpressionNode::Name(path) = program.expression_table.expression(call.receiver) else {
            return None;
        };
        let static_segments = program
            .expression_table
            .name_path_members(path.members)
            .iter()
            .map(|segment| segment.as_str())
            .collect::<Vec<_>>();
        (!static_segments.is_empty()).then_some(())?;
        psi_typed_trees::operator::resolve_named_call(
            program,
            call.target_symbol,
            Some(&static_segments),
            call.target.as_str(),
            program
                .expression_table
                .expression_handles(call.arguments)
                .len(),
            false,
        )
    })
}

fn checked_name_path_segment_target(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    path: &psi_typed_trees::expression::TableNamePath,
    target_index: usize,
) -> SymbolHandle {
    let direct = crate::lookup::resolve_name_path_member_symbol(program, path, target_index);
    if direct.is_valid() {
        return direct;
    }

    let mut contextual_root = None;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut expressions = Vec::new();
                crate::monomorphization::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                if !expressions.contains(&expression) {
                    continue;
                }
                let Some(crate::flow::CanonicalPlace {
                    root: psi_facts::PlaceRoot::Symbol(root),
                    ..
                }) = crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    expression,
                )
                else {
                    continue;
                };
                let root = authored_contextual_root(program, machine, state, statement_index, root);
                if contextual_root.is_some_and(|candidate| candidate != root) {
                    return SymbolHandle::invalid();
                }
                contextual_root = Some(root);
            }
        }
    }

    let Some(mut selected) = contextual_root else {
        return SymbolHandle::invalid();
    };
    if target_index == 0 {
        return selected;
    }
    for member in program
        .expression_table
        .name_path_members(path.members)
        .iter()
        .skip(1)
        .take(target_index)
    {
        selected = crate::flow::symbol_type_symbol(program, selected)
            .and_then(|type_symbol| {
                crate::flow::resolve_member_symbol_from_type_symbol(
                    program,
                    type_symbol,
                    member.as_str(),
                )
            })
            .unwrap_or_else(SymbolHandle::invalid);
        if !selected.is_valid() {
            break;
        }
    }
    selected
}

fn authored_contextual_root(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statement_index: usize,
    selected: SymbolHandle,
) -> SymbolHandle {
    let Some(template_symbol) = program
        .machine_specializations
        .iter()
        .find(|specialization| {
            specialization.instance == machine.symbol
                && specialization.template != specialization.instance
        })
        .map(|specialization| specialization.template)
    else {
        return selected;
    };
    let Some(template) = crate::lookup::machine_by_symbol(program, template_symbol) else {
        return selected;
    };
    let Some(state_ordinal) = program
        .machine_states(machine)
        .iter()
        .position(|candidate| candidate.symbol == state.symbol)
    else {
        return selected;
    };
    let Some(template_state) = program.machine_states(template).get(state_ordinal) else {
        return selected;
    };

    if let Some(parameter_ordinal) = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == selected)
    {
        return program
            .state_parameters(template_state)
            .get(parameter_ordinal)
            .map_or(selected, |parameter| parameter.symbol);
    }

    let statements = program.statement_table.statements(state.statement_nodes);
    let template_statements = program
        .statement_table
        .statements(template_state.statement_nodes);
    for (ordinal, statement) in statements.iter().take(statement_index).enumerate() {
        let psi_typed_trees::statement::StatementNode::LocalData(local) = statement else {
            continue;
        };
        if local.symbol != selected {
            continue;
        }
        return template_statements
            .get(ordinal)
            .and_then(|statement| match statement {
                psi_typed_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
                _ => None,
            })
            .unwrap_or(selected);
    }
    selected
}

fn declaration_target(symbol: SymbolHandle) -> Option<CheckedResolutionTarget> {
    symbol
        .is_valid()
        .then_some(CheckedResolutionTarget::Declaration(symbol))
}

fn intrinsic_operator_operand_is_primitive(
    program: &TypedTrees,
    node: &ExpressionNode,
    origin: psi_checked_trees::CheckedValueOrigin,
) -> bool {
    let operand = match node {
        ExpressionNode::Binary(binary) => binary.left,
        ExpressionNode::Unary(unary) => unary.operand,
        _ => return false,
    };
    crate::operators::expression_type_reference_for_origin(program, operand, origin)
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some()
        || expression_is_intrinsic_primitive_without_origin(program, operand)
}

fn checked_operator_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
) -> Option<CheckedResolutionTarget> {
    // These operators have no authored declaration/spelling surface. Once
    // ordinary checking accepts their operand types, their exact meaning is
    // necessarily compiler intrinsic; nested-expression origins need not
    // recover a synthetic type reference merely to finalize custody.
    if matches!(
        node,
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                psi_typed_trees::expression::BinaryOperator::And
                    | psi_typed_trees::expression::BinaryOperator::BitwiseAnd
                    | psi_typed_trees::expression::BinaryOperator::BitwiseOr
                    | psi_typed_trees::expression::BinaryOperator::BitwiseXor
                    | psi_typed_trees::expression::BinaryOperator::Or
                    | psi_typed_trees::expression::BinaryOperator::ShiftLeft
                    | psi_typed_trees::expression::BinaryOperator::ShiftRight
            )
    ) {
        return Some(CheckedResolutionTarget::Intrinsic(
            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
        ));
    }

    let uses = facts
        .operators
        .uses
        .iter()
        .filter_map(|(_, operator_use)| {
            (operator_use.expression == expression).then_some(operator_use)
        })
        .collect::<Vec<_>>();

    uses.iter()
        .find_map(|operator_use| {
            (operator_use.status == CheckedOperatorResolutionStatus::Resolved)
                .then(|| declaration_target(operator_use.selected_operator_symbol))
                .flatten()
        })
        .or_else(|| {
            uses.iter()
                .any(|operator_use| {
                    operator_use.status == CheckedOperatorResolutionStatus::BuiltinFallback
                        || (operator_use.status == CheckedOperatorResolutionStatus::Missing
                            && intrinsic_operator_operand_is_primitive(
                                program,
                                node,
                                operator_use.origin,
                            ))
                })
                .then_some(CheckedResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                ))
        })
        .or_else(|| {
            contract_resolution::checked_operator_resolution(program, facts, expression, node)
                .and_then(|resolution| match resolution {
                    contract_resolution::CheckedContractOperatorResolution::Declaration(symbol) => {
                        declaration_target(symbol)
                    }
                    contract_resolution::CheckedContractOperatorResolution::Builtin => {
                        Some(CheckedResolutionTarget::Intrinsic(
                            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                        ))
                    }
                })
        })
        .or_else(|| {
            resolve_authored_operator_without_use_fact(program, node)
                .and_then(|operator| declaration_target(operator.symbol))
        })
        .or_else(|| {
            let operand = match node {
                ExpressionNode::Binary(binary) => binary.left,
                ExpressionNode::Unary(unary) => unary.operand,
                _ => return None,
            };
            (contract_resolution::checked_operand_type(program, facts, expression, operand)
                .and_then(|type_reference| program.primitive_type_reference(type_reference))
                .is_some()
                || expression_is_intrinsic_primitive_without_origin(program, operand)
                || checked_operator_expression_is_intrinsic_primitive(facts, operand)
                || expression_is_contextual_domain_primitive(program, expression, operand)
                || expression_is_contextual_statement_primitive(program, expression, operand))
            .then_some(CheckedResolutionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
            ))
        })
        .or_else(|| {
            operator_has_no_authored_spelling_candidate(program, node).then_some(
                CheckedResolutionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                ),
            )
        })
}

fn checked_operator_target_for_occurrence(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    node: &ExpressionNode,
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
) -> Option<CheckedResolutionTarget> {
    checked_operator_target(program, facts, expression, node).or_else(|| {
        program
            .expression_table
            .iter_expressions()
            .filter(|(candidate, _)| *candidate != expression)
            .filter(|(candidate, _)| {
                program
                    .expression_table
                    .authored_selection_occurrences(*candidate)
                    .any(|retained| retained == occurrence)
            })
            .find_map(|(candidate, candidate_node)| {
                checked_operator_target(program, facts, candidate, candidate_node)
            })
    })
}

fn checked_operator_expression_is_intrinsic_primitive(
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    facts.operators.uses.iter().any(|(_, operator_use)| {
        operator_use.expression == expression
            && operator_use.status == CheckedOperatorResolutionStatus::BuiltinFallback
    })
}

fn checked_structural_equality_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    call: &psi_typed_trees::expression::TableCallExpression,
) -> bool {
    if call.target.as_str() != "equals" || !call.target_symbol.is_valid() {
        return false;
    }

    let owners = program
        .machines()
        .iter()
        .filter(|machine| {
            machine.attached_data.is_some()
                && program.machine_states(machine).iter().any(|state| {
                    state.symbol == call.target_symbol && state.name.as_str() == "equals"
                })
        })
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return false;
    };
    let Some(carrier) = owner.attached_data.as_ref() else {
        return false;
    };
    if !program.conformances().iter().any(|conformance| {
        conformance.trait_name.as_str() == "Equatable"
            && conformance
                .carrier_name()
                .is_some_and(|candidate| candidate.as_str() == carrier.as_str())
    }) {
        return false;
    }

    let checked_targets = facts
        .flow
        .control
        .states
        .iter()
        .flat_map(|(_, state)| {
            facts
                .flow
                .control
                .calls
                .span_or_empty(state.calls)
                .iter()
                .map(move |checked_call| (state, checked_call))
        })
        .filter_map(|(state, checked_call)| {
            match crate::semantic_calls::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                checked_call.statement_index,
                checked_call.call_ordinal,
            ) {
                Some(crate::semantic_calls::CallSite::Expression {
                    expression: candidate,
                    ..
                }) if candidate == expression => Some(checked_call.target_symbol),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    !checked_targets.is_empty()
        && checked_targets
            .iter()
            .all(|target| *target == call.target_symbol)
}

/// Declaration contracts and other proof-static expressions are validated
/// without an executable value origin, so they do not always produce a
/// `CheckedOperatorUseFact`. Finalize their authored selection only when the
/// typed operands select one exact declared operator; partial or ambiguous
/// reconstruction remains unresolved and therefore rejects package custody.
fn resolve_authored_operator_without_use_fact<'program>(
    program: &'program TypedTrees,
    node: &ExpressionNode,
) -> Option<&'program psi_typed_trees::operator::OperatorDefinition> {
    use psi_language_core::OperatorSpelling;
    use psi_typed_trees::expression::BinaryOperator;

    let (spelling, operand_types) = match node {
        ExpressionNode::Binary(binary) => {
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                BinaryOperator::Equal => OperatorSpelling::Equal,
                BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                BinaryOperator::And
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::Or
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight => return None,
            };
            (
                spelling,
                vec![
                    Some(authored_operand_type(program, binary.left)?),
                    Some(authored_operand_type(program, binary.right)?),
                ],
            )
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = Some(authored_operand_type(program, indexed.collection)?);
            match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(range) => (
                    OperatorSpelling::Range,
                    vec![
                        collection,
                        authored_operand_type(program, range.start),
                        authored_operand_type(program, range.end),
                    ],
                ),
                _ => (
                    OperatorSpelling::Index,
                    vec![collection, authored_operand_type(program, indexed.index)],
                ),
            }
        }
        _ => return None,
    };
    let candidates =
        psi_typed_trees::operator::resolve_spelling_for_operands(program, spelling, &operand_types);
    let [candidate] = candidates.as_slice() else {
        return None;
    };
    Some(candidate.operator)
}

fn authored_operand_type(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => authored_operand_type(program, atomic.value),
        ExpressionNode::Borrow(borrow) => authored_operand_type(program, borrow.target),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Member(member) => type_reference_for_symbol(
            program,
            crate::flow::effective_member_symbol(program, member.receiver, member),
        ),
        ExpressionNode::Call(call) => {
            exact_named_operator_call(program, call).map(|operator| operator.return_type)
        }
        ExpressionNode::Name(path) => type_reference_for_symbol(program, path.symbol)
            .or_else(|| operator_contract_value_type(program, expression, path)),
        _ => None,
    }
}

fn operator_contract_value_type(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    path: &psi_typed_trees::expression::TableNamePath,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    let name = program
        .expression_table
        .name_path_members(path.members)
        .last()?
        .as_str();
    let mut types = program
        .operators()
        .iter()
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain)),
        )
        .filter(|operator| {
            program.operator_contracts(operator).iter().any(|contract| {
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .any(|fact| match fact {
                        psi_typed_trees::domain::ProofFact::Expression(root) => {
                            expression_tree_contains(program, *root, expression)
                        }
                        psi_typed_trees::domain::ProofFact::Membership(membership) => {
                            expression_tree_contains(program, membership.value, expression)
                        }
                        psi_typed_trees::domain::ProofFact::Proposition(_) => false,
                    })
            })
        })
        .filter_map(|operator| {
            if name == "result" {
                return Some(operator.return_type);
            }
            program
                .operator_parameters(operator)
                .iter()
                .find(|parameter| parameter.name.as_str() == name)
                .map(|parameter| parameter.type_reference)
        });
    let first = types.next()?;
    types
        .all(|type_reference| type_reference == first)
        .then_some(first)
}

fn expression_tree_contains(
    program: &TypedTrees,
    root: psi_typed_trees::expression::ExpressionHandle,
    target: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    use psi_typed_trees::expression::ExpressionNode;

    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if expression == target {
            return true;
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Atomic(atomic) => pending.push(atomic.value),
            ExpressionNode::Binary(binary) => {
                pending.push(binary.left);
                pending.push(binary.right);
            }
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            _ => {}
        }
    }
    false
}

fn operator_has_no_authored_spelling_candidate(
    program: &TypedTrees,
    node: &ExpressionNode,
) -> bool {
    use psi_language_core::OperatorSpelling;
    use psi_typed_trees::expression::BinaryOperator;
    let spelling = match node {
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::Add => OperatorSpelling::Add,
            BinaryOperator::Subtract => OperatorSpelling::Subtract,
            BinaryOperator::Multiply => OperatorSpelling::Multiply,
            BinaryOperator::Divide => OperatorSpelling::Divide,
            BinaryOperator::Modulo => OperatorSpelling::Modulo,
            BinaryOperator::Equal => OperatorSpelling::Equal,
            BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
            BinaryOperator::Less => OperatorSpelling::Less,
            BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
            BinaryOperator::Greater => OperatorSpelling::Greater,
            BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
            BinaryOperator::And
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::Or
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight => return true,
        },
        ExpressionNode::Indexed(indexed) => {
            if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) {
                OperatorSpelling::Range
            } else {
                OperatorSpelling::Index
            }
        }
        // Unary operators have no authored declaration dispatch surface.
        ExpressionNode::Unary(_) => return true,
        _ => return false,
    };
    psi_typed_trees::operator::resolve_spelling(program, spelling, None).is_empty()
}

fn contextual_domain_member_target(
    program: &TypedTrees,
    containing_expression: psi_typed_trees::expression::ExpressionHandle,
    member: &psi_typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let target_type = contextual_domain_target_type(program, containing_expression)?;
    contextual_self_member_symbol(program, member, target_type).and_then(declaration_target)
}

fn checked_member_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    expression: psi_typed_trees::expression::ExpressionHandle,
    member: &psi_typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    declaration_target(crate::flow::effective_member_symbol(
        program,
        member.receiver,
        member,
    ))
    .or_else(|| {
        contexts::checked_member_target_from_exact_owner(program, facts, expression, member).map(
            |target| match target {
                contexts::OwnerMemberTarget::Declaration(symbol) => {
                    CheckedResolutionTarget::Declaration(symbol)
                }
                contexts::OwnerMemberTarget::CollectionLength => {
                    CheckedResolutionTarget::Intrinsic(
                        AuthoredDeclarationSelectionIntrinsic::CollectionLength,
                    )
                }
            },
        )
    })
    .or_else(|| {
        let matching = facts
            .fact_call_projections
            .iter()
            .filter(|projection| projection.projection_expression == expression)
            .collect::<Vec<_>>();
        let [projection] = matching.as_slice() else {
            return None;
        };
        declaration_target(projection.field)
    })
    .or_else(|| checked_value_member_target(program, facts, member))
    .or_else(|| authored_member_target(program, member))
    .or_else(|| contextual_domain_member_target(program, expression, member))
    .or_else(|| contextual_statement_member_target(program, expression, member))
}

fn checked_value_member_target(
    program: &TypedTrees,
    facts: &CheckFacts,
    member: &psi_typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let mut resolved = None;
    for (_, value) in facts.values.expression_values(member.receiver) {
        if !value.type_reference.is_valid() {
            continue;
        }
        let target = member_symbol_from_type_reference(
            program,
            value.type_reference,
            member.member.as_str(),
        )
        .and_then(declaration_target)
        .or_else(|| {
            (member.member.as_str() == "len"
                && type_reference_is_collection(program, value.type_reference))
            .then_some(CheckedResolutionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::CollectionLength,
            ))
        });
        let Some(target) = target else {
            continue;
        };
        if resolved.is_some_and(|candidate| candidate != target) {
            return None;
        }
        resolved = Some(target);
    }
    resolved
}

fn authored_member_target(
    program: &TypedTrees,
    member: &psi_typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let receiver_type = authored_operand_type(program, member.receiver)?;
    member_symbol_from_type_reference(program, receiver_type, member.member.as_str())
        .and_then(declaration_target)
        .or_else(|| {
            (member.member.as_str() == "len"
                && type_reference_is_collection(program, receiver_type))
            .then_some(CheckedResolutionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::CollectionLength,
            ))
        })
}

fn contextual_statement_member_target(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    member: &psi_typed_trees::expression::TableMemberExpression,
) -> Option<CheckedResolutionTarget> {
    let mut resolved = None;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut expressions = Vec::new();
                crate::monomorphization::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                if !expressions.contains(&expression) {
                    continue;
                }

                let receiver_type = crate::flow::expression_type_reference_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    member.receiver,
                )
                .or_else(|| {
                    captured_entry_parameter_type_reference(program, state.symbol, member.receiver)
                })?;
                let target = member_symbol_from_type_reference(
                    program,
                    receiver_type,
                    member.member.as_str(),
                )
                .and_then(declaration_target)
                .or_else(|| {
                    (member.member.as_str() == "len"
                        && type_reference_is_collection(program, receiver_type))
                    .then_some(CheckedResolutionTarget::Intrinsic(
                        AuthoredDeclarationSelectionIntrinsic::CollectionLength,
                    ))
                })?;
                if resolved.is_some_and(|candidate| candidate != target) {
                    return None;
                }
                resolved = Some(target);
            }
        }
    }
    resolved
}

fn captured_entry_parameter_type_reference(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    use psi_typed_trees::expression::ExpressionNode;

    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };

    // Nested states capture the callable entry telescope lexically without
    // copying its parameters into each state's local telescope. Name checking
    // has already established an unambiguous machine-local binding; recover
    // that binding's authored type from the containing entry state.
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == state_symbol)
            .then_some(machine)
            .and_then(|machine| program.machine_states(machine).first())
            .and_then(|entry| {
                program
                    .state_parameters(entry)
                    .iter()
                    .find(|parameter| parameter.name == *name)
            })
            .map(|parameter| parameter.type_reference)
    })
}

fn member_symbol_from_type_reference(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    use psi_typed_trees::types::TypeReferenceNode;

    let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            return member_symbol_from_type_reference(program, *referee, member_name);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            return member_symbol_from_type_reference(program, *base_type, member_name);
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => (*base_symbol, base_name),
        TypeReferenceNode::Named { symbol, name } => (*symbol, name),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => return None,
    };
    let data = program.data_definitions().iter().find(|definition| {
        (symbol.is_valid() && definition.symbol == symbol) || definition.name == *name
    })?;
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == member_name =>
            {
                Some(field.symbol)
            }
            psi_typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == member_name =>
            {
                Some(variant.symbol)
            }
            psi_typed_trees::data::DataMember::Variant(variant) => program
                .data_payload_fields(variant)
                .iter()
                .find_map(|field| (field.name.as_str() == member_name).then_some(field.symbol)),
            _ => None,
        })
}

fn type_reference_is_collection(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    use psi_typed_trees::types::TypeReferenceNode;
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_is_collection(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_collection(program, *base_type)
        }
        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => true,
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => false,
    }
}

fn expression_is_contextual_domain_primitive(
    program: &TypedTrees,
    containing_expression: psi_typed_trees::expression::ExpressionHandle,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    let Some(target_type) = contextual_domain_target_type(program, containing_expression) else {
        return false;
    };
    contextual_expression_type_reference(program, expression, target_type)
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some()
}

fn expression_is_contextual_statement_primitive(
    program: &TypedTrees,
    containing_expression: psi_typed_trees::expression::ExpressionHandle,
    operand: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    let mut found = false;
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut expressions = Vec::new();
                crate::monomorphization::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                if !expressions.contains(&containing_expression) {
                    continue;
                }

                let Some(type_reference) = crate::flow::expression_type_reference_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    operand,
                )
                .or_else(|| {
                    captured_entry_parameter_type_reference(program, state.symbol, operand)
                }) else {
                    return false;
                };
                if program.primitive_type_reference(type_reference).is_none() {
                    return false;
                }
                found = true;
            }
        }
    }
    found
}

fn contextual_expression_type_reference(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    domain_target_type: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) if contextual_self_path(program, path) => {
            Some(domain_target_type)
        }
        ExpressionNode::Member(member) => {
            contextual_self_member_symbol(program, member, domain_target_type)
                .and_then(|symbol| type_reference_for_symbol(program, symbol))
        }
        ExpressionNode::Borrow(inner) => {
            contextual_expression_type_reference(program, inner.target, domain_target_type)
        }
        _ => None,
    }
}

fn contextual_self_member_symbol(
    program: &TypedTrees,
    member: &psi_typed_trees::expression::TableMemberExpression,
    domain_target_type: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver) else {
        return None;
    };
    if !contextual_self_path(program, path) {
        return None;
    }
    crate::flow::resolve_member_symbol_from_type_symbol(
        program,
        program.type_reference_table.type_symbol(domain_target_type),
        member.member.as_str(),
    )
}

fn contextual_self_path(
    program: &TypedTrees,
    path: &psi_typed_trees::expression::TableNamePath,
) -> bool {
    let members = program.expression_table.name_path_members(path.members);
    !path.symbol.is_valid() && members.len() == 1 && members[0].as_str() == "self"
}

fn contextual_domain_target_type(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    for domain in program.domain_definitions() {
        for fact in program.proof_facts(domain) {
            let root = match fact {
                psi_typed_trees::domain::ProofFact::Expression(root) => *root,
                psi_typed_trees::domain::ProofFact::Membership(membership) => membership.value,
                psi_typed_trees::domain::ProofFact::Proposition(_) => continue,
            };
            if expression_contains(program, root, expression, &mut Vec::new()) {
                return Some(domain.target_type);
            }
        }
    }
    None
}

fn expression_contains(
    program: &TypedTrees,
    root: psi_typed_trees::expression::ExpressionHandle,
    target: psi_typed_trees::expression::ExpressionHandle,
    visited: &mut Vec<psi_typed_trees::expression::ExpressionHandle>,
) -> bool {
    if !root.is_valid() || visited.contains(&root) {
        return false;
    }
    if root == target {
        return true;
    }
    visited.push(root);
    match program.expression_table.expression(root) {
        ExpressionNode::Atomic(atomic) => {
            expression_contains(program, atomic.value, target, visited)
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|child| expression_contains(program, *child, target, visited)),
        ExpressionNode::Binary(binary) => {
            expression_contains(program, binary.left, target, visited)
                || expression_contains(program, binary.right, target, visited)
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_contains(program, call.receiver, target, visited))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|child| expression_contains(program, *child, target, visited))
        }
        ExpressionNode::Cast(cast) => expression_contains(program, cast.value, target, visited),
        ExpressionNode::Indexed(indexed) => {
            expression_contains(program, indexed.collection, target, visited)
                || expression_contains(program, indexed.index, target, visited)
        }
        ExpressionNode::Member(member) => {
            expression_contains(program, member.receiver, target, visited)
        }
        ExpressionNode::Borrow(inner) => {
            expression_contains(program, inner.target, target, visited)
        }
        ExpressionNode::Range(range) => {
            expression_contains(program, range.start, target, visited)
                || expression_contains(program, range.end, target, visited)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_contains(program, field.value, target, visited)),
        ExpressionNode::Unary(unary) => {
            expression_contains(program, unary.operand, target, visited)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn expression_is_intrinsic_primitive_without_origin(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    let type_reference = match program.tables.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) | ExpressionNode::Float(_) | ExpressionNode::Integer(_) => {
            return true;
        }
        ExpressionNode::Name(path) => type_reference_for_symbol(program, path.symbol),
        ExpressionNode::Call(call) => program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine))
            .find_map(|state| (state.symbol == call.target_symbol).then_some(state.return_type)),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Member(member) => {
            if contexts::checked_member_target_from_exact_owner(
                program,
                &CheckFacts::default(),
                expression,
                member,
            ) == Some(contexts::OwnerMemberTarget::CollectionLength)
            {
                return true;
            }
            type_reference_for_symbol(
                program,
                crate::flow::effective_member_symbol(program, member.receiver, member),
            )
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                psi_typed_trees::expression::BinaryOperator::And
                    | psi_typed_trees::expression::BinaryOperator::BitwiseAnd
                    | psi_typed_trees::expression::BinaryOperator::BitwiseOr
                    | psi_typed_trees::expression::BinaryOperator::BitwiseXor
                    | psi_typed_trees::expression::BinaryOperator::Or
                    | psi_typed_trees::expression::BinaryOperator::ShiftLeft
                    | psi_typed_trees::expression::BinaryOperator::ShiftRight
            ) =>
        {
            return true;
        }
        ExpressionNode::Binary(binary) => {
            use psi_language_core::OperatorSpelling;
            use psi_typed_trees::expression::BinaryOperator;

            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                BinaryOperator::Equal => OperatorSpelling::Equal,
                BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                BinaryOperator::And
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::Or
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight => unreachable!("handled above"),
            };
            let operand_types = [
                authored_operand_type(program, binary.left),
                authored_operand_type(program, binary.right),
            ];
            if operand_types.iter().all(Option::is_none)
                || !psi_typed_trees::operator::resolve_spelling_for_operands(
                    program,
                    spelling,
                    &operand_types,
                )
                .is_empty()
            {
                return false;
            }
            return expression_is_intrinsic_primitive_without_origin(program, binary.left)
                || expression_is_intrinsic_primitive_without_origin(program, binary.right);
        }
        ExpressionNode::Unary(_) => return true,
        ExpressionNode::Borrow(inner) => {
            return expression_is_intrinsic_primitive_without_origin(program, inner.target);
        }
        _ => None,
    };
    type_reference
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
        .is_some()
}

/// Return whether an early typed operator expression cannot select an authored
/// operator declaration. Ordinary checked lowering remains responsible for
/// rejecting a semantically invalid builtin use.
pub(crate) fn typed_operator_has_no_authored_selection(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    let node = program.expression_table.expression(expression);
    operator_has_no_authored_spelling_candidate(program, node)
}

pub(crate) fn typed_operator_authored_selection_candidates(
    program: &TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Vec<SymbolHandle> {
    use psi_language_core::OperatorSpelling;
    use psi_typed_trees::expression::BinaryOperator;

    let (spelling, operand_types) = match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                BinaryOperator::Divide => OperatorSpelling::Divide,
                BinaryOperator::Modulo => OperatorSpelling::Modulo,
                BinaryOperator::Equal => OperatorSpelling::Equal,
                BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                BinaryOperator::Less => OperatorSpelling::Less,
                BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                BinaryOperator::Greater => OperatorSpelling::Greater,
                BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                BinaryOperator::And
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::Or
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight => return Vec::new(),
            };
            (
                spelling,
                vec![
                    authored_operand_type(program, binary.left),
                    authored_operand_type(program, binary.right),
                ],
            )
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = authored_operand_type(program, indexed.collection);
            match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(range) => (
                    OperatorSpelling::Range,
                    vec![
                        collection,
                        authored_operand_type(program, range.start),
                        authored_operand_type(program, range.end),
                    ],
                ),
                _ => (
                    OperatorSpelling::Index,
                    vec![collection, authored_operand_type(program, indexed.index)],
                ),
            }
        }
        ExpressionNode::Unary(_) => return Vec::new(),
        _ => return Vec::new(),
    };

    psi_typed_trees::operator::resolve_spelling_for_operands(program, spelling, &operand_types)
        .into_iter()
        .map(|candidate| candidate.operator.symbol)
        .collect()
}

fn type_reference_for_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    if let Some(type_reference) = program
        .const_declarations()
        .iter()
        .find_map(|declaration| (declaration.symbol == symbol).then_some(declaration.declared_type))
    {
        return Some(type_reference);
    }
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            match member {
                psi_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return Some(field.type_reference);
                }
                psi_typed_trees::data::DataMember::Variant(variant) => {
                    if let Some(type_reference) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find_map(|field| (field.symbol == symbol).then_some(field.type_reference))
                    {
                        return Some(type_reference);
                    }
                }
                _ => {}
            }
        }
    }
    for machine in program.machines() {
        if let Some(type_reference) = program
            .machine_owned_data(machine)
            .iter()
            .find_map(|owned| (owned.symbol == symbol).then_some(owned.type_reference))
        {
            return Some(type_reference);
        }
        for state in program.machine_states(machine) {
            if let Some(type_reference) =
                program
                    .state_parameters(state)
                    .iter()
                    .find_map(|parameter| {
                        (parameter.symbol == symbol).then_some(parameter.type_reference)
                    })
            {
                return Some(type_reference);
            }
            if let Some(type_reference) = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    psi_typed_trees::statement::StatementNode::LocalData(local)
                        if local.symbol == symbol =>
                    {
                        Some(local.type_reference)
                    }
                    _ => None,
                })
            {
                return Some(type_reference);
            }
        }
    }
    for definition in program.traits() {
        for signature in program.trait_machine_signatures(definition) {
            if let Some(type_reference) = program
                .state_signature_parameters(signature)
                .iter()
                .find_map(|parameter| {
                    (parameter.symbol == symbol).then_some(parameter.type_reference)
                })
            {
                return Some(type_reference);
            }
        }
    }
    for proposition in program.propositions() {
        if let Some(type_reference) = program
            .proposition_parameters(proposition)
            .iter()
            .find_map(|parameter| (parameter.symbol == symbol).then_some(parameter.type_reference))
        {
            return Some(type_reference);
        }
    }
    None
}

fn late_binding_ordinal(
    program: &TypedTrees,
    prior_occurrences: &[AuthoredDeclarationSelectionOccurrenceId],
    binding: AuthoredDeclarationSelectionLateBinding,
) -> usize {
    prior_occurrences
        .iter()
        .filter(|occurrence| {
            program
                .authored_declaration_selections()
                .get(**occurrence)
                .is_some_and(|selection| {
                    selection.target() == AuthoredDeclarationSelectionTarget::LateBound(binding)
                })
        })
        .count()
}

fn push_consistent_resolution(
    resolutions: &mut Vec<CheckedResolution>,
    candidate: CheckedResolution,
) -> Result<(), Diagnostic> {
    if let Some(existing) = resolutions
        .iter()
        .find(|resolution| resolution.occurrence == candidate.occurrence)
    {
        if *existing != candidate {
            return Err(Diagnostic::error(format!(
                "authored declaration selection occurrence {} resolved inconsistently across compiler-derived copies",
                candidate.occurrence.ordinal()
            )));
        }
        return Ok(());
    }
    resolutions.push(candidate);
    Ok(())
}

fn finalization_diagnostic(
    resolution: CheckedResolution,
    error: AuthoredDeclarationSelectionFinalizationError,
) -> Diagnostic {
    Diagnostic::error(format!(
        "failed to finalize authored declaration selection occurrence {}: {error:?}",
        resolution.occurrence.ordinal()
    ))
}
