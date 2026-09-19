//! Building one checked machine from its typed states and statements.

use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::call_occurrences;
use crate::execution::terminal_unit::control::call_results::{
    bind_scalar_call_result, bind_structural_call_result, checked_unit_scalar_result_local,
    checked_unit_structural_result_local,
};
use crate::execution::terminal_unit::statement_sequence;
use crate::execution::terminal_unit::{
    BTreeSet, CheckFacts, CheckedStructuralAccess, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralTypeShape, ExpectedCallValueResult,
    ExpressionNode, Multiplicity, ShapeCollector, StatementNode, SymbolHandle, TypeReferenceNode,
    TypedTrees, build_affine_array_construction_prefix, build_call_operation,
    build_selected_ieee_float_fma, build_selected_operator_scalar_call,
    build_selected_operator_structural_call, build_selected_operator_structural_scalar_call,
    build_structural_scalar_field_store, build_unit_trivial_affine_locals,
    build_write_only_primitive_store, checked_provider_attachment_requirements,
    checked_state_contracts_supported, entry_claims, free_fused_service_scalar_signature,
    free_selected_operator_structural_signature, free_structural_scalar_signature,
    fused_service_scalar_signature, is_reference, is_unit, machine_binders, receiver_aliases,
    return_unit_affine_discards, scalar_expression_local_suffix,
    selected_ieee_float_fma_result_locals, selected_operator_scalar_result_local,
    selected_operator_structural_result_local, state_flow, structural_scalar_signature,
    structural_signature, type_graph_requires_nominal_drop,
};

/// Test convenience: the traced builder without a trace.
#[cfg(test)]
pub(crate) fn build_checked_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CheckedUnitEffectMachinePlan> {
    build_checked_machine_traced(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        selected_operator_applications,
        selected_ieee_float_fma_applications,
        call_frames,
        &LocalConstructionTrace::default(),
    )
}

/// `build_checked_machine` with a trace of where the last attempt stopped:
/// the ambient attempt's phase, or the retained-self retry's when that
/// retry ran.
pub(crate) fn build_checked_machine_traced(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectMachinePlan> {
    build_checked_machine_with_trace(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        selected_operator_applications,
        selected_ieee_float_fma_applications,
        false,
        call_frames,
        trace,
    )
    .or_else(|| {
        // Retain borrowed self when ambient attachment cannot plan the body.
        // Completed callee signatures can also demand it during receiver-call
        // reconciliation. Retain it unconditionally once the entry bridge
        // provisions the receiver and passes its loan.
        let [state] = program.machine_states(machine) else {
            return None;
        };
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_self && is_reference(program, parameter.type_reference))
            .then(|| {
                build_checked_machine_with_trace(
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    machine,
                    selected_operator_applications,
                    selected_ieee_float_fma_applications,
                    true,
                    call_frames,
                    trace,
                )
            })
            .flatten()
    })
}

pub(crate) fn build_checked_machine_with(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
    retain_reference_self: bool,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CheckedUnitEffectMachinePlan> {
    build_checked_machine_with_trace(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        selected_operator_applications,
        selected_ieee_float_fma_applications,
        retain_reference_self,
        call_frames,
        &LocalConstructionTrace::default(),
    )
}

fn build_checked_machine_with_trace(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
    retain_reference_self: bool,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectMachinePlan> {
    trace.phase("single-state body");
    let [state] = program.machine_states(machine) else {
        return None;
    };
    // Ambient attachment cannot replace runtime field observations or published
    // crash predicates: both need the invocation's actual receiver. Contextual
    // cleanup requirements retain their separate receipt-bound environment.
    let retain_reference_self = retain_reference_self
        // Scalar computation calls retain their declared shared receiver as an
        // operand even when the body does not read it. Body specialization must
        // not change the producer/consumer signature of that ordinary call.
        || (program.primitive_type_reference(state.return_type).is_some()
            && program.state_parameters(state).iter().any(|parameter| parameter.is_self
                && matches!(program.type_reference_table.type_reference(parameter.type_reference),
                    TypeReferenceNode::Reference { access: language_semantics::ReferenceAccess::Shared, .. })))
        || crate::execution::terminal_unit::receiver_calls::reads_receiver(program, facts, state)
        || facts
            .contract_plans
            .for_machine(machine.symbol)
            .is_some_and(|contract| {
                contract.crash.published().iter().any(|bucket| {
                    bucket
                        .alternative_guards()
                        .iter()
                        .any(|guard| matches!(guard, checked_trees::CrashRouteGuard::Predicate(_)))
                })
            });
    trace.phase("result type");
    if !is_unit(program, state.return_type)
        && validation::reference_result_custody::parts(program, state.return_type).is_none()
        && !validation::reference_result_custody::is_reference_record(program, state.return_type)
        && !validation::is_closed_primitive_array_type(program, state.return_type)
        && !validation::has_plain_owned_contents_with_numeric_constraints(
            program,
            state.return_type,
        )
        && program
            .primitive_type_reference(state.return_type)
            .is_none()
    {
        return None;
    }
    trace.phase("result contract");
    // Entry predicates and normal guarantees belong to the shared invocation
    // contract, independent of the operation that produces the result. Result
    // refinements still need their separate qualification evidence.
    if program
        .primitive_type_reference(state.return_type)
        .is_some()
        && (program
            .machine_contracts(machine)
            .iter()
            .chain(program.state_contracts(state))
            .any(|contract| {
                !matches!(
                    contract.kind,
                    typed_trees::signature::SignatureContractKind::Crashes { .. }
                        | typed_trees::signature::SignatureContractKind::Requires
                        | typed_trees::signature::SignatureContractKind::Ensures
                ) || contract.binding.is_some()
            })
            || (matches!(
                program
                    .type_reference_table
                    .type_reference(state.return_type),
                TypeReferenceNode::Constrained { .. }
            ) && !validation::is_arithmetic_policy_only_integer(program, state.return_type)
                && validation::closed_scalar_result_range(program, state.return_type).is_none()))
    {
        return None;
    }
    trace.phase("signature");
    let statements = program.statement_table.statements(state.statement_nodes);
    let selected_scalar_result_local = selected_operator_scalar_result_local(
        program,
        machine,
        state,
        statements,
        selected_operator_applications,
    );
    let selected_structural_result_local = selected_operator_structural_result_local(
        program,
        shapes,
        machine,
        state,
        statements,
        selected_operator_applications,
    );
    let selected_structural_result_symbol = selected_structural_result_local
        .as_ref()
        .map(|(_, _, symbol)| *symbol);
    let binders = machine_binders(program, machine);
    let structural_result_local = selected_structural_result_local
        .is_none()
        .then(|| checked_unit_structural_result_local(program, shapes, statements, &binders))
        .flatten();
    let structural_result_symbol = structural_result_local.as_ref().map(|(_, symbol)| *symbol);
    let carries_fused_service_parameter = program.state_parameters(state).iter().any(|parameter| {
        typed_trees::service::exact_bound_service_requirement(program, parameter.type_reference)
            .is_some()
    });
    let carries_scalar_parameter = program.state_parameters(state).iter().any(|parameter| {
        !parameter.is_self
            && !parameter.relevance.is_erased()
            && program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
    });
    let (attachment_type_identity, mut structural_parameters, scalar_parameters) =
        if machine.attached_data.is_none() {
            if carries_fused_service_parameter {
                let (structural, scalar) =
                    free_fused_service_scalar_signature(program, shapes, state, &binders)?;
                (None, structural, scalar)
            } else if (selected_scalar_result_local.is_some()
                || selected_structural_result_local.is_some())
                && !carries_scalar_parameter
                && !program.state_parameters(state).is_empty()
            {
                // Selected operators retain their affine signature contract. An
                // ordinary result local does not establish that category: its call
                // retains the declared signature, including unrestricted arrays.
                let structural =
                    free_selected_operator_structural_signature(program, shapes, state, &binders)?;
                (None, structural, Vec::new())
            } else {
                let (structural, scalar) =
                    free_structural_scalar_signature(program, shapes, state, &binders)?;
                (None, structural, scalar)
            }
        } else if carries_fused_service_parameter {
            let (attachment, structural, scalar) = fused_service_scalar_signature(
                program,
                shapes,
                machine,
                state,
                &binders,
                retain_reference_self,
            )?;
            (Some(attachment), structural, scalar)
        } else if carries_scalar_parameter {
            let (attachment, structural, scalar) = structural_scalar_signature(
                program,
                shapes,
                machine,
                state,
                &binders,
                retain_reference_self,
            )?;
            (Some(attachment), structural, scalar)
        } else {
            let (attachment, structural) = structural_signature(
                program,
                shapes,
                machine,
                state,
                &binders,
                retain_reference_self,
            )?;
            (Some(attachment), structural, Vec::new())
        };
    trace.phase("state contracts");
    if !checked_state_contracts_supported(program, machine, state, &structural_parameters) {
        return None;
    }
    trace.phase("entry claims");
    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    trace.phase("state flow");
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let source_calls = facts.flow.control.calls.span_or_empty(state_flow.calls);
    trace.phase("outer calls");
    let calls = call_occurrences::outer_calls_traced(
        program,
        facts,
        machine.symbol,
        state,
        source_calls,
        trace,
    )?;
    trace.phase("result-local family");
    let construction = build_affine_array_construction_prefix(
        program, facts, shapes, machine, state, &binders, statements,
    );
    let selected_ieee_float_fma_result_locals = selected_ieee_float_fma_result_locals(
        program,
        machine,
        state,
        statements,
        selected_ieee_float_fma_applications,
    );
    if (selected_scalar_result_local.is_some()
        || selected_structural_result_local.is_some()
        || structural_result_local.is_some())
        && selected_ieee_float_fma_result_locals.is_some()
    {
        return None;
    }
    let scalar_result_local = (selected_scalar_result_local.is_none()
        && selected_structural_result_local.is_none()
        && structural_result_local.is_none()
        && selected_ieee_float_fma_result_locals.is_none())
    .then(|| checked_unit_scalar_result_local(program, statements))
    .flatten();
    let selected_write_only_scalar_result_local = selected_scalar_result_local
        .as_ref()
        .map(|(_, result)| result);
    let write_only_scalar_result_local = scalar_result_local
        .as_ref()
        .or(selected_write_only_scalar_result_local);
    let write_only_store = build_write_only_primitive_store(
        program,
        facts,
        shapes,
        machine,
        state,
        &structural_parameters,
        &scalar_parameters,
        statements,
        scalar_result_local.as_ref(),
        selected_write_only_scalar_result_local,
    );
    let structural_scalar_field_store = write_only_store
        .is_none()
        .then(|| {
            build_structural_scalar_field_store(
                program,
                facts,
                machine,
                state,
                &structural_parameters,
                &scalar_parameters,
                statements,
                scalar_result_local.as_ref(),
                selected_write_only_scalar_result_local,
                trace,
            )
        })
        .flatten();
    // Construction remains owned by the existing prefix builders. The shared
    // statement sequence receives their exact local identities, not a synthetic
    // structural result or a second establishment operation.
    let sequence_trivial_locals = if construction.is_none() {
        let count = statements
            .iter()
            .take_while(|statement| {
                matches!(statement, StatementNode::LocalData(local)
                if program.expression_table.expression_is_valid(local.initial_value)
                    && matches!(program.expression_table.expression(local.initial_value),
                        ExpressionNode::StructLiteral(_)))
            })
            .count();
        (count != 0)
            .then(|| {
                build_unit_trivial_affine_locals(
                    program,
                    facts,
                    shapes,
                    machine,
                    state,
                    &binders,
                    &statements[..count],
                )
            })
            .flatten()
    } else {
        None
    };
    let construction_statement_count = sequence_trivial_locals.as_ref().map_or(0, Vec::len);
    trace.phase("statement sequence");
    let statement_sequence = if selected_scalar_result_local.is_none()
        && selected_structural_result_local.is_none()
        && selected_ieee_float_fma_result_locals.is_none()
        && construction.is_none()
        && write_only_store.is_none()
        && structural_scalar_field_store.is_none()
        && statement_sequence::has_statement_shape(
            program,
            facts,
            machine,
            state,
            construction_statement_count,
        ) {
        Some(statement_sequence::build(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            &structural_parameters,
            &scalar_parameters,
            &entry_claims,
            &calls,
            sequence_trivial_locals.as_deref().unwrap_or(&[]),
            construction_statement_count,
            call_frames,
            trace,
        )?)
    } else {
        None
    };
    trace.phase("scalar expression locals");
    let scalar_expression_locals = if statement_sequence.is_some() {
        Vec::new()
    } else if selected_scalar_result_local.is_some() || scalar_result_local.is_some() {
        scalar_expression_local_suffix(program, facts, state, statements)?
    } else {
        crate::execution::terminal_unit::scalar_locals::scalar_expression_local_prefix(
            program, facts, state, statements,
        )
        .unwrap_or_default()
    };
    let scalar_result_local_count = statement_sequence
        .as_ref()
        .map(|sequence| sequence.local_count)
        .unwrap_or_else(|| {
            selected_ieee_float_fma_result_locals.as_ref().map_or_else(
                || {
                    usize::from(
                        scalar_result_local.is_some()
                            || selected_scalar_result_local.is_some()
                            || selected_structural_result_local.is_some()
                            || structural_result_local.is_some(),
                    ) + scalar_expression_locals.len()
                },
                Vec::len,
            )
        });
    trace.phase("shape-family consistency");
    let has_scalar_result_local = scalar_result_local_count != 0;
    if has_scalar_result_local && statement_sequence.is_none() && construction.is_some() {
        return None;
    }
    if write_only_store.is_some()
        && has_scalar_result_local
        && (write_only_scalar_result_local.is_none() || !scalar_expression_locals.is_empty())
    {
        return None;
    }
    let borrow_alias_prefix = reborrow_restored_call_alias_prefix(
        program, facts, machine, state, statements,
    )
    .or_else(|| {
        receiver_aliases::prefix(program, facts, machine, state).map(|aliases| aliases.len())
    });
    let local_count = if has_scalar_result_local {
        scalar_result_local_count
    } else {
        construction.as_ref().map_or_else(
            || {
                borrow_alias_prefix.unwrap_or_else(|| {
                    statements
                        .iter()
                        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
                        .count()
                })
            },
            |(_, local_statement_count)| *local_statement_count,
        )
    };
    trace.phase("call statement shape");
    let call_statements = if construction.is_some() {
        &statements[statements.len()..]
    } else {
        &statements[local_count..]
    };
    if write_only_store.is_some() || structural_scalar_field_store.is_some() {
        trace.phase("call statement shape: store route statement count");
        if construction.is_some()
            || if scalar_result_local.is_some() {
                local_count != 1 || calls.len() != 1 || statements.len() != 2
            } else if selected_scalar_result_local.is_some() {
                local_count != 1 || !calls.is_empty() || statements.len() != 2
            } else {
                local_count != 0 || !calls.is_empty()
            }
        {
            return None;
        }
    } else {
        let expected_call_count = call_statements.len().checked_add(usize::from(
            scalar_result_local.is_some() || structural_result_local.is_some(),
        ))?;
        if statement_sequence.is_none() {
            trace.phase("call statement shape: call statements without a statement sequence");
            if calls.len() != expected_call_count {
                trace.phase("call statement shape: call count without a statement sequence");
                return None;
            }
            if let Some(index) =
                call_statements
                    .iter()
                    .enumerate()
                    .find_map(|(index, statement)| {
                        (!matches!(statement, StatementNode::Call(_))
                            && local_count
                                .checked_add(index)
                                .and_then(|index| {
                                    call_occurrences::unit_statement_call(
                                        program, machine, state, index,
                                    )
                                })
                                .is_none())
                        .then_some(index)
                    })
            {
                trace.statement(
                    local_count
                        .checked_add(index)
                        .and_then(|index| u32::try_from(index).ok()),
                );
                return None;
            }
        }
        // Primitive structural places belong to the checked store/call closure,
        // including stores and returned reference leaves retained by the shared
        // statement sequence. One exact
        // empty write-only sink also remains a projected forwarding target.
        let carries_primitive = structural_parameters.iter().any(|parameter| {
            shapes
                .types
                .get(&parameter.type_identity)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
                    )
                })
        });
        let exact_write_only_primitive_sink = matches!(
            structural_parameters.as_slice(),
            [parameter]
                if parameter.multiplicity == Multiplicity::Unrestricted
                    && parameter.access == CheckedStructuralAccess::WriteOnlyBorrow
                    && parameter.qualifications.is_empty()
                    && shapes.types.get(&parameter.type_identity).is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
                        )
                    })
        );
        let source_parameters = program.state_parameters(state);
        let exact_shared_primitive_observer = statements.is_empty()
            && calls.is_empty()
            && entry_claims.is_empty()
            && matches!(structural_parameters.len(), 2 | 3)
            && structural_parameters.iter().all(|parameter| {
                parameter.multiplicity == Multiplicity::Unrestricted
                    && parameter.access == CheckedStructuralAccess::SharedBorrow
                    && parameter.qualifications.is_empty()
                    && parameter.type_identity == structural_parameters[0].type_identity
            })
            && source_parameters.len() == structural_parameters.len()
            && source_parameters
                .iter()
                .all(|parameter| !parameter.is_self && !parameter.is_const);
        // Primitive carriers are also consumed by reads of their exact storage
        // and by shared onward loans, including a loan retained as a scheduled
        // computation's call argument rather than as an operation field.
        let primitive_positions = structural_parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| {
                shapes
                    .types
                    .get(&parameter.type_identity)
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
                        )
                    })
            })
            .filter_map(|(index, _)| u32::try_from(index).ok())
            .collect::<BTreeSet<_>>();
        let primitive_symbols = structural_parameters
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                u32::try_from(*index).is_ok_and(|index| primitive_positions.contains(&index))
            })
            .filter_map(|(_, parameter)| {
                source_parameters
                    .get(usize::try_from(parameter.position).ok()?)
                    .map(|source| source.symbol)
            })
            .collect::<Vec<_>>();
        trace.phase("call statement shape: primitive carrier without a store or call closure");
        if carries_primitive
            && source_calls
                .iter()
                .all(|call| facts.flow.control.is_retired(state.symbol, call))
            && !statement_sequence.as_ref().is_some_and(|sequence| {
                sequence
                    .structural_result
                    .as_ref()
                    .is_some_and(|result| !result.reference_sources.is_empty())
                    || sequence.operations.iter().any(|operation| {
                        matches!(
                            operation,
                            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                                | CheckedUnitEffectOperationPlan::EstablishReference { .. }
                        ) || operation_observes_primitive_carrier(
                            facts,
                            operation,
                            &primitive_positions,
                            &primitive_symbols,
                        )
                    })
            })
            && !exact_write_only_primitive_sink
            && !exact_shared_primitive_observer
        {
            return None;
        }
    }
    trace.phase("trivial affine locals");
    let local_rows = match (has_scalar_result_local, construction, borrow_alias_prefix) {
        (true, None, None) => sequence_trivial_locals.unwrap_or_default(),
        (false, Some((rows, _)), None) => rows,
        (false, None, Some(_)) => Vec::new(),
        (false, None, None) => build_unit_trivial_affine_locals(
            program,
            facts,
            shapes,
            machine,
            state,
            &binders,
            &statements[..local_count],
        )?,
        _ => {
            unreachable!("scalar, affine, and restored-alias local lanes were separated above")
        }
    };
    let trivial_affine_locals = local_rows
        .iter()
        .map(|(plan, _)| plan.clone())
        .collect::<Vec<_>>();
    let mut admitted_local_symbols = local_rows
        .iter()
        .map(|(_, symbol)| *symbol)
        .collect::<Vec<_>>();
    admitted_local_symbols.extend(selected_structural_result_symbol);
    admitted_local_symbols.extend(structural_result_symbol);
    if let Some(sequence) = &statement_sequence {
        admitted_local_symbols.extend(&sequence.structural_local_symbols);
    }

    let mut operations = trivial_affine_locals
        .iter()
        .map(
            |local| CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: local
                    .construction
                    .as_ref()
                    .and_then(|element| u32::try_from(element.index).ok())
                    .and_then(|index| index.checked_add(1))
                    .unwrap_or(local.declaration_ordinal),
                declaration_ordinal: local.declaration_ordinal,
                type_identity: local.type_identity.clone(),
            },
        )
        .collect::<Vec<_>>();
    if statement_sequence.is_none()
        && scalar_result_local.is_none()
        && selected_scalar_result_local.is_none()
    {
        operations.extend(
            scalar_expression_locals
                .iter()
                .cloned()
                .map(
                    |(result, value)| CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                        result,
                        value: checked_trees::CheckedCallScalarArgument::Pure(value),
                    },
                ),
        );
    }
    operations.reserve(calls.len() + 1);
    trace.phase("result ownership");
    let structural_result = statement_sequence
        .as_ref()
        .and_then(|sequence| sequence.structural_result.clone());
    let scalar_result = statement_sequence
        .as_ref()
        .and_then(|sequence| sequence.scalar_result);
    let scalar_control = statement_sequence
        .as_ref()
        .and_then(|sequence| sequence.scalar_control.clone());
    if !is_unit(program, state.return_type)
        && structural_result.is_none()
        && scalar_result.is_none()
        && scalar_control.is_none()
    {
        return None;
    }
    trace.phase("call operations");
    if let Some(sequence) = statement_sequence {
        operations.extend(sequence.operations);
    } else if let Some(store) = write_only_store {
        if let Some((application, result)) = selected_scalar_result_local {
            operations.push(build_selected_operator_scalar_call(
                program,
                facts,
                state,
                application,
                result,
            )?);
        } else if let Some(result) = scalar_result_local {
            trace.statement(Some(result.statement_index));
            let call = calls.first()?;
            if call.statement_index != usize::try_from(result.statement_index).ok()?
                || call.call_ordinal != 0
            {
                return None;
            }
            let call_operation = build_call_operation(
                program,
                facts,
                Some(scalar_callees),
                machine,
                state,
                &structural_parameters,
                &local_rows,
                &entry_claims,
                call,
                false,
                Some(ExpectedCallValueResult::Scalar(result.primitive_type)),
                &[],
            )?;
            operations.push(bind_scalar_call_result(
                facts,
                call_operation,
                result,
                false,
            )?);
        }
        operations.push(store);
    } else if let Some(store) = structural_scalar_field_store {
        if let Some((application, result)) = selected_scalar_result_local {
            operations.push(build_selected_operator_scalar_call(
                program,
                facts,
                state,
                application,
                result,
            )?);
        } else if let Some(result) = scalar_result_local {
            trace.statement(Some(result.statement_index));
            let call = calls.first()?;
            if call.statement_index != usize::try_from(result.statement_index).ok()?
                || call.call_ordinal != 0
            {
                return None;
            }
            let call_operation = build_call_operation(
                program,
                facts,
                Some(scalar_callees),
                machine,
                state,
                &structural_parameters,
                &local_rows,
                &entry_claims,
                call,
                false,
                Some(ExpectedCallValueResult::Scalar(result.primitive_type)),
                &[],
            )?;
            operations.push(bind_scalar_call_result(
                facts,
                call_operation,
                result,
                false,
            )?);
        }
        operations.push(CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(
            store,
        ));
    } else {
        let mut structural_result_bindings = Vec::new();
        let call_offset =
            if let Some((application, result)) = selected_scalar_result_local {
                let operation =
                    build_selected_operator_scalar_call(program, facts, state, application, result)
                        .or_else(|| {
                            build_selected_operator_structural_scalar_call(
                                program,
                                facts,
                                scalar_callees,
                                shapes,
                                machine,
                                state,
                                &mut structural_parameters,
                                &entry_claims,
                                application,
                                result,
                            )
                        })?;
                operations.push(operation);
                operations.extend(scalar_expression_locals.iter().cloned().map(
                    |(result, value)| CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                        result,
                        value: checked_trees::CheckedCallScalarArgument::Pure(value),
                    },
                ));
                0
            } else if let Some((application, result, _)) = selected_structural_result_local {
                operations.push(build_selected_operator_structural_call(
                    program,
                    facts,
                    shapes,
                    machine,
                    state,
                    &mut structural_parameters,
                    &entry_claims,
                    application,
                    result,
                )?);
                0
            } else if let Some(locals) = selected_ieee_float_fma_result_locals {
                for (application, result) in locals {
                    operations.push(build_selected_ieee_float_fma(
                        program,
                        facts,
                        state,
                        application,
                        result,
                    )?);
                }
                0
            } else if let Some((result, _)) = structural_result_local {
                trace.statement(Some(result.statement_index));
                let call = calls.first()?;
                if call.statement_index != usize::try_from(result.statement_index).ok()?
                    || call.call_ordinal != 0
                {
                    return None;
                }
                let operation = build_call_operation(
                    program,
                    facts,
                    Some(scalar_callees),
                    machine,
                    state,
                    &structural_parameters,
                    &local_rows,
                    &entry_claims,
                    call,
                    false,
                    Some(ExpectedCallValueResult::Structural(&result)),
                    &[],
                )?;
                let operation = bind_structural_call_result(operation, result)?;
                if let CheckedUnitEffectOperationPlan::StructuralCall { .. } = &operation {
                    for plan in &facts.flow.terminal_structural_returns.structural_types {
                        if shapes
                            .types
                            .get(&plan.identity)
                            .is_some_and(|existing| existing != plan)
                        {
                            return None;
                        }
                        shapes.types.insert(plan.identity.clone(), plan.clone());
                    }
                }
                // A boundary call's bound structural result is a live caller
                // local exactly like an ordinary structural call's: a later
                // call argument rooted at its symbol replays the retained
                // binding's identity, multiplicity, and custody events.
                match &operation {
                    CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } => {
                        structural_result_bindings.push((
                            result.clone(),
                            facts::PlaceRoot::Symbol(structural_result_symbol?),
                        ))
                    }
                    _ => {}
                }
                operations.push(operation);
                1
            } else {
                0
            };
        for (call_index, call) in calls[call_offset..].iter().enumerate() {
            let statement_index = local_count.checked_add(call_index)?;
            trace.statement(u32::try_from(statement_index).ok());
            if call.statement_index != statement_index || call.call_ordinal != 0 {
                return None;
            }
            let operation = build_call_operation(
                program,
                facts,
                Some(scalar_callees),
                machine,
                state,
                &structural_parameters,
                &local_rows,
                &entry_claims,
                call,
                false,
                None,
                &structural_result_bindings,
            )?;
            // A later call consuming an earlier call's bound result clears the
            // producer's pending return disposal through the shared custody
            // join — boundary structural results carry the same contract.
            statement_sequence::consume_results(&mut operations, &operation)?;
            operations.push(operation);
        }
    }
    trace.phase("completion");
    let transferred_local_ordinals = operations
        .iter()
        .flat_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                structural_arguments,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                structural_arguments,
                ..
            } => structural_arguments
                .iter()
                .filter_map(|argument| argument.source_local_declaration_ordinal())
                .collect::<Vec<_>>(),
            CheckedUnitEffectOperationPlan::PortWrite { .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
            | CheckedUnitEffectOperationPlan::EstablishReference { .. }
            | CheckedUnitEffectOperationPlan::ReleaseReference { .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
            | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
            | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. }
            | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
            | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
            | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
            | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
            | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
            | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
            | CheckedUnitEffectOperationPlan::Complete { .. } => Vec::new(),
        })
        .collect::<BTreeSet<_>>();
    // A cleanup-owned local still owned at return would need its exact
    // owner-attached `::drop` invoked here; the bounded lane has no per-local
    // cleanup edge, so the machine stays outside the slice rather than
    // discarding the owner silently.
    if operations.iter().any(|operation| {
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            discard_result_on_return,
            ..
        } = operation
        else {
            return false;
        };
        *discard_result_on_return
            && matches!(
                statements.get(result.statement_index as usize),
                Some(StatementNode::LocalData(local))
                    if type_graph_requires_nominal_drop(program, local.type_reference)
            )
    }) {
        return None;
    }
    operations.push(CheckedUnitEffectOperationPlan::Complete {
        statement_index: u32::try_from(statements.len()).ok()?,
        trivial_affine_local_discard_ordinals: (0..trivial_affine_locals.len())
            .rev()
            .map(|ordinal| u32::try_from(ordinal).ok())
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .filter(|ordinal| !transferred_local_ordinals.contains(ordinal))
            .collect(),
        trivial_affine_discards: return_unit_affine_discards(
            program,
            facts,
            machine.symbol,
            state.symbol,
            &structural_parameters,
            program.state_parameters(state),
            &operations,
            &admitted_local_symbols,
        )?,
    });

    trace.phase("contract plan");
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    let mut body_qualifications = facts
        .qualifications
        .for_machine(machine.symbol)
        .map(|fact| {
            fact.body_committed
                .iter()
                .copied()
                .filter(|domain| {
                    !matches!(
                        *domain,
                        language_semantics::SemanticDomainTable::WRAPPING
                            | language_semantics::SemanticDomainTable::SATURATING
                            | language_semantics::SemanticDomainTable::TRAPPING
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    body_qualifications.sort_by_key(|domain| domain.0);
    body_qualifications.dedup();

    trace.phase("provider attachment requirements");
    let provider_attachment_requirements = match attachment_type_identity.as_deref() {
        Some(attachment) => checked_provider_attachment_requirements(
            program,
            shapes,
            machine,
            state,
            attachment,
            &structural_parameters,
            source_calls,
            &operations,
        )?,
        None => Vec::new(),
    };

    trace.phase("service reach");
    let erased_scalar_parameters =
        crate::execution::terminal_unit::types::erased_scalar_parameter_plans(program, state)?;
    Some(CheckedUnitEffectMachinePlan {
        scalar_result,
        scalar_control,
        structural_result,
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameters,
        scalar_parameters,
        erased_scalar_parameters,
        provider_attachment_requirements,
        trivial_affine_locals,
        entry_claims,
        body_qualifications,
        contract_report_fingerprint: contract.report_fingerprint,
        contract_commitment: contract.commitment,
        contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
        service_reach: state_flow.service_reach,
        operations,
    })
}

/// Recognize only the erased parent alias and the one-, two-, or three-child roster
/// named by the checked
/// post-reactivation certificate. These source bindings carry borrow
/// lifetimes, not independent Terminal runtime places; every other reference
/// local continues to reject through the ordinary affine-local path.
fn reborrow_restored_call_alias_prefix(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
) -> Option<usize> {
    let StatementNode::LocalData(parent_local) = statements.first()? else {
        return None;
    };
    let child_count = facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .filter_map(|(_, certificate)| {
            (certificate.machine_symbol == machine.symbol
                && certificate.state_symbol == state.symbol)
                .then_some(())?;
            facts
                .borrow
                .reborrow_disposition_events
                .is_valid(certificate.disposition)
                .then(|| {
                    facts
                        .borrow
                        .reborrow_disposition_events
                        .get(certificate.disposition)
                        .shared_cohort
                        .len()
                        .max(1)
                })
        })
        .find(|count| matches!(count, 1..=3))?;
    let child_locals = statements
        .get(1..=child_count)?
        .iter()
        .map(|statement| match statement {
            StatementNode::LocalData(local) if !local.is_mutable => Some(local),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if parent_local.is_mutable {
        return None;
    }
    let ExpressionNode::Borrow(parent_borrow) = program
        .expression_table
        .expression(parent_local.initial_value)
    else {
        return None;
    };
    let child_borrows = child_locals
        .iter()
        .map(
            |local| match program.expression_table.expression(local.initial_value) {
                ExpressionNode::Borrow(borrow) => Some(borrow),
                _ => None,
            },
        )
        .collect::<Option<Vec<_>>>()?;
    if parent_borrow.access != language_semantics::ReferenceAccess::Mutable {
        return None;
    }

    let parent_source = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        0,
        parent_borrow.target,
    )?;
    let child_sources = child_borrows
        .iter()
        .enumerate()
        .map(|(offset, borrow)| {
            crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                offset + 1,
                borrow.target,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let candidates = facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .filter(|(_, certificate)| {
            if certificate.machine_symbol != machine.symbol
                || certificate.state_symbol != state.symbol
                || certificate.carrier_place.root_symbol != parent_local.symbol
                || !certificate.carrier_place.segments.is_empty()
                || parent_source.root
                    != facts::PlaceRoot::Symbol(certificate.restored_place.root_symbol)
                || parent_source.segments != certificate.restored_place.segments
                || child_sources.iter().any(|source| {
                    source.root != facts::PlaceRoot::Symbol(parent_local.symbol)
                        || !source.segments.is_empty()
                })
                || !facts
                    .borrow
                    .reborrow_loan_resources
                    .is_valid(certificate.child_resource)
                || !facts.flow.control.calls.is_valid(certificate.call)
            {
                return false;
            }
            let child = facts
                .borrow
                .reborrow_loan_resources
                .get(certificate.child_resource);
            let call = facts.flow.control.calls.get(certificate.call);
            let disposition = facts
                .borrow
                .reborrow_disposition_events
                .get(certificate.disposition);
            let roster = if disposition.shared_cohort.is_empty() {
                vec![certificate.child_resource]
            } else {
                disposition.shared_cohort.clone()
            };
            roster.len() == child_count
                && child_locals
                    .iter()
                    .zip(&child_borrows)
                    .all(|(local, borrow)| {
                        roster.iter().any(|resource| {
                            if !facts.borrow.reborrow_loan_resources.is_valid(*resource) {
                                return false;
                            }
                            let member = facts.borrow.reborrow_loan_resources.get(*resource);
                            member.owner_symbol == local.symbol
                                && borrow.access
                                    == match member.access {
                                        checked_trees::BorrowAccessKind::Mutable => {
                                            language_semantics::ReferenceAccess::Mutable
                                        }
                                        checked_trees::BorrowAccessKind::WriteOnly => {
                                            language_semantics::ReferenceAccess::WriteOnly
                                        }
                                        checked_trees::BorrowAccessKind::Read => {
                                            language_semantics::ReferenceAccess::Shared
                                        }
                                    }
                        })
                    })
                && roster.contains(&certificate.child_resource)
                && child_locals
                    .iter()
                    .any(|local| local.symbol == child.owner_symbol)
                && call.statement_index == child_count + usize::from(child_count > 1) + 1
                && call.call_ordinal == 0
                && call.target_symbol == certificate.target_symbol
        })
        .count();
    (candidates == 1).then_some(child_count + 1)
}

/// One operation observes a primitive structural parameter's place when a
/// retained scalar value reads its storage or a structural argument loans its
/// exact `Parameter` position onward, including inside a scheduled
/// computation's call arguments. Reads and forwarding are the same storage
/// the callee sees; nothing here constructs a copied or projected referent.
fn operation_observes_primitive_carrier(
    facts: &CheckFacts,
    operation: &CheckedUnitEffectOperationPlan,
    positions: &BTreeSet<u32>,
    symbols: &[SymbolHandle],
) -> bool {
    let scalar_argument = |argument: &checked_trees::CheckedCallScalarArgument| match argument {
        checked_trees::CheckedCallScalarArgument::Pure(expression) => {
            scalar_expression_reads_carrier(expression, symbols)
        }
        checked_trees::CheckedCallScalarArgument::Computation(root) => {
            computation_observes_primitive_carrier(
                facts,
                *root,
                positions,
                symbols,
                &mut Vec::new(),
            )
        }
    };
    let structural_argument = |argument: &checked_trees::CheckedUnitStructuralArgumentPlan| {
        argument
            .source_parameter_index()
            .is_some_and(|index| positions.contains(&index))
    };
    match operation {
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { value, .. }
        | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { value, .. } => {
            scalar_argument(value)
        }
        CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { value, .. } => {
            scalar_expression_reads_carrier(value, symbols)
        }
        CheckedUnitEffectOperationPlan::EstablishScalarArray { elements, .. } => {
            elements.iter().any(scalar_argument)
        }
        CheckedUnitEffectOperationPlan::EstablishReference { source, .. } => {
            structural_argument(source)
        }
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } => {
            structural_value_observes_primitive_carrier(
                facts,
                *value,
                positions,
                symbols,
                &mut Vec::new(),
                &mut Vec::new(),
            )
        }
        CheckedUnitEffectOperationPlan::CallUnit {
            scalar_arguments,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::ScalarCall {
            scalar_arguments,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::StructuralCall {
            scalar_arguments,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryCall {
            scalar_arguments,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            scalar_arguments,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            scalar_arguments,
            structural_arguments,
            ..
        } => {
            scalar_arguments.iter().any(&scalar_argument)
                || structural_arguments.iter().any(&structural_argument)
        }
        CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
            scalar_arguments, ..
        } => scalar_arguments
            .iter()
            .any(|expression| scalar_expression_reads_carrier(expression, symbols)),
        CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
            scalar_arguments,
            structural_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
            scalar_arguments,
            structural_arguments,
            ..
        } => {
            scalar_arguments
                .iter()
                .any(|expression| scalar_expression_reads_carrier(expression, symbols))
                || structural_arguments.iter().any(&structural_argument)
        }
        CheckedUnitEffectOperationPlan::ReleaseReference { .. }
        | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
        | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
        | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. }
        | CheckedUnitEffectOperationPlan::PortWrite { .. }
        | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
        | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
        | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
        | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
        | CheckedUnitEffectOperationPlan::Complete { .. } => false,
    }
}

/// A computation node reads a carrier's `StorageRead` or loans the exact
/// parameter place to a nested call or structural operand.
fn computation_observes_primitive_carrier(
    facts: &CheckFacts,
    handle: checked_trees::CheckedScalarComputationHandle,
    positions: &BTreeSet<u32>,
    symbols: &[SymbolHandle],
    visited: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
) -> bool {
    let computations = &facts.values.scalar_computations;
    if !computations.nodes.is_valid(handle) || visited.contains(&handle) {
        return false;
    }
    visited.push(handle);
    let computation_structural_argument =
        |argument: &checked_trees::CheckedScalarComputationStructuralArgument,
         visited: &mut Vec<checked_trees::CheckedScalarComputationHandle>| match argument {
            checked_trees::CheckedScalarComputationStructuralArgument::Place(plan) => plan
                .source_parameter_index()
                .is_some_and(|index| positions.contains(&index)),
            checked_trees::CheckedScalarComputationStructuralArgument::Array {
                elements, ..
            } => computations
                .operands
                .span_or_empty(*elements)
                .iter()
                .any(|element| {
                    computation_observes_primitive_carrier(
                        facts, *element, positions, symbols, visited,
                    )
                }),
            checked_trees::CheckedScalarComputationStructuralArgument::Case(_) => false,
        };
    match &computations.nodes.get(handle).kind {
        checked_trees::CheckedScalarComputationKind::Value(expression) => {
            scalar_expression_reads_carrier(expression, symbols)
        }
        checked_trees::CheckedScalarComputationKind::Call {
            arguments,
            structural_arguments,
            ..
        } => {
            computations
                .operands
                .span_or_empty(*arguments)
                .iter()
                .any(|operand| {
                    computation_observes_primitive_carrier(
                        facts, *operand, positions, symbols, visited,
                    )
                })
                || computations
                    .structural_arguments
                    .span_or_empty(*structural_arguments)
                    .iter()
                    .any(|argument| computation_structural_argument(argument, visited))
        }
        checked_trees::CheckedScalarComputationKind::Select {
            condition,
            when_true,
            when_false,
            ..
        } => [*condition, *when_true, *when_false].iter().any(|operand| {
            computation_observes_primitive_carrier(facts, *operand, positions, symbols, visited)
        }),
        checked_trees::CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
            computation_observes_primitive_carrier(facts, *subject, positions, symbols, visited)
                || computations
                    .dispatch_arms
                    .span_or_empty(*arms)
                    .iter()
                    .any(|arm| {
                        computation_observes_primitive_carrier(
                            facts, arm.value, positions, symbols, visited,
                        ) || match arm.pattern {
                            checked_trees::CheckedScalarDispatchPattern::Value(pattern) => {
                                computation_observes_primitive_carrier(
                                    facts, pattern, positions, symbols, visited,
                                )
                            }
                            checked_trees::CheckedScalarDispatchPattern::Wildcard => false,
                        }
                    })
        }
        checked_trees::CheckedScalarComputationKind::Apply {
            expression,
            operands,
            ..
        } => {
            scalar_expression_reads_carrier(expression, symbols)
                || computations
                    .operands
                    .span_or_empty(*operands)
                    .iter()
                    .any(|operand| {
                        computation_observes_primitive_carrier(
                            facts, *operand, positions, symbols, visited,
                        )
                    })
        }
        checked_trees::CheckedScalarComputationKind::Qualification { operand, .. }
        | checked_trees::CheckedScalarComputationKind::BooleanToInteger { operand, .. } => {
            computation_observes_primitive_carrier(facts, *operand, positions, symbols, visited)
        }
        checked_trees::CheckedScalarComputationKind::StructuralField { subject, .. } => subject
            .source_parameter_index()
            .is_some_and(|index| positions.contains(&index)),
        checked_trees::CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
            [*left, *right].iter().any(|operand| {
                computation_observes_primitive_carrier(facts, *operand, positions, symbols, visited)
            })
        }
        checked_trees::CheckedScalarComputationKind::CaseMembership { subject, .. } => {
            computation_structural_argument(subject, visited)
        }
    }
}

/// A structural value observes a carrier when its `Place`/`Reference` source
/// loans the parameter position, or a nested projection, record field,
/// dispatch subject, or scheduled call operand does.
fn structural_value_observes_primitive_carrier(
    facts: &CheckFacts,
    handle: checked_trees::CheckedStructuralValueHandle,
    positions: &BTreeSet<u32>,
    symbols: &[SymbolHandle],
    visited: &mut Vec<checked_trees::CheckedStructuralValueHandle>,
    visited_computations: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
) -> bool {
    let plans = &facts.values.structural_values;
    if !plans.nodes.is_valid(handle) || visited.contains(&handle) {
        return false;
    }
    visited.push(handle);
    match &plans.nodes.get(handle).kind {
        checked_trees::CheckedStructuralValueKind::Reference { source }
        | checked_trees::CheckedStructuralValueKind::Place(source) => source
            .source_parameter_index()
            .is_some_and(|index| positions.contains(&index)),
        checked_trees::CheckedStructuralValueKind::Projection { source, .. } => {
            structural_value_observes_primitive_carrier(
                facts,
                *source,
                positions,
                symbols,
                visited,
                visited_computations,
            )
        }
        checked_trees::CheckedStructuralValueKind::Record { fields, .. } => plans
            .record_fields
            .span_or_empty(*fields)
            .iter()
            .any(|field| match field.value {
                checked_trees::CheckedStructuralRecordFieldValue::Scalar(operand) => {
                    computation_observes_primitive_carrier(
                        facts,
                        operand,
                        positions,
                        symbols,
                        visited_computations,
                    )
                }
                checked_trees::CheckedStructuralRecordFieldValue::Structural(child) => {
                    structural_value_observes_primitive_carrier(
                        facts,
                        child,
                        positions,
                        symbols,
                        visited,
                        visited_computations,
                    )
                }
            }),
        checked_trees::CheckedStructuralValueKind::Dispatch { subject, arms } => {
            computation_observes_primitive_carrier(
                facts,
                *subject,
                positions,
                symbols,
                visited_computations,
            ) || plans.dispatch_arms.span_or_empty(*arms).iter().any(|arm| {
                structural_value_observes_primitive_carrier(
                    facts,
                    arm.value,
                    positions,
                    symbols,
                    visited,
                    visited_computations,
                ) || match arm.pattern {
                    checked_trees::CheckedScalarDispatchPattern::Value(pattern) => {
                        computation_observes_primitive_carrier(
                            facts,
                            pattern,
                            positions,
                            symbols,
                            visited_computations,
                        )
                    }
                    checked_trees::CheckedScalarDispatchPattern::Wildcard => false,
                }
            })
        }
        checked_trees::CheckedStructuralValueKind::Case(_)
        | checked_trees::CheckedStructuralValueKind::Call { .. } => false,
    }
}

/// A scalar expression reads a primitive carrier only through `StorageRead`
/// of its authored parameter symbol; every other leaf is an independent
/// position or constant.
fn scalar_expression_reads_carrier(
    expression: &checked_trees::CheckedScalarExpression,
    symbols: &[SymbolHandle],
) -> bool {
    match expression {
        checked_trees::CheckedScalarExpression::StorageRead { symbol, .. } => {
            symbols.contains(symbol)
        }
        checked_trees::CheckedScalarExpression::Boolean(operand) => {
            boolean_expression_reads_carrier(operand, symbols)
        }
        checked_trees::CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            scalar_expression_reads_carrier(left, symbols)
                || scalar_expression_reads_carrier(right, symbols)
        }
        checked_trees::CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | checked_trees::CheckedScalarExpression::IntegerWiden { operand, .. }
        | checked_trees::CheckedScalarExpression::IntegerExactCast { operand, .. }
        | checked_trees::CheckedScalarExpression::IntegerWrappingCast { operand, .. }
        | checked_trees::CheckedScalarExpression::IntegerTrappingCast { operand, .. }
        | checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
            index: operand,
            ..
        } => scalar_expression_reads_carrier(operand, symbols),
        checked_trees::CheckedScalarExpression::StructuralParameterByteLength { .. }
        | checked_trees::CheckedScalarExpression::Parameter { .. }
        | checked_trees::CheckedScalarExpression::ErasedParameter { .. }
        | checked_trees::CheckedScalarExpression::Local { .. }
        | checked_trees::CheckedScalarExpression::StructuralParameterField { .. }
        | checked_trees::CheckedScalarExpression::IntegerLiteral { .. }
        | checked_trees::CheckedScalarExpression::IeeeFloatLiteral { .. } => false,
    }
}

fn boolean_expression_reads_carrier(
    expression: &checked_trees::CheckedBooleanExpression,
    symbols: &[SymbolHandle],
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { symbol, .. } => {
            symbols.contains(symbol)
        }
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            boolean_expression_reads_carrier(operand, symbols)
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right }
        | checked_trees::CheckedBooleanExpression::And { left, right }
        | checked_trees::CheckedBooleanExpression::Or { left, right } => {
            boolean_expression_reads_carrier(left, symbols)
                || boolean_expression_reads_carrier(right, symbols)
        }
        checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            scalar_expression_reads_carrier(left, symbols)
                || scalar_expression_reads_carrier(right, symbols)
        }
        checked_trees::CheckedBooleanExpression::Constant(_)
        | checked_trees::CheckedBooleanExpression::Parameter { .. }
        | checked_trees::CheckedBooleanExpression::ErasedParameter { .. }
        | checked_trees::CheckedBooleanExpression::Local { .. }
        | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
    }
}
