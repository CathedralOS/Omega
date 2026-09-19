//! Exact source correspondence and ordered operands for indexed byte replacement.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitStructuralPathSegment, LoweringError,
    Operation, OperationKind, OperationResult, PrimitiveType, StructuralFieldType,
    StructuralParameterDeclaration, StructuralTypeDeclaration, ValueDeclaration, allocate_dense,
    direct_expression_contains_short_circuit, emit_direct_expression, obligation_id,
    terminal_scalar_type, unsupported, validate_direct_parameter_types, value_id,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use checked_trees::CheckedStructuralByteSequenceFieldByteStorePlan;
use checked_trees::expression::ExpressionNode;

pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
    store: &CheckedStructuralByteSequenceFieldByteStorePlan,
) -> Result<(), LoweringError> {
    let (owner, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
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
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        None,
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
    let (binding, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(
            state_symbol,
            store.statement_index,
            CheckedScalarExpressionRole::AssignmentIndex,
        )
        .ok_or(LoweringError::Unsupported(
            "byte replacement lost a scalar source binding",
        ))?;
    if binding.expression != indexed.index || expression != &store.index {
        return unsupported("byte replacement substituted its evaluated scalar operand");
    }
    crate::expression_preparation::source_custody::validate_pure(
        checked,
        binding,
        terminal_scalar_type(
            crate::expression_preparation::source_custody::locate(
                checked,
                state_symbol,
                store.statement_index,
                CheckedScalarExpressionRole::AssignmentIndex,
            )?
            .primitive_type,
        )?,
    )?;
    let value_expressions = checked
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .filter(|expression| {
            expression.state == state_symbol
                && expression.statement_ordinal == store.statement_index
                && expression.role == CheckedScalarExpressionRole::AssignmentValue
        })
        .collect::<Vec<_>>();
    match &store.value {
        // A store reading its own statement's call result has no authored
        // scalar expression to select: the right-hand side is the call. The
        // producing call is rejoined where the operation order and the value
        // are lowered; a selected expression here would mean the checked
        // stage chose a different source.
        checked_trees::CheckedByteSequenceStoreValue::ScalarResult { .. } => {
            if !value_expressions.is_empty() {
                return unsupported("byte replacement replaced a selected RHS with a result");
            }
            if !matches!(
                checked.expression_table.expression(assignment.value),
                ExpressionNode::Call(_)
            ) {
                return unsupported("byte replacement call result has no authored call");
            }
        }
        checked_trees::CheckedByteSequenceStoreValue::Pure(retained) => {
            let (binding, expression) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(
                    state_symbol,
                    store.statement_index,
                    CheckedScalarExpressionRole::AssignmentValue,
                )
                .ok_or(LoweringError::Unsupported(
                    "byte replacement lost a scalar source binding",
                ))?;
            if binding.expression != assignment.value || expression != retained {
                return unsupported("byte replacement substituted its evaluated scalar operand");
            }
            crate::expression_preparation::source_custody::validate_pure(
                checked,
                binding,
                terminal_scalar_type(PrimitiveType::U8)?,
            )?;
        }
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
    let (path, field) = crate::emission::structural_scalar_store::lower_structural_field_place(
        store.statement_index,
        store.statement_index,
        store.destination_parameter_position,
        &store.carrier_path,
        &store.field_identity,
        parameter,
        structural_types,
        crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
    )?;
    if !matches!(
        field.field_type,
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
    ) || !matches!(
        index.scalar_type(),
        semantic_vocabulary::ScalarType::Integer(_)
    ) || value.scalar_type() != terminal_scalar_type(PrimitiveType::U8)?
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
    let index = super::emit_byte_index(index, values, next_value, next_obligation, operations)?;
    let value = emit_direct_expression(value, values, next_value, operations);
    let length = value_id(allocate_dense(next_value)?);
    let id = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
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
