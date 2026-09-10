//! Exact source correspondence and ordered operands for indexed byte replacement.

use super::*;
use checked_trees::CheckedStructuralByteSequenceFieldByteStorePlan;
use checked_trees::expression::ExpressionNode;

pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
    store: &CheckedStructuralByteSequenceFieldByteStorePlan,
) -> Result<(), LoweringError> {
    let (owner, state) = crate::scalar_source_custody::authored_state(checked, state_symbol)?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(assignment.target)
    else {
        return unsupported("byte replacement lost its authored index");
    };
    if owner.symbol != machine
        || !validation::place_has_builtin_coordinates(
            &checked.typed,
            owner,
            Some(state),
            assignment.target,
        )
        || matches!(
            checked.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        )
    {
        return unsupported("byte replacement requires exact builtin indexed custody");
    }
    if checked.facts.operators.uses.iter().any(|(_, selected)| {
        selected.expression == assignment.target
            && (selected.spelling != language_core::OperatorSpelling::Index
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
    }) {
        return unsupported("byte replacement changed its selected index meaning");
    }
    let source = crate::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        indexed.collection,
    )?;
    let parameter = checked
        .state_parameters(state)
        .get(store.destination_parameter_position as usize)
        .ok_or(LoweringError::Unsupported(
            "byte replacement has no authored parameter",
        ))?;
    let mut path = store.carrier_path.clone();
    path.push(CheckedUnitStructuralPathSegment::Field(
        store.field_identity.clone(),
    ));
    if source.root != parameter.symbol || source.path != path {
        return unsupported("byte replacement differs from its authored field");
    }
    for (role, authored, retained, primitive) in [
        (
            CheckedScalarExpressionRole::AssignmentIndex,
            indexed.index,
            &store.index,
            PrimitiveType::U64,
        ),
        (
            CheckedScalarExpressionRole::AssignmentValue,
            assignment.value,
            &store.value,
            PrimitiveType::U8,
        ),
    ] {
        let (binding, expression) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state_symbol, store.statement_index, role)
            .ok_or(LoweringError::Unsupported(
                "byte replacement lost a scalar source binding",
            ))?;
        if binding.expression != authored || expression != retained {
            return unsupported("byte replacement substituted its evaluated scalar operand");
        }
        crate::scalar_source_custody::validate_pure(
            checked,
            binding,
            terminal_scalar_type(primitive)?,
        )?;
    }
    Ok(())
}

pub(crate) fn emit(
    store: &CheckedStructuralByteSequenceFieldByteStorePlan,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    index: &LoweredDirectExpression,
    value: &LoweredDirectExpression,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == store.destination_parameter_position)
        .ok_or(LoweringError::Unsupported(
            "byte replacement destination is absent",
        ))?;
    let (path, field) = crate::structural_scalar_store::lower_structural_field_place(
        store.statement_index,
        store.statement_index,
        store.destination_parameter_position,
        &store.carrier_path,
        &store.field_identity,
        parameter,
        structural_types,
        crate::structural_scalar_store::StoreAccessPolicy::Exclusive,
    )?;
    if !matches!(
        field.field_type,
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
    ) || index.scalar_type() != terminal_scalar_type(PrimitiveType::U64)?
        || value.scalar_type() != terminal_scalar_type(PrimitiveType::U8)?
        || direct_expression_contains_short_circuit(index)
        || direct_expression_contains_short_circuit(value)
    {
        return unsupported(
            "byte replacement requires bounded bytes and exact branch-free operands",
        );
    }
    let types = values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    validate_direct_parameter_types(index, &types)?;
    validate_direct_parameter_types(value, &types)?;
    let index = emit_direct_expression(index, values, next_value, operations);
    let value = emit_direct_expression(value, values, next_value, operations);
    let length = value_id(allocate_dense(next_value)?);
    let id = operations.allocate();
    operations.push(Operation {
        id,
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: length,
            scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
        }),
        kind: OperationKind::StructuralByteSequenceFieldLength {
            source: parameter.place,
            path: path.clone(),
            field: field.id,
        },
    });
    Ok(OperationKind::StructuralByteSequenceFieldByteStore {
        destination: parameter.place,
        path,
        field: field.id,
        index,
        value,
        length,
        obligation: obligation_id(allocate_dense(next_obligation)?),
    })
}
