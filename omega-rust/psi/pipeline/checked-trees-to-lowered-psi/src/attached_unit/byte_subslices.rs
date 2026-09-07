//! Source-bound byte ranges are evaluated at their authored call positions.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::{CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan};

pub(super) fn arguments(
    operation: &CheckedUnitEffectOperationPlan,
) -> &[CheckedUnitStructuralArgumentPlan] {
    match operation {
        CheckedUnitEffectOperationPlan::CallUnit {
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
        } => structural_arguments,
        _ => &[],
    }
}

pub(super) fn contains(operation: &CheckedUnitEffectOperationPlan) -> bool {
    arguments(operation).iter().any(|argument| {
        matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
        )
    })
}

pub(super) fn emit(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
    ordinal: usize,
    parameters: &[StructuralParameterDeclaration],
    structural_parameters: &[(u32, StructuralParameterDeclaration)],
    values: &[ValueDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let argument = arguments(operation)
        .get(ordinal)
        .ok_or(LoweringError::Unsupported(
            "subslice has no checked argument",
        ))?;
    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
        parameter_index,
        expression,
        start,
        end,
    } = &argument.source
    else {
        return unsupported("subslice schedule names a different argument kind");
    };
    let coordinate = match operation {
        CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
        | CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. } => *coordinate,
        _ => return unsupported("subslice schedule has no call coordinate"),
    };
    let authored =
        crate::call_source_custody::authored::locate_source(checked, plan.state, coordinate)?;
    if authored
        .structural_arguments
        .get(ordinal)
        .is_none_or(|(_, source)| source != expression)
    {
        return unsupported("subslice disagrees with its authored call argument");
    }
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(*expression) else {
        return unsupported("subslice has no authored indexed expression");
    };
    let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index) else {
        return unsupported("subslice has no authored range");
    };
    if range.end_inclusive
        || range.start.is_valid() != start.is_some()
        || range.end.is_valid() != end.is_some()
    {
        return unsupported("subslice endpoint presence or inclusivity changed");
    }
    if checked.facts.operators.uses.iter().any(|(_, selected)| {
        selected.expression == *expression
            && (selected.spelling != language_core::OperatorSpelling::Range
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
    }) {
        return unsupported("subslice no longer selects the built-in byte range");
    }
    let parameter = parameters
        .get(*parameter_index as usize)
        .ok_or(LoweringError::Unsupported(
            "subslice source parameter is absent",
        ))?;
    let source_parameter = plan
        .structural_parameters
        .get(*parameter_index as usize)
        .ok_or(LoweringError::Unsupported("subslice source plan is absent"))?;
    let (_, state) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    let source_symbol = checked
        .state_parameters(state)
        .get(source_parameter.position as usize)
        .ok_or(LoweringError::Unsupported(
            "subslice source has no authored parameter",
        ))?
        .symbol;
    if !matches!(checked.expression_table.expression(indexed.collection), ExpressionNode::Name(path)
        if path.symbol == source_symbol && path.head_symbol == source_symbol)
    {
        return unsupported("subslice must retain its exact whole parameter source");
    }
    let structural_type = lookup_type_id(type_ids, &argument.type_identity)?;
    if parameter.structural_type != structural_type
        || parameter.access != StructuralAccess::SharedBorrow
        || parameter.multiplicity != StructuralMultiplicity::Unrestricted
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || !argument.path.is_empty()
        || argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow
    {
        return unsupported("subslice source or result changed immutable byte-view custody");
    }
    let bindings = crate::scalar_bindings::ScalarBindings::new(values.len())
        .with_structural_parameters(structural_parameters);
    let count_type = terminal_scalar_type(PrimitiveType::U64)?;
    let argument_ordinal = u32::try_from(ordinal)
        .map_err(|_| LoweringError::Unsupported("subslice argument ordinal exceeds u32"))?;
    let mut endpoint =
        |retained: &CheckedScalarExpression, role| -> Result<ValueId, LoweringError> {
            let (_, selected) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(plan.state, coordinate.statement_index, role)
                .ok_or(LoweringError::Unsupported(
                    "subslice endpoint has no source-bound scalar plan",
                ))?;
            if selected != retained {
                return unsupported("subslice endpoint differs from its source-bound scalar plan");
            }
            let lowered =
                bindings.expression_at(checked, plan.state, coordinate.statement_index, role)?;
            if lowered.scalar_type() != count_type {
                return unsupported("subslice endpoints must retain u64 values");
            }
            crate::scalar_graph_lowering::validate_direct_parameter_types(
                &lowered,
                &values
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            if let LoweredDirectExpression::ByteSequenceLength { source, .. } = lowered
                && let Some(value) = operations
                    .byte_lengths
                    .iter()
                    .rev()
                    .find_map(|(place, value)| (*place == source).then_some(*value))
            {
                return Ok(value);
            }
            Ok(crate::operation_emission::emit_direct_expression(
                &lowered, values, next_value, operations,
            ))
        };
    let start_value = start
        .as_ref()
        .map(|start| {
            endpoint(
                start,
                CheckedScalarExpressionRole::ByteSequenceSubsliceStart {
                    call_ordinal: coordinate.call_ordinal,
                    argument_ordinal,
                },
            )
        })
        .transpose()?;
    let end_value = end
        .as_ref()
        .map(|end| {
            endpoint(
                end,
                CheckedScalarExpressionRole::ByteSequenceSubsliceEnd {
                    call_ordinal: coordinate.call_ordinal,
                    argument_ordinal,
                },
            )
        })
        .transpose()?;
    let length = operations
        .byte_lengths
        .iter()
        .rev()
        .find_map(|(place, value)| (*place == parameter.place).then_some(*value))
        .unwrap_or_else(|| {
            crate::operation_emission::emit_byte_length(parameter.place, next_value, operations)
        });
    let start = start_value.unwrap_or_else(|| {
        crate::operation_emission::emit_direct_expression(
            &LoweredDirectExpression::IntegerLiteral {
                value: IntegerValue::Unsigned(0),
                scalar_type: count_type,
            },
            values,
            next_value,
            operations,
        )
    });
    let end = end_value.unwrap_or(length);
    let place = place_id(allocate_dense(next_place)?);
    let producer = operations.allocate();
    operations.push(Operation {
        id: producer,
        result: OperationResult::Structural(StructuralOperationResult {
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::ByteSequenceSubslice {
            source: parameter.place,
            start,
            end,
            length,
            obligation: obligation_id(producer.get().checked_add(1).ok_or(
                LoweringError::Unsupported("subslice obligation identity overflows"),
            )?),
        },
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        },
    })
}

pub(super) fn argument_places(
    arguments: &[CheckedUnitStructuralArgumentPlan],
    literals: &[StructuralPlaceDeclaration],
    next_literal: &mut usize,
    derived: &[(usize, PlaceId)],
) -> Result<Vec<PlaceId>, LoweringError> {
    let mut places = Vec::new();
    for (ordinal, argument) in arguments.iter().enumerate() {
        if argument.byte_sequence_literal().is_some() {
            places.extend(literal_argument_places(
                std::slice::from_ref(argument),
                literals,
                next_literal,
            )?);
        } else if matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
        ) {
            places.push(
                derived
                    .iter()
                    .find_map(|(index, place)| (*index == ordinal).then_some(*place))
                    .ok_or(LoweringError::Unsupported(
                        "subslice was not evaluated at its argument position",
                    ))?,
            );
        }
    }
    Ok(places)
}
