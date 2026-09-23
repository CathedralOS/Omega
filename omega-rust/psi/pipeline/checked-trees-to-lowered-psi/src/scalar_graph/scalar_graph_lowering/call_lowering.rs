//! Lowering scalar calls, direct call bindings and graph successors.

use crate::emission::expression_validation::validate_direct_parameter_types;
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::emission::operation_emission::calls::{LoweredDirectCallBinding, ScalarCallCrashScope};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::expression_preparation::bindings as storage;
use crate::expression_preparation::qualifications::PreparedScalarQualifications;
use crate::expression_preparation::source_custody;
use crate::scalar_graph::scalar_computations as computations;
use crate::scalar_graph::scalar_contracts::LoweredProofTerm;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph;
use crate::scalar_graph::scalar_graph_lowering::structural_values;
use crate::scalar_graph::{
    CheckedScalarExpressionRole, CheckedScalarSuccessor, CheckedTrees, LoweringError,
    QualifiedScalarType, StructuralAccess, StructuralArgument, StructuralPathSegment,
    StructuralTypeDeclaration, allocate_dense, place_id, scalar_carriers, unsupported,
};
use checked_trees::CheckedErasedProofParameterPlan;

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_checked_direct_call_binding(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    statement_ordinal: u32,
    binding_ordinal: u32,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    call_ordinal: u32,
    argument_count: u32,
    result_type: QualifiedScalarType,
    caller_value_types: &[QualifiedScalarType],
    scalar_bindings: &storage::ScalarBindings,
    caller_erased_proof_parameters: &[CheckedErasedProofParameterPlan],
) -> Result<LoweredDirectCallBinding, LoweringError> {
    source_custody::direct_calls::validate(
        checked,
        caller_machine,
        caller_state,
        statement_ordinal,
        binding_ordinal,
        target_machine,
        target_state,
        call_ordinal,
        argument_count,
        result_type.scalar_type,
    )?;
    let arguments = (0..argument_count)
        .map(|argument_ordinal| {
            scalar_bindings.expression_at(
                checked,
                caller_state,
                statement_ordinal,
                CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal,
                    argument_ordinal,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    lower_scalar_call(
        checked,
        qualifications,
        caller_machine,
        caller_state,
        statement_ordinal,
        target_machine,
        target_state,
        call_ordinal,
        result_type,
        caller_value_types,
        scalar_bindings,
        arguments,
        Vec::new(),
        ScalarCallCrashScope::CallerValues,
        caller_erased_proof_parameters,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_scalar_call(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    statement_ordinal: u32,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    call_ordinal: u32,
    result_type: QualifiedScalarType,
    caller_value_types: &[QualifiedScalarType],
    scalar_bindings: &storage::ScalarBindings,
    arguments: Vec<LoweredDirectExpression>,
    structural_arguments: Vec<StructuralArgument>,
    crash_scope: ScalarCallCrashScope,
    caller_erased_proof_parameters: &[CheckedErasedProofParameterPlan],
) -> Result<LoweredDirectCallBinding, LoweringError> {
    let target = if structural_arguments.is_empty() {
        crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee::find(
            checked,
            target_machine,
        )?
    } else {
        crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
            checked,
            target_machine,
        )?
    };
    if target.structural_parameters().len() != structural_arguments.len()
        || !target.entry_claims().is_empty()
        || structural_arguments.iter().any(|argument| {
            !argument.path.is_empty()
                && (argument.access != StructuralAccess::SharedBorrow
                    || argument
                        .path
                        .iter()
                        .any(|segment| !matches!(segment, StructuralPathSegment::Field(_))))
        })
    {
        return unsupported("computed scalar call requires exact structural custody");
    }
    let (target_parameter_types, target_result_type) =
        qualifications.scalar_state_types(checked, target_state)?;
    if target.entry_state()? != target_state {
        return unsupported("direct scalar call must target the callee entry state");
    }
    if target_result_type != result_type {
        return unsupported("direct scalar call result type must match its local binding");
    }
    if arguments.len() != target_parameter_types.len() {
        return unsupported("direct scalar call argument count must match the callee signature");
    }
    for (expression, target_type) in arguments.iter().zip(&target_parameter_types) {
        if expression.value_type(caller_value_types)? != *target_type {
            return unsupported(
                "checked scalar call argument type must match its callee parameter",
            );
        }
        validate_direct_parameter_types(expression, &scalar_carriers(caller_value_types))?;
    }
    // Erased actuals sit on the proof-only lane the checker bound under
    // `ErasedUnitCallArgument`, in the target's erased-formal order.
    let erased_arguments = (0..target.erased_parameters().len())
        .map(|erased_ordinal| {
            scalar_bindings.expression_at(
                checked,
                caller_state,
                statement_ordinal,
                CheckedScalarExpressionRole::ErasedUnitCallArgument {
                    call_ordinal,
                    erased_ordinal: u32::try_from(erased_ordinal).map_err(|_| {
                        LoweringError::Unsupported(
                            "scalar call erased argument ordinal exceeds u32",
                        )
                    })?,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Proof-only actuals sit on the adjacent lane under the same
    // `ErasedUnitCallArgument` role, in the target's erased-proof roster
    // order; each converts through the caller machine's roster so forwarded
    // `Formal` positions stay dense.
    let erased_proof_arguments = (0..target.erased_proof_parameters().len())
        .map(|erased_ordinal| {
            let role = checked_trees::CheckedProofTermRole::ErasedUnitCallArgument {
                call_ordinal,
                erased_ordinal: u32::try_from(erased_ordinal).map_err(|_| {
                    LoweringError::Unsupported("scalar call erased proof ordinal exceeds u32")
                })?,
            };
            let term = checked
                .facts
                .values
                .proof_terms
                .term_at(caller_state, statement_ordinal, role)
                .ok_or(LoweringError::Unsupported(
                    "scalar call erased proof actual is absent",
                ))?;
            crate::scalar_graph::scalar_contracts::checked_proof_term(
                checked,
                term,
                caller_erased_proof_parameters,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let checked_call = checked
        .facts
        .contract_plans
        .for_machine(caller_machine)
        .and_then(|plan| {
            plan.crash
                .checked_call_at(caller_state, statement_ordinal, call_ordinal)
        })
        .ok_or(LoweringError::Unsupported(
            "direct scalar call has no matching checked crash-refinement row",
        ))?;
    if checked_call.target_machine() != target_machine
        || checked_call.target_state() != target_state
    {
        return unsupported("checked scalar call target disagrees with crash refinement");
    }
    let target_contract = checked
        .facts
        .contract_plans
        .for_machine(target_machine)
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no checked contract plan",
        ))?;
    if checked_call.target_contract_report_fingerprint() != target_contract.report_fingerprint
        || checked_call.target_contract_commitment() != target_contract.commitment
    {
        return unsupported("checked scalar call target contract identity disagrees");
    }
    // Computed arguments have already become values. Bind the pinned callee
    // routes to those values, as ordinary staged calls do, rather than trying
    // to turn their effectful source expressions into pure caller predicates.
    // The pinned ceiling is the callee's effective crash routes — authored
    // buckets for a published ceiling or the union of retained body evidence
    // for an inferred contract — in the callee's parameter namespace, exactly
    // the ceiling the emitted machine contract publishes and the verifier
    // substitutes back at this call.
    let target_routes = crate::unit::effective_crash_routes(checked, target_machine)?;
    let crash_continuations = match crash_scope {
        ScalarCallCrashScope::CallerValues => checked_call.surviving_buckets().to_vec(),
        ScalarCallCrashScope::Arguments => target_routes.clone(),
    };
    // This summary only tells graph preparation whether the checked call can
    // crash; an identity-only predicate suffices for that question. Emission
    // lowers `target_routes` against evaluated arguments, not this refined
    // summary. Its structured-term checks and independent callee substitution
    // still reject an unsupported or mismatched executable continuation.
    Ok(LoweredDirectCallBinding {
        source_coordinate: SourceCallCoordinate {
            state: caller_state,
            statement_index: usize::try_from(statement_ordinal).map_err(|_| {
                LoweringError::Unsupported("scalar call statement ordinal exceeds usize")
            })?,
            call_ordinal: usize::try_from(call_ordinal)
                .map_err(|_| LoweringError::Unsupported("scalar call ordinal exceeds usize"))?,
        },
        target_machine,
        result_type,
        arguments,
        erased_arguments,
        erased_proof_arguments,
        structural_arguments,
        // The selected body owns storage even when its public signature has
        // only scalars. Graph and ordered-body callers use the same decision.
        uses_structural_frame: target.requires_structural_frame(),
        crash_continuations,
        parameter_relative_crash_routes: target_routes,
    })
}

pub(crate) fn lower_scalar_graph_successor(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    states: &[checked_trees::CheckedScalarStateGraph],
    source_state: symbols::SymbolHandle,
    source_value_types: &[QualifiedScalarType],
    successor: &CheckedScalarSuccessor,
    scalar_bindings: &storage::ScalarBindings,
    computations: &mut computations::Expansion<'_>,
    structural_types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<
    (
        usize,
        Vec<LoweredDirectExpression>,
        Vec<LoweredDirectExpression>,
        Vec<LoweredProofTerm>,
    ),
    LoweringError,
> {
    source_custody::validate_successor(checked, source_state, successor)?;
    let Some(target) = states
        .iter()
        .position(|candidate| candidate.state == successor.target)
    else {
        return unsupported("scalar graph successor must belong to the selected machine");
    };
    let (target_parameter_types, _) =
        qualifications.scalar_state_types(checked, states[target].state)?;
    let plans = &checked.facts.flow.terminal_scalar_graphs;
    let scalar_arguments = plans
        .scalar_arguments
        .span(successor.scalar_arguments)
        .ok_or(LoweringError::Unsupported(
            "scalar successor argument span is stale",
        ))?;
    if scalar_arguments.len() != target_parameter_types.len() {
        return unsupported(
            "scalar graph successor bindings must match the target parameter count",
        );
    }
    let source = states
        .iter()
        .find(|state| state.state == source_state)
        .ok_or(LoweringError::Unsupported(
            "scalar successor lost its source state",
        ))?;
    let (_, source_state_typed) = source_custody::authored_state(checked, source_state)?;
    let mut structural_effects = Vec::new();
    let mut structural_arguments = Vec::new();
    for transfer in plans
        .structural_transfers
        .span(successor.structural_transfers)
        .ok_or(LoweringError::Unsupported(
            "scalar successor transfer span is stale",
        ))?
        .iter()
    {
        match transfer.source {
            checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
                let parameter = source.structural_parameters.get(index as usize).ok_or(
                    LoweringError::Unsupported("scalar successor transfer parameter is absent"),
                )?;
                let parameter_index = checked
                    .typed
                    .state_parameters(source_state_typed)
                    .iter()
                    .take(parameter.position as usize)
                    .filter(|parameter| {
                        !parameter.relevance.is_erased()
                            && checked
                                .primitive_type_reference(parameter.type_reference)
                                .is_none()
                    })
                    .count();
                structural_arguments.push(scalar_bindings.owned_argument(
                    &checked_trees::CheckedUnitStructuralArgumentPlan {
                        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                            parameter_index: u32::try_from(parameter_index).ok().ok_or(
                                LoweringError::Unsupported(
                                    "scalar successor transfer parameter index exceeds the host type",
                                ),
                            )?,
                        },
                        path: Vec::new(),
                        type_identity: parameter.type_identity.clone(),
                        access: parameter.access,
                    },
                )?);
            }
            checked_trees::CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
                parameter_index,
                expression,
            }
            | checked_trees::CheckedStructuralControlTransferSourcePlan::ElementViewSubslice {
                parameter_index,
                expression,
            } => {
                let element = matches!(
                    transfer.source,
                    checked_trees::CheckedStructuralControlTransferSourcePlan::ElementViewSubslice {
                        ..
                    }
                );
                let retained_source = source
                    .structural_parameters
                    .get(parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "scalar successor subslice source is absent",
                    ))?;
                let retained_target = states[target]
                    .structural_parameters
                    .get(transfer.target_parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "scalar successor subslice target is absent",
                    ))?;
                let checked_trees::expression::ExpressionNode::Indexed(indexed) =
                    checked.expression_table.expression(expression)
                else {
                    return unsupported("scalar successor subslice lost its indexed source");
                };
                let checked_trees::expression::ExpressionNode::Range(range) =
                    checked.expression_table.expression(indexed.index)
                else {
                    return unsupported("scalar successor subslice lost its authored range");
                };
                let endpoint =
                    |handle: checked_trees::expression::ExpressionHandle,
                     role: CheckedScalarExpressionRole|
                     -> Result<Option<LoweredDirectExpression>, LoweringError> {
                        if !handle.is_valid() {
                            return Ok(None);
                        }
                        let expression = scalar_bindings.expression_at(
                            checked,
                            source_state,
                            successor.statement_ordinal,
                            role,
                        )?;
                        validate_direct_parameter_types(
                            &expression,
                            &scalar_carriers(source_value_types),
                        )?;
                        Ok(Some(expression))
                    };
                let start = endpoint(
                    range.start,
                    CheckedScalarExpressionRole::TransitionSubsliceStart {
                        argument_ordinal: retained_target.position,
                    },
                )?
                .ok_or(LoweringError::Unsupported(
                    "scalar successor subslice lost its start endpoint",
                ))?;
                let end = endpoint(
                    range.end,
                    CheckedScalarExpressionRole::TransitionSubsliceEnd {
                        argument_ordinal: retained_target.position,
                    },
                )?;
                let source = scalar_bindings.shared_structural_argument(
                    &checked_trees::CheckedUnitStructuralArgumentPlan {
                        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                            parameter_index,
                        },
                        path: Vec::new(),
                        type_identity: retained_source.type_identity.clone(),
                        access: checked_trees::CheckedStructuralAccess::SharedBorrow,
                    },
                    source_custody::authored_state(checked, source_state)?.0,
                    checked
                        .state_parameters(source_custody::authored_state(checked, source_state)?.1),
                )?;
                let structural_type = structural_types
                    .iter()
                    .find(|declaration| declaration.identity == retained_target.type_identity)
                    .ok_or(LoweringError::Unsupported(
                        "scalar successor subslice lost its view carrier",
                    ))?
                    .id;
                let place = place_id(allocate_dense(next_place)?);
                structural_effects.push(if element {
                    prepared_graph::LoweredScalarEffect::ElementViewSubslice {
                        source: source.place,
                        start,
                        end,
                        place,
                        structural_type,
                    }
                } else {
                    prepared_graph::LoweredScalarEffect::ByteSequenceSubslice {
                        source: source.place,
                        start,
                        end,
                        place,
                        structural_type,
                    }
                });
                structural_arguments.push(StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                });
            }
            _ => {
                return unsupported("scalar successor cannot lower this structural transfer");
            }
        }
    }
    // `exit_target` answers a lowered branch-state index: the edge lands on
    // the cleanup wrapper it may have pushed, not on the checked target's
    // position in `states`. Keep the two index spaces apart — the erased
    // proof roster below still reads `states[target]` by checked position.
    let lowered_target = structural_values::exit_target(
        checked,
        source_state,
        scalar_bindings,
        &target_parameter_types,
        &mut structural_arguments,
        target,
        computations,
        structural_types,
        next_place,
    )?;
    if let Some(entry) = computations.successor(
        source_state,
        successor,
        scalar_bindings,
        source_value_types,
        lowered_target,
        &target_parameter_types,
        &structural_arguments,
        &structural_effects,
    )? {
        return Ok((
            entry,
            computations::parameters(source_value_types),
            Vec::new(),
            Vec::new(),
        ));
    }
    let arguments = scalar_arguments
        .iter()
        .map(|argument| argument.argument_ordinal)
        .zip(&target_parameter_types)
        .map(|(argument_ordinal, target_type)| {
            let expression = scalar_bindings.expression_at(
                checked,
                source_state,
                successor.statement_ordinal,
                if successor.is_continuation {
                    CheckedScalarExpressionRole::TransitionContinuationArgument { argument_ordinal }
                } else {
                    CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                },
            )?;
            validate_direct_parameter_types(&expression, &scalar_carriers(source_value_types))?;
            (expression.value_type(source_value_types)? == *target_type)
                .then_some(expression)
                .ok_or(LoweringError::Unsupported(
                    "checked scalar successor expression type must match its target",
                ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let erased_arguments = plans
        .scalar_arguments
        .span(successor.erased_arguments)
        .ok_or(LoweringError::Unsupported(
            "scalar successor erased argument span is stale",
        ))?
        .iter()
        .map(|argument| {
            let expression = scalar_bindings.expression_at(
                checked,
                source_state,
                successor.statement_ordinal,
                if successor.is_continuation {
                    CheckedScalarExpressionRole::TransitionContinuationArgument {
                        argument_ordinal: argument.argument_ordinal,
                    }
                } else {
                    CheckedScalarExpressionRole::TransitionArgument {
                        argument_ordinal: argument.argument_ordinal,
                    }
                },
            )?;
            validate_direct_parameter_types(&expression, &scalar_carriers(source_value_types))?;
            let erased_type: QualifiedScalarType =
                crate::emission::scalar_types::terminal_scalar_type(argument.primitive_type)?
                    .into();
            (expression.value_type(source_value_types)? == erased_type)
                .then_some(expression)
                .ok_or(LoweringError::Unsupported(
                    "checked scalar successor erased expression type must match its target",
                ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let erased_proof_arguments = plans
        .erased_proof_arguments
        .span(successor.erased_proof_arguments)
        .ok_or(LoweringError::Unsupported(
            "scalar successor erased proof span is stale",
        ))?;
    if erased_proof_arguments.len() != states[target].erased_proof_parameters.len() {
        return unsupported("scalar graph successor erased proof roster drifted from its target");
    }
    let erased_proof_arguments = erased_proof_arguments
        .iter()
        .map(|term| {
            crate::scalar_graph::scalar_contracts::checked_proof_term(
                checked,
                term,
                &source.erased_proof_parameters,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    // The edge lands on the lowered successor frontier `exit_target` chose —
    // the affine-cleanup wrapper when owners had to be rebound — never on the
    // checked-state position still held by `target`.
    Ok((
        lowered_target,
        arguments,
        erased_arguments,
        erased_proof_arguments,
    ))
}
