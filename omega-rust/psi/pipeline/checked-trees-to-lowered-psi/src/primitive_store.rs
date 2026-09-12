//! Primitive-reference store emission shared by Unit and scalar-result bodies.

use super::*;

pub(super) fn validate_assignment(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    destination: &CheckedUnitStructuralParameterPlan,
    value: &checked_trees::CheckedCallScalarArgument,
) -> Result<(), LoweringError> {
    let (_, state) = crate::scalar_source_custody::authored_state(checked, state_symbol)?;
    let parameter = checked
        .state_parameters(state)
        .get(destination.position as usize)
        .ok_or(LoweringError::Unsupported(
            "primitive store lost its authored destination",
        ))?;
    validate_symbol_assignment(
        checked,
        state_symbol,
        statement_index,
        parameter.symbol,
        value,
    )
}

pub(super) fn validate_symbol_assignment(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    destination: symbols::SymbolHandle,
    value: &checked_trees::CheckedCallScalarArgument,
) -> Result<(), LoweringError> {
    use checked_trees::{expression::ExpressionNode, statement::StatementNode};
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, state_symbol)?;
    let Some(StatementNode::Assignment(assignment)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index as usize)
    else {
        return unsupported("primitive store has no authored assignment");
    };
    let ExpressionNode::Name(target) = checked.expression_table.expression(assignment.target)
    else {
        return unsupported("primitive store destination is not a direct parameter");
    };
    if !destination.is_valid()
        || target.symbol != destination
        || target.head_symbol != destination
        || checked
            .expression_table
            .name_path_members(target.members)
            .len()
            != 1
    {
        return unsupported("primitive store destination differs from its authored parameter");
    }
    let role = CheckedScalarExpressionRole::AssignmentValue;
    let computations = &checked.facts.values.scalar_computations;
    let mut roots = computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.state == state_symbol
                && root.statement_ordinal == statement_index
                && root.role == role
        });
    let root = roots.next();
    if roots.next().is_some() {
        return unsupported("primitive store has duplicate RHS computation roots");
    }
    let value = match value {
        checked_trees::CheckedCallScalarArgument::Computation(handle) => {
            let root = root.ok_or(LoweringError::Unsupported(
                "primitive store lost its RHS computation root",
            ))?;
            if root.machine != machine.symbol
                || root.root != *handle
                || !computations.nodes.is_valid(*handle)
            {
                return unsupported("primitive store RHS computation has different custody");
            }
            let source =
                crate::scalar_source_custody::locate(checked, state_symbol, statement_index, role)?;
            let node = computations.nodes.get(*handle);
            if source.machine != machine.symbol
                || source.destination != destination
                || source.expression != assignment.value
                || node.authored_root != assignment.value
                || node.primitive_type != source.primitive_type
                || checked
                    .facts
                    .values
                    .scalar_expressions
                    .expressions
                    .iter()
                    .any(|expression| {
                        expression.state == state_symbol
                            && expression.statement_ordinal == statement_index
                            && expression.role == role
                    })
                || checked
                    .facts
                    .values
                    .scalar_expressions
                    .source_bindings
                    .iter()
                    .any(|(_, binding)| {
                        binding.state == state_symbol
                            && binding.statement_ordinal == statement_index
                            && binding.role == role
                    })
            {
                return unsupported("primitive store computation differs from its authored RHS");
            }
            crate::scalar_source_custody::validate_computation_calls(
                checked,
                machine.symbol,
                state_symbol,
                statement_index,
                *handle,
                assignment.value,
            )?;
            return crate::scalar_source_custody::value_correspondence::validate(
                checked,
                state_symbol,
                statement_index,
                assignment.value,
                source.primitive_type,
                value,
            );
        }
        checked_trees::CheckedCallScalarArgument::Pure(value) => {
            if root.is_some() {
                return unsupported(
                    "primitive store replaced its RHS computation with a pure value",
                );
            }
            value
        }
    };
    let (binding, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(
            state_symbol,
            statement_index,
            CheckedScalarExpressionRole::AssignmentValue,
        )
        .ok_or(LoweringError::Unsupported(
            "primitive store lost its unique RHS binding",
        ))?;
    if binding.expression != assignment.value
        || binding.destination != destination
        || expression != value
    {
        return unsupported("primitive store RHS differs from its authored assignment");
    }
    crate::scalar_source_custody::validate_namespace(checked, binding)
}

pub(super) fn emit(
    destination_parameter_index: u32,
    value: &CheckedScalarExpression,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    scalar_parameter_count: usize,
    scalar_values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    let destination = parameters
        .get(usize::try_from(destination_parameter_index).map_err(|_| {
            LoweringError::Unsupported("write-only store parameter index exceeds usize")
        })?)
        .ok_or(LoweringError::Unsupported(
            "write-only store names an unknown structural parameter",
        ))?;
    if !matches!(
        destination.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !destination.qualifications.is_empty()
    {
        return unsupported(
            "write-only store destination lost its exclusive unrestricted unqualified custody",
        );
    }
    let destination_shape = structural_types
        .iter()
        .find(|declaration| declaration.id == destination.structural_type)
        .map(|declaration| &declaration.shape)
        .ok_or(LoweringError::Unsupported(
            "write-only store destination type is absent",
        ))?;
    let StructuralTypeShape::PrimitiveScalar(destination_type) = destination_shape else {
        return unsupported("write-only store destination is not a primitive scalar root");
    };
    let direct_literal = matches!(value, CheckedScalarExpression::IntegerLiteral { .. })
        || matches!(value, CheckedScalarExpression::IeeeFloatLiteral { .. })
        || matches!(
            value,
            CheckedScalarExpression::Boolean(expression)
                if matches!(
                    expression.as_ref(),
                    checked_trees::CheckedBooleanExpression::Constant(_)
                )
        );
    let direct_parameter = match value {
        CheckedScalarExpression::Parameter { position, .. } => *position < scalar_parameter_count,
        CheckedScalarExpression::Boolean(expression) => match expression.as_ref() {
            checked_trees::CheckedBooleanExpression::Parameter { position } => {
                *position < scalar_parameter_count
            }
            _ => false,
        },
        _ => false,
    };
    let direct_result_home = matches!(
        value,
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } if *position >= scalar_parameter_count
            && position.checked_add(1) == Some(scalar_values.len())
            && terminal_scalar_type(*primitive_type).ok() == Some(*destination_type)
            && matches!(
                primitive_type,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            )
    );
    if !direct_literal && !direct_parameter && !direct_result_home {
        return unsupported("write-only store value is outside the admitted direct scalar rung");
    }
    let value = lower_checked_scalar_expression(value)?;
    if value.scalar_type() != *destination_type {
        return unsupported("write-only store value type disagrees with its destination");
    }
    emit_value(
        destination.place,
        *destination_type,
        &value,
        scalar_values,
        next_value,
        operations,
    )
}

pub(super) fn emit_value(
    destination: PlaceId,
    destination_type: ScalarType,
    value: &LoweredDirectExpression,
    scalar_values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<OperationKind, LoweringError> {
    if value.scalar_type() != destination_type || direct_expression_contains_short_circuit(value) {
        return unsupported("primitive store RHS has an incompatible type or unexpanded control");
    }
    let source_types = scalar_values
        .iter()
        .map(|value| value.scalar_type)
        .collect::<Vec<_>>();
    validate_direct_parameter_types(value, &source_types)?;
    let value = emit_direct_expression(value, scalar_values, next_value, operations);
    Ok(OperationKind::WriteOnlyPrimitiveStore { destination, value })
}
