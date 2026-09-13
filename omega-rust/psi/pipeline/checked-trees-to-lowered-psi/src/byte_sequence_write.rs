//! Source-bound writes through exact whole mutable byte views.
use super::*;
use checked_trees::{CheckedByteSequenceWritePlan, expression::ExpressionNode};

pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
    write: &CheckedByteSequenceWritePlan,
) -> Result<(), LoweringError> {
    let (owner, state) = crate::scalar_source_custody::authored_state(checked, state_symbol)?;
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
    let source = crate::call_source_custody::projected_receivers::store_destination(
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
    for (role, authored, retained, primitive) in [
        (
            CheckedScalarExpressionRole::AssignmentIndex,
            indexed.index,
            &write.index,
            PrimitiveType::U64,
        ),
        (
            CheckedScalarExpressionRole::AssignmentValue,
            assignment.value,
            &write.value,
            PrimitiveType::U8,
        ),
    ] {
        let (binding, expression) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state_symbol, write.statement_index, role)
            .ok_or(LoweringError::Unsupported(
                "byte-view write lost its operand source",
            ))?;
        if binding.expression != authored || expression != retained {
            return unsupported("byte-view write substituted an authored operand");
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
