//! Source-bound exclusive byte windows evaluated only on their selected edge.

use super::*;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn validate(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    statement_ordinal: u32,
    argument_ordinal: u32,
    source_position: u32,
    expression: ExpressionHandle,
) -> Result<(), LoweringError> {
    let (machine, authored) = crate::scalar_source_custody::authored_state(checked, state.state)?;
    let parameter = checked
        .state_parameters(authored)
        .get(source_position as usize)
        .ok_or(LoweringError::Unsupported(
            "state subslice source has no authored parameter",
        ))?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(expression) else {
        return unsupported("state subslice has no indexed source expression");
    };
    let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index) else {
        return unsupported("state subslice has no authored range");
    };
    if range.end_inclusive
        || !matches!(
            checked.expression_table.expression(indexed.collection), ExpressionNode::Name(name)
            if name.symbol == parameter.symbol && name.head_symbol == parameter.symbol
                && checked.expression_table.name_path_members(name.members).len() == 1
        )
    {
        return unsupported("state subslice lost its exclusive range or exact parameter");
    }
    let spelling = language_core::OperatorSpelling::Range;
    if checked.facts.operators.uses.iter().any(|(_, selected)| {
        selected.expression == expression
            && (selected.spelling != spelling
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
    }) {
        return unsupported("state subslice no longer selects builtin range meaning");
    }
    if !validation::has_builtin_subslice_meaning(
        &checked.typed,
        machine,
        Some(authored),
        expression,
    ) {
        return unsupported("state subslice cannot replace an authored range operator");
    }
    for (endpoint, role) in [
        (
            range.start,
            CheckedScalarExpressionRole::TransitionSubsliceStart { argument_ordinal },
        ),
        (
            range.end,
            CheckedScalarExpressionRole::TransitionSubsliceEnd { argument_ordinal },
        ),
    ] {
        if !endpoint.is_valid() {
            continue;
        }
        let (binding, value) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state.state, statement_ordinal, role)
            .ok_or(LoweringError::Unsupported(
                "state subslice endpoint has no checked binding",
            ))?;
        if binding.expression != endpoint
            || binding.destination.is_valid()
            || value.primitive_type() != Some(PrimitiveType::U64)
        {
            return unsupported("state subslice endpoint binding changed");
        }
        crate::scalar_source_custody::validate_pure(
            checked,
            binding,
            terminal_scalar_type(PrimitiveType::U64)?,
        )?;
    }
    Ok(())
}

pub(super) fn emit(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    statement_ordinal: u32,
    argument_ordinal: u32,
    expression: ExpressionHandle,
    source: &StructuralParameterDeclaration,
    destination: PlaceId,
    bindings: &crate::scalar_bindings::ScalarBindings,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(expression) else {
        return unsupported("state subslice source disappeared after admission");
    };
    let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index) else {
        return unsupported("state subslice range disappeared after admission");
    };
    let count_type = terminal_scalar_type(PrimitiveType::U64)?;
    let mut endpoint = |expression: ExpressionHandle,
                        role|
     -> Result<Option<ValueId>, LoweringError> {
        if !expression.is_valid() {
            return Ok(None);
        }
        let lowered = bindings.expression_at(checked, state.state, statement_ordinal, role)?;
        if lowered.scalar_type() != count_type || direct_expression_contains_short_circuit(&lowered)
        {
            return unsupported("state subslice endpoint needs a branch-free u64 value");
        }
        validate_direct_parameter_types(
            &lowered,
            &values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        if let LoweredDirectExpression::ByteSequenceLength { source, .. } = &lowered
            && let Some(value) = operations
                .byte_lengths
                .iter()
                .rev()
                .find_map(|(place, value)| (*place == *source).then_some(*value))
        {
            return Ok(Some(value));
        }
        Ok(Some(emit_direct_expression(
            &lowered, values, next_value, operations,
        )))
    };
    let start = endpoint(
        range.start,
        CheckedScalarExpressionRole::TransitionSubsliceStart { argument_ordinal },
    )?;
    let end = endpoint(
        range.end,
        CheckedScalarExpressionRole::TransitionSubsliceEnd { argument_ordinal },
    )?;
    let length = operations
        .byte_lengths
        .iter()
        .rev()
        .find_map(|(place, value)| (*place == source.place).then_some(*value))
        .unwrap_or_else(|| {
            crate::operation_emission::emit_byte_length(source.place, next_value, operations)
        });
    let start = start.unwrap_or_else(|| {
        emit_direct_expression(
            &LoweredDirectExpression::IntegerLiteral {
                value: IntegerValue::Unsigned(0),
                scalar_type: count_type,
            },
            values,
            next_value,
            operations,
        )
    });
    let producer = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        id: producer,
        result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
            place: destination,
            structural_type: source.structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::ByteSequenceSubslice {
            source: source.place,
            start,
            end: end.unwrap_or(length),
            length,
            obligation: obligation_id(producer.get().checked_add(1).ok_or(
                LoweringError::Unsupported("state subslice obligation identity overflows"),
            )?),
        },
    });
    Ok(StructuralPlaceDeclaration {
        id: destination,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type: source.structural_type,
        },
    })
}
