//! Source-bound writes through exact whole mutable byte views.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, LoweringError, Operation, OperationKind,
    OperationResult, PrimitiveType, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    ValueDeclaration, allocate_dense, direct_expression_contains_short_circuit,
    emit_direct_expression, obligation_id, terminal_scalar_type, unsupported,
    validate_direct_parameter_types, value_id,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use checked_trees::{CheckedByteSequenceWritePlan, expression::ExpressionNode};

pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
    write: &CheckedByteSequenceWritePlan,
) -> Result<(), LoweringError> {
    let (owner, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(assignment.target)
    else {
        return unsupported("byte-view write lost its authored index");
    };
    let parameter = checked
        .state_parameters(state)
        .get(write.destination_parameter_position as usize)
        .ok_or(LoweringError::Unsupported(
            "byte-view write has no authored parameter",
        ))?;
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        None,
        indexed.collection,
    )?;
    if owner.symbol != machine
        || source.root != parameter.symbol
        || !source.path.is_empty()
        || !validation::place_has_builtin_coordinates(
            &checked.typed,
            owner,
            Some(state),
            assignment.target,
        )
        || !matches!(
            checked
                .type_reference_table
                .type_reference(parameter.type_reference),
            checked_trees::types::TypeReferenceNode::Reference {
                access: language_core::ReferenceAccess::Mutable,
                ..
            }
        )
    {
        return unsupported("byte-view write changed its authored mutable destination");
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
        return unsupported("byte-view write changed its selected index meaning");
    }
    let (binding, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(
            state_symbol,
            write.statement_index,
            CheckedScalarExpressionRole::AssignmentIndex,
        )
        .ok_or(LoweringError::Unsupported(
            "byte-view write lost its operand source",
        ))?;
    if binding.expression != indexed.index || expression != &write.index {
        return unsupported("byte-view write substituted an authored operand");
    }
    crate::expression_preparation::source_custody::validate_pure(
        checked,
        binding,
        terminal_scalar_type(PrimitiveType::U64)?,
    )?;
    let value_expressions = checked
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .filter(|expression| {
            expression.state == state_symbol
                && expression.statement_ordinal == write.statement_index
                && expression.role == CheckedScalarExpressionRole::AssignmentValue
        })
        .collect::<Vec<_>>();
    match &write.value {
        // A store reading its own statement's call result has no authored
        // scalar expression to select: the right-hand side is the call. The
        // producing call is rejoined where the operation order and the value
        // are lowered; a selected expression here would mean the checked
        // stage chose a different source.
        checked_trees::CheckedByteSequenceStoreValue::ScalarResult { .. } => {
            if !value_expressions.is_empty() {
                return unsupported("byte-view write replaced a selected RHS with a result");
            }
            if !matches!(
                checked.expression_table.expression(assignment.value),
                ExpressionNode::Call(_)
            ) {
                return unsupported("byte-view write call result has no authored call");
            }
        }
        checked_trees::CheckedByteSequenceStoreValue::Pure(retained) => {
            let (binding, expression) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(
                    state_symbol,
                    write.statement_index,
                    CheckedScalarExpressionRole::AssignmentValue,
                )
                .ok_or(LoweringError::Unsupported(
                    "byte-view write lost its operand source",
                ))?;
            if binding.expression != assignment.value || expression != retained {
                return unsupported("byte-view write substituted an authored operand");
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
    write: &CheckedByteSequenceWritePlan,
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
        .find(|parameter| parameter.position == write.destination_parameter_position)
        .ok_or(LoweringError::Unsupported(
            "byte-view write destination is absent",
        ))?;
    if parameter.access != StructuralAccess::MutableBorrow
        || parameter.multiplicity != StructuralMultiplicity::Unrestricted
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || !structural_types.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView
                    )
                )
        })
        || index.scalar_type() != terminal_scalar_type(PrimitiveType::U64)?
        || value.scalar_type() != terminal_scalar_type(PrimitiveType::U8)?
        || direct_expression_contains_short_circuit(index)
        || direct_expression_contains_short_circuit(value)
    {
        return unsupported(
            "byte-view write requires exact mutable byte custody and scalar operands",
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
        static_reach_binding: None,
        id,
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: length,
            scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
        }),
        kind: OperationKind::ByteSequenceLength {
            source: parameter.place,
        },
    });
    Ok(OperationKind::ByteSequenceWrite {
        destination: parameter.place,
        index,
        value,
        length,
        obligation: obligation_id(allocate_dense(next_obligation)?),
    })
}
