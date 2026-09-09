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
            ) {
                statements.len().saturating_sub(1)
            } else {
                statements.len()
            }
        });
    let prefix = state.bindings.len();
    let marker_count = super::cases::validate_markers(checked, source, state, end)?;
    if prefix > end || state.operations.len() + marker_count != end - prefix {
        return unsupported("Unit graph dropped or added a body effect");
    }
    for (ordinal, operation) in state.operations.iter().enumerate() {
        let ordinal = prefix + ordinal;
        match (operation, &statements[ordinal]) {
            (
                CheckedUnitEffectOperationPlan::StructuralCall {
                    coordinate,
                    result,
                    discard_result_on_return,
                    ..
                },
                StatementNode::LocalData(_),
            ) if coordinate.statement_index as usize == ordinal
                && coordinate.call_ordinal == 0
                && !discard_result_on_return
                && matches!(&state.terminator, CheckedComposedUnitControlTerminatorPlan::ClosedSum { result: selected, .. } if selected == result) =>
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
                && matches!(&state.terminator, CheckedComposedUnitControlTerminatorPlan::ClosedSum { result: selected, .. } if selected == result) =>
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
        .find(|parameter| parameter.position == store.destination_parameter_position)
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
