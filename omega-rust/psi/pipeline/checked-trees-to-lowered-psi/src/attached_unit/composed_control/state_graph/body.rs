//! Rejoin the complete authored body before emitting its ordered effects.

use super::*;
use checked_trees::statement::StatementNode;

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<usize, LoweringError> {
    let statements = checked.statement_table.statements(source.statement_nodes);
    let end = statements
        .iter()
        .position(|statement| matches!(statement, StatementNode::Transition(_)))
        .unwrap_or_else(|| {
            if matches!(
                state.terminator,
                CheckedComposedUnitControlTerminatorPlan::ReturnCase { .. }
                    | CheckedComposedUnitControlTerminatorPlan::ReturnStructural { .. }
            ) {
                statements.len().saturating_sub(1)
            } else {
                statements.len()
            }
        });
    let prefix = state.bindings.len();
    let marker_count = super::cases::validate_markers(checked, machine, source, state, end)?;
    let tail_value = usize::from(matches!(state.terminator, CheckedComposedUnitControlTerminatorPlan::ReturnStructural { .. })
        && state.operations.last().is_some_and(|operation| matches!(operation, CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } | CheckedUnitEffectOperationPlan::StructuralCall { result, .. } if result.statement_index as usize == statements.len().saturating_sub(1))));
    if prefix > end || state.operations.len() + marker_count != end - prefix + tail_value {
        return unsupported("Unit graph dropped or added a body effect");
    }
    let mut next_structural_binding = 0_u32;
    // Mutable storage writes occupy source statements, not immutable result
    // ordinals. Continue the same namespace used by prefix scalar replay.
    let mut next_scalar_binding = u32::try_from(
        state
            .bindings
            .iter()
            .filter(|binding| {
                binding.destination == checked_trees::CheckedScalarBindingDestination::Immutable
            })
            .count(),
    )
    .map_err(|_| LoweringError::Unsupported("Unit graph scalar binding count overflow"))?;
    for (ordinal, operation) in state.operations.iter().enumerate() {
        let ordinal = prefix + ordinal;
        match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
            | CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } => {
                if result.binding_ordinal != next_structural_binding {
                    return unsupported("Unit graph structural binding namespace drifted");
                }
                next_structural_binding =
                    next_structural_binding
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph structural binding count overflow",
                        ))?;
            }
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. } => {
                if result.binding_ordinal != next_scalar_binding {
                    return unsupported("Unit graph scalar binding namespace drifted");
                }
                next_scalar_binding =
                    next_scalar_binding
                        .checked_add(1)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph scalar binding count overflow",
                        ))?;
            }
            _ => {}
        }
        match (operation, &statements[ordinal]) {
            (
                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    discard_result_on_return: false,
                    ..
                },
                StatementNode::LocalData(_) | StatementNode::Expression(_),
            ) if result.statement_index as usize == ordinal => {
                crate::attached_unit::structural_values::source_custody::validate(
                    checked,
                    machine,
                    state.state,
                    operation,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value },
                StatementNode::LocalData(local),
            ) if !local.is_mutable
                && result.statement_index as usize == ordinal
                && checked.primitive_type_reference(local.type_reference)
                    == Some(result.primitive_type) =>
            {
                let role = CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: result.binding_ordinal,
                };
                match value {
                    checked_trees::CheckedCallScalarArgument::Pure(value) => {
                        let (binding, retained) = checked
                            .facts
                            .values
                            .scalar_expressions
                            .bound_expression_at(state.state, result.statement_index, role)
                            .ok_or(LoweringError::Unsupported(
                                "Unit graph scalar local has no source binding",
                            ))?;
                        if retained != value {
                            return unsupported("Unit graph scalar local value changed");
                        }
                        crate::scalar_source_custody::validate_pure(
                            checked,
                            binding,
                            terminal_scalar_type(result.primitive_type)?,
                        )?;
                    }
                    checked_trees::CheckedCallScalarArgument::Computation(handle) => {
                        let mut roots = checked
                            .facts
                            .values
                            .scalar_computations
                            .roots
                            .iter()
                            .map(|(_, root)| root)
                            .filter(|root| {
                                root.state == state.state
                                    && root.statement_ordinal == result.statement_index
                                    && root.role == role
                            });
                        let root = roots.next().ok_or(LoweringError::Unsupported(
                            "Unit graph scalar local has no computation root",
                        ))?;
                        if roots.next().is_some() || root.machine != machine || root.root != *handle
                        {
                            return unsupported("Unit graph scalar local computation changed");
                        }
                        crate::scalar_source_custody::validate_computation_calls(
                            checked,
                            machine,
                            state.state,
                            result.statement_index,
                            *handle,
                            local.initial_value,
                        )?;
                    }
                }
            }
            (
                CheckedUnitEffectOperationPlan::StructuralCall {
                    coordinate,
                    result,
                    discard_result_on_return,
                    ..
                },
                StatementNode::LocalData(_) | StatementNode::Expression(_),
            ) if coordinate.statement_index as usize == ordinal
                && coordinate.call_ordinal == 0
                && !discard_result_on_return
                && result.statement_index as usize == ordinal =>
            {
                crate::call_source_custody::validate_operation(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    coordinate,
                    result,
                    completion_receipts,
                    discard_result_on_return,
                    ..
                },
                StatementNode::LocalData(_),
            ) if completion_receipts.is_empty()
                && coordinate.statement_index as usize == ordinal
                && coordinate.call_ordinal == 0
                && !discard_result_on_return
                && result.statement_index as usize == ordinal =>
            {
                crate::call_source_custody::validate_operation(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::ByteSequenceWrite(write),
                StatementNode::Assignment(assignment),
            ) => {
                if write.statement_index as usize != ordinal {
                    return unsupported("Unit graph reordered a byte-view write");
                }
                crate::byte_sequence_write::validate_assignment(
                    checked,
                    machine,
                    state.state,
                    assignment,
                    write,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store),
                StatementNode::Assignment(assignment),
            ) => {
                if store.statement_index as usize != ordinal {
                    return unsupported("Unit graph reordered an indexed byte store");
                }
                crate::structural_byte_sequence_index_store::validate_assignment(
                    checked,
                    machine,
                    state.state,
                    assignment,
                    store,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store),
                StatementNode::Assignment(assignment),
            ) => {
                if store.statement_index as usize != ordinal {
                    return unsupported("Unit graph reordered a byte-field store");
                }
                crate::structural_byte_sequence_store::validate_assignment(
                    checked,
                    machine,
                    state.state,
                    assignment,
                    store,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
                StatementNode::Assignment(assignment),
            ) => {
                if store.statement_index as usize != ordinal {
                    return unsupported("Unit graph reordered a field store");
                }
                crate::structural_scalar_store_source::validate_assignment(
                    checked,
                    machine,
                    state.state,
                    store.statement_index,
                    assignment,
                    store,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    coordinate,
                    completion_receipts,
                    ..
                },
                StatementNode::Call(_) | StatementNode::Expression(_),
            ) if completion_receipts.is_empty()
                && coordinate.statement_index as usize == ordinal
                && coordinate.call_ordinal == 0 =>
            {
                crate::call_source_custody::validate_operation(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                )?;
            }
            (
                CheckedUnitEffectOperationPlan::CallUnit {
                    coordinate,
                    claim_transfers,
                    ..
                },
                StatementNode::Call(_) | StatementNode::Expression(_),
            ) if claim_transfers.is_empty()
                && coordinate.statement_index as usize == ordinal
                && coordinate.call_ordinal == 0 =>
            {
                crate::call_source_custody::validate_operation(
                    checked,
                    machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                )?;
            }
            _ => return unsupported("Unit graph reordered a source effect"),
        }
    }
    Ok(end)
}

pub(in crate::attached_unit::composed_control) fn emit_store(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &CheckedComposedUnitControlStatePlan,
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &mut crate::attached_unit::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    let destination = parameters
        .iter()
        .find(|parameter| Some(parameter.position) == store.destination.parameter_position())
        .ok_or(LoweringError::Unsupported(
            "Unit graph store destination is absent",
        ))?;
    let lowered = crate::structural_scalar_store::lower_structural_scalar_store_place(
        store,
        store.statement_index,
        destination,
        &catalogs.structural_types,
        crate::structural_scalar_store::StoreAccessPolicy::Exclusive,
    )?;
    let mut calls = catalogs.scalar_calls.emission_context();
    let value = evaluation.field_assignment_value(
        checked,
        machine,
        state.state,
        store,
        values,
        next_value,
        next_block,
        next_edge,
        operations,
        &mut calls,
    )?;
    catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
    if value.scalar_type != lowered.scalar_type {
        return unsupported("Unit graph store RHS differs from its field type");
    }
    let id = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Unit,
        kind: OperationKind::StructuralScalarFieldStore {
            destination: destination.place,
            path: lowered.path,
            field: lowered.field,
            value: value.id,
        },
    });
    Ok(())
}
