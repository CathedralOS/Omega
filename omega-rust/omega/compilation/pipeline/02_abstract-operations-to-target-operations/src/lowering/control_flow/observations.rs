//! Once-only scalar observations and live immutable descriptor establishments.

use super::{KnownUnitInteger, LiveDefinitions};
use crate::LoweringError;
use crate::lowering::scalar::{
    KnownInteger, KnownScalar, byte_views, element_views, equal_boolean, equal_integer,
    order_integer, scalar_parameter_location,
};
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{
    AbstractFunction, AbstractOperation, AbstractParameter, IntegerArithmeticOperator,
    IntegerOverflowPolicy,
};
use calling_conventions::ValueShape;
use semantic_vocabulary::{IntegerSign, PlaceId, ScalarType, ValueId};
use std::collections::BTreeMap;
use target_operations::{
    ScalarAbiValue, TargetBooleanExpression, TargetIntegerExpression, TargetScalarExpression,
    TargetUnitOperation, TargetUnitScalarHomeRequirement, TerminalPsiProvenance,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    prepared: &super::super::function_signature::PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let mut values = scalar_values(live, &prepared.scalar_parameters)?;

    let lower_byte_observation = |operation, values, provenance_operations| {
        byte_views::lower_byte_observation_with_lengths(
            operation,
            function,
            structural_types,
            &prepared.parameters,
            live.element_view_backings(),
            values,
            &live.lengths,
            provenance_operations,
        )
    };
    let lower_element_observation = |operation, values, provenance_operations| {
        element_views::lower_element_observation_with_lengths(
            operation,
            function,
            structural_types,
            &prepared.parameters,
            live.element_view_backings(),
            values,
            &live.lengths,
            provenance_operations,
        )
    };
    match operation {
        AbstractOperation::ByteSequenceRead { .. } => {
            // The shared descriptor resolver rejoins the exact live storage
            // or SSA recipe. Producer-family admission here would omit copies
            // and other materialized values with the same readable carrier.
            lower_byte_observation(operation, &mut values, &mut provenance.operations)?;
        }
        AbstractOperation::ByteSequencePredicate { operand, .. } => {
            byte_sequence_operand_gate(function, structural_types, prepared, live, operand)?;
            lower_byte_observation(operation, &mut values, &mut provenance.operations)?;
        }
        AbstractOperation::ByteSequenceEqual { left, right, .. } => {
            for operand in [left, right] {
                byte_sequence_operand_gate(function, structural_types, prepared, live, operand)?;
            }
            lower_byte_observation(operation, &mut values, &mut provenance.operations)?;
        }
        AbstractOperation::ElementViewLength { .. }
        | AbstractOperation::ByteSequenceLength { .. }
        | AbstractOperation::ByteSequenceSubslice { .. }
        | AbstractOperation::ElementViewSubslice { .. } => match operation {
            AbstractOperation::ElementViewLength { .. } => {
                lower_element_observation(operation, &mut values, &mut provenance.operations)?;
            }
            AbstractOperation::ByteSequenceLength { .. } => {
                lower_byte_observation(operation, &mut values, &mut provenance.operations)?;
            }
            _ => {}
        },
        AbstractOperation::ElementViewRead { result, .. }
            if matches!(result.scalar_type, ScalarType::IeeeFloat(_)) => {}
        AbstractOperation::ElementViewRead { .. } => {
            lower_element_observation(operation, &mut values, &mut provenance.operations)?;
        }
        _ => {}
    }
    if let AbstractOperation::ByteSequenceSubslice {
        psi_operation,
        result,
        ..
    } = operation
    {
        let (view, _) = byte_views::resolve_byte_view(
            function,
            structural_types,
            &prepared.parameters,
            live.element_view_backings(),
            &values,
            &live.lengths,
            result.place,
            &mut Vec::new(),
        )?;
        record_view_result(
            live,
            function.machine,
            structural_types,
            provenance,
            *psi_operation,
            result,
        )?;
        operations.push(TargetUnitOperation::ByteSequenceSubslice {
            result: result.clone(),
            view,
        });
        return Ok(());
    }
    if let AbstractOperation::ElementViewSubslice {
        psi_operation,
        result,
        ..
    } = operation
    {
        let (view, _) = element_views::element_view_for_place(
            function,
            structural_types,
            &prepared.parameters,
            live.element_view_backings(),
            &values,
            &live.lengths,
            result.place,
            &mut Vec::new(),
        )?;
        record_view_result(
            live,
            function.machine,
            structural_types,
            provenance,
            *psi_operation,
            result,
        )?;
        operations.push(TargetUnitOperation::ElementViewSubslice {
            result: result.clone(),
            view,
        });
        return Ok(());
    }
    let (psi_operation, result, scalar_type, expression) = match operation {
        AbstractOperation::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } if arithmetic.policy() == IntegerOverflowPolicy::Exact
            && matches!(
                arithmetic.operator(),
                IntegerArithmeticOperator::Add | IntegerArithmeticOperator::Subtract
            ) =>
        {
            use crate::lowering::scalar::{IntegerBinaryKind, lower_integer_binary};
            let known = lower_integer_binary(
                &values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::Arithmetic(*arithmetic),
                *psi_operation,
            )?;
            provenance.operations.push(*psi_operation);
            (
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                TargetScalarExpression::Integer {
                    scalar_type: *scalar_type,
                    expression: known.into_expression(*result),
                },
            )
        }
        AbstractOperation::IntegerArithmetic {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerBitwiseAnd {
            psi_operation,
            result,
            ..
        }
        | abstract_operations::bitwise_or_xor_pair_pattern!(psi_operation, result, ..)
        | AbstractOperation::IntegerBitwiseNot {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerShift {
            psi_operation,
            result,
            ..
        } => {
            if !crate::lowering::scalar::try_lower_integer_operation(
                operation,
                &mut values,
                &mut provenance.operations,
            )? {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let known = values
                .remove(result)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            let expression = known.into_expression(*result)?;
            let TargetScalarExpression::Integer { scalar_type, .. } = &expression else {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            };
            (
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                expression,
            )
        }
        AbstractOperation::IntegerExactCast {
            psi_operation,
            obligation,
            result,
            source_type,
            target_type,
            operand,
        } => {
            let operand_value = match values.get(operand).cloned() {
                Some(KnownScalar::Integer { scalar_type, value })
                    if scalar_type == *source_type
                        && source_type.can_exact_cast_to(*target_type) =>
                {
                    value
                }
                Some(_) => return Err(LoweringError::IntegerExactCastTypeMismatch(*result)),
                None => return Err(LoweringError::UnknownValue(*operand)),
            };
            provenance.operations.push(*psi_operation);
            (
                *psi_operation,
                *result,
                ScalarType::Integer(*target_type),
                TargetScalarExpression::Integer {
                    scalar_type: *target_type,
                    expression: TargetIntegerExpression::IntegerExactCast {
                        psi_operation: *psi_operation,
                        obligation: *obligation,
                        source_type: *source_type,
                        operand: Box::new(operand_value.into_expression(*operand)),
                    },
                },
            )
        }
        // A Trapping primitive is an ordered definition of its own: it is
        // never folded here, even over literal operands, because its crash
        // site and fuel must survive exactly as Terminal owns them.
        AbstractOperation::TrappingInteger {
            psi_operation,
            result,
            scalar_type,
            operand_type,
            operation: primitive,
        } => {
            let operand = |value: ValueId, expected: semantic_vocabulary::IntegerType| match values
                .get(&value)
                .cloned()
            {
                Some(KnownScalar::Integer {
                    scalar_type,
                    value: known,
                }) if scalar_type == expected => Ok(Box::new(known.into_expression(value))),
                Some(_) => Err(LoweringError::ValueTypeMismatch(value)),
                None => Err(LoweringError::UnknownValue(value)),
            };
            use terminal_psi::TrappingIntegerOperation as T;
            let (left, right) = match *primitive {
                terminal_psi::trapping_arith_set_pattern!(left, right, ..) => {
                    if operand_type != scalar_type {
                        return Err(LoweringError::ValueTypeMismatch(*result));
                    }
                    (
                        operand(left, *scalar_type)?,
                        Some(operand(right, *scalar_type)?),
                    )
                }
                T::ShiftLeft { value, count } | T::ShiftRight { value, count } => (
                    operand(value, *scalar_type)?,
                    Some(operand(count, *operand_type)?),
                ),
                T::Convert { operand: source } => (operand(source, *operand_type)?, None),
            };
            provenance.operations.push(*psi_operation);
            (
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                TargetScalarExpression::Integer {
                    scalar_type: *scalar_type,
                    expression: TargetIntegerExpression::Trapping {
                        psi_operation: *psi_operation,
                        primitive: primitive.primitive(),
                        operand_type: *operand_type,
                        left,
                        right,
                    },
                },
            )
        }
        AbstractOperation::ByteSequenceLength {
            psi_operation,
            result,
            source,
        } => {
            if source.path.is_empty() {
                live.lengths.insert(result.value, source.place);
            }
            let expression =
                removed_scalar_expression(&mut values, function.machine, result.value)?;
            (*psi_operation, result.value, result.scalar_type, expression)
        }
        AbstractOperation::ElementViewLength {
            psi_operation,
            result,
            source,
        } => {
            // Whole-view reads use whole-descriptor lengths only. A sibling or
            // projected descriptor cannot donate this observation's bound.
            if source.path.is_empty() {
                live.lengths.insert(result.value, source.place);
            }
            let expression =
                removed_scalar_expression(&mut values, function.machine, result.value)?;
            (*psi_operation, result.value, result.scalar_type, expression)
        }
        abstract_operations::byte_read_predicate_equal_pattern!(psi_operation, result, ..) => {
            let expression = values
                .remove(&result.value)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                .into_expression(result.value)?;
            (*psi_operation, result.value, result.scalar_type, expression)
        }
        AbstractOperation::ElementViewRead {
            psi_operation,
            result,
            ..
        } if matches!(result.scalar_type, ScalarType::IeeeFloat(_)) => {
            let expression = element_views::float_element_read(
                operation,
                function,
                structural_types,
                &prepared.parameters,
                live.element_view_backings(),
                &values,
                &live.lengths,
            )?;
            provenance.operations.push(*psi_operation);
            (*psi_operation, result.value, result.scalar_type, expression)
        }
        AbstractOperation::ElementViewRead {
            psi_operation,
            result,
            ..
        } => {
            let expression = values
                .remove(&result.value)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                .into_expression(result.value)?;
            (*psi_operation, result.value, result.scalar_type, expression)
        }
        AbstractOperation::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            // Ordered definitions retain the equality occurrence even for literal operands.
            let operand = |value| {
                let known = values
                    .get(&value)
                    .cloned()
                    .ok_or(LoweringError::UnknownValue(value))?;
                let TargetScalarExpression::Boolean(expression) = known.into_expression(value)?
                else {
                    return Err(LoweringError::ValueTypeMismatch(value));
                };
                Ok(KnownScalar::BooleanRuntime(expression))
            };
            let known = equal_boolean(operand(*left)?, operand(*right)?, *psi_operation, *result)?;
            provenance.operations.push(*psi_operation);
            (
                *psi_operation,
                *result,
                ScalarType::Boolean,
                known.into_expression(*result)?,
            )
        }
        AbstractOperation::BooleanNot {
            psi_operation,
            result,
            operand,
        } => {
            let known_operand = values
                .get(operand)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*operand))?;
            let TargetScalarExpression::Boolean(operand_expression) =
                known_operand.into_expression(*operand)?
            else {
                return Err(LoweringError::ValueTypeMismatch(*result));
            };
            // The ordered graph still owns this negation's operation and fuel.
            // Folding a literal or cancelling nested negations here would lose
            // that occurrence without an optimizer rewrite and its evidence.
            provenance.operations.push(*psi_operation);
            (
                *psi_operation,
                *result,
                ScalarType::Boolean,
                TargetScalarExpression::Boolean(TargetBooleanExpression::Not {
                    psi_operation: *psi_operation,
                    operand: Box::new(operand_expression),
                }),
            )
        }
        abstract_operations::integer_equal_less_pair_pattern!(
            psi_operation,
            result,
            left,
            right,
            ..
        )
        | AbstractOperation::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let left_value = values
                .get(left)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*left))?;
            let right_value = values
                .get(right)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*right))?;
            let known = if matches!(operation, AbstractOperation::IntegerEqual { .. }) {
                equal_integer(
                    *left,
                    left_value,
                    *right,
                    right_value,
                    *psi_operation,
                    *result,
                )?
            } else {
                order_integer(
                    *left,
                    left_value,
                    *right,
                    right_value,
                    *psi_operation,
                    *result,
                    matches!(operation, AbstractOperation::IntegerLessOrEqual { .. }),
                )?
            };
            provenance.operations.push(*psi_operation);
            (
                *psi_operation,
                *result,
                ScalarType::Boolean,
                known.into_expression(*result)?,
            )
        }
        _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
    };
    let shape = match scalar_type {
        ScalarType::IeeeFloat(format) => ValueShape::float(format.byte_size()),
        ScalarType::Boolean => ValueShape::integer(1, 1),
        ScalarType::Integer(integer) => {
            crate::lowering::scalar_abi::fixed_native_integer_shape(integer)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
        }
    };
    let home = TargetUnitScalarHomeRequirement {
        defining_operation: psi_operation,
        source_value: result,
        scalar_type,
        shape,
    };
    match scalar_type {
        ScalarType::IeeeFloat(_) | ScalarType::Boolean => {
            live.scalar_homes.insert(result, home);
        }
        ScalarType::Integer(_) => {
            live.integers.insert(result, KnownUnitInteger::Home(home));
        }
    }
    operations.push(TargetUnitOperation::ScalarDefinition {
        result_home: home,
        expression,
    });
    Ok(())
}

pub(super) fn scalar_values(
    live: &LiveDefinitions,
    parameters: &[ScalarAbiValue],
) -> Result<BTreeMap<ValueId, KnownScalar>, LoweringError> {
    let mut values = live
        .integers
        .iter()
        .map(|(value, known)| {
            let scalar_type = known.scalar_type();
            let expression = match *known {
                KnownUnitInteger::BlockParameter {
                    block,
                    value,
                    scalar_type,
                } => TargetIntegerExpression::BlockParameter(
                    target_operations::TargetScalarBlockValue {
                        block,
                        value,
                        scalar_type: ScalarType::Integer(scalar_type),
                    },
                ),
                KnownUnitInteger::Parameter {
                    parameter_index, ..
                } => {
                    let parameter_index = usize::try_from(parameter_index)
                        .map_err(|_| LoweringError::UnknownValue(*value))?;
                    let parameter = parameters
                        .get(parameter_index)
                        .ok_or(LoweringError::UnknownValue(*value))?;
                    if parameter.value != *value
                        || parameter.scalar_type != ScalarType::Integer(scalar_type)
                    {
                        return Err(LoweringError::ValueTypeMismatch(*value));
                    }
                    TargetIntegerExpression::Parameter {
                        source_value: *value,
                        parameter_index,
                        location: scalar_parameter_location(
                            &AbstractParameter {
                                value: *value,
                                scalar_type: parameter.scalar_type,
                            },
                            &parameter.placement,
                        )?,
                    }
                }
                KnownUnitInteger::Immediate { value: literal, .. } => {
                    TargetIntegerExpression::Immediate {
                        source_value: *value,
                        value: literal,
                    }
                }
                KnownUnitInteger::Home(home) => TargetIntegerExpression::ScalarHome(home),
            };
            Ok((
                *value,
                KnownScalar::Integer {
                    scalar_type,
                    value: KnownInteger::Runtime(expression),
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
    for (parameter_index, parameter) in parameters.iter().enumerate() {
        if parameter.scalar_type == ScalarType::Boolean {
            values.insert(
                parameter.value,
                KnownScalar::BooleanRuntime(TargetBooleanExpression::Parameter {
                    source_value: parameter.value,
                    parameter_index,
                    location: scalar_parameter_location(
                        &AbstractParameter {
                            value: parameter.value,
                            scalar_type: parameter.scalar_type,
                        },
                        &parameter.placement,
                    )?,
                }),
            );
        }
    }
    for (value, (_, immediate)) in &live.booleans {
        values.insert(*value, KnownScalar::Boolean(*immediate));
    }
    for (value, home) in &live.scalar_homes {
        if home.scalar_type == ScalarType::Boolean {
            values.insert(
                *value,
                KnownScalar::BooleanRuntime(TargetBooleanExpression::ScalarHome(*home)),
            );
        }
    }
    for (value, parameter) in &live.scalar_block_parameters {
        if matches!(parameter.scalar_type, ScalarType::IeeeFloat(_)) {
            continue;
        }
        if parameter.scalar_type != ScalarType::Boolean || parameter.value != *value {
            return Err(LoweringError::ValueTypeMismatch(*value));
        }
        values.insert(
            *value,
            KnownScalar::BooleanRuntime(TargetBooleanExpression::BlockParameter(*parameter)),
        );
    }
    Ok(values)
}

/// Whether one place's own storage is a bounded byte owner: a live owner
/// address join, or an owned `{length, bytes}` home of the same carrier.
/// The semantic operation, not the pointer layout, determines whether an
/// observation of that storage is metadata or payload.
pub(super) fn bounded_owner_storage(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    source: PlaceId,
) -> bool {
    let bounded_owner = |structural_type| {
        structural_types
            .get(&structural_type)
            .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
    };
    (live.address_joins.contains(&source)
        && super::transfers::address_parameter(function, live, source)
            .is_some_and(|parameter| bounded_owner(parameter.structural_type)))
        || live
            .structural_homes
            .get(&source)
            .is_some_and(|home| bounded_owner(home.structural_type()))
}

/// Field operands retain their exact readable-field and measured-length
/// coupling here. Whole operands resolve their live descriptor and measured
/// length through the shared byte operand resolver, not a producer catalogue.
/// A field operand spells an immutable byte view
/// whose length operand is the field-length observation that measured it —
/// the same authority and coupling a concatenation operand asks.
fn byte_sequence_operand_gate(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    prepared: &super::super::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    operand: &terminal_psi::control_flow::ByteSequenceOperand,
) -> Result<(), LoweringError> {
    match operand {
        terminal_psi::control_flow::ByteSequenceOperand::Whole { .. } => {}
        terminal_psi::control_flow::ByteSequenceOperand::Field {
            source,
            field,
            length,
        } => {
            let authority = super::primitive_storage::byte_field_observation(
                function,
                structural_types,
                prepared,
                live,
                source.place,
                &source.path,
                *field,
                terminal_semantics::SequenceFieldObservation::Contents,
            )?;
            if source.access != terminal_psi::StructuralAccess::SharedBorrow
                || authority.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
                || !function.operations.iter().any(|candidate| {
                    matches!(candidate,
                    AbstractOperation::StructuralByteSequenceFieldLength {
                        source: measured, path, field: measured_field, result, ..
                    } if *measured == source.place && *path == source.path
                        && measured_field == field && result.value == *length)
                })
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let scalar = live
                .integers
                .get(length)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                .scalar_type();
            if scalar.sign() != IntegerSign::Unsigned || scalar.bits() != 64 || scalar.is_address()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
        }
    }
    Ok(())
}

/// The scalar expression `value` built from the map entry `result.value`
/// owns, consuming the entry.
fn removed_scalar_expression(
    values: &mut BTreeMap<ValueId, KnownScalar>,
    machine: semantic_vocabulary::MachineId,
    result_value: ValueId,
) -> Result<TargetScalarExpression, LoweringError> {
    values
        .remove(&result_value)
        .ok_or_else(|| LoweringError::unsupported_control_flow(machine))?
        .into_expression(result_value)
}

/// The view `result`'s place records: the live view, its structural home, and
/// the provenance operation.
fn record_view_result(
    live: &mut LiveDefinitions,
    machine: semantic_vocabulary::MachineId,
    structural_types: &StructuralTypeLookup<'_>,
    provenance: &mut TerminalPsiProvenance,
    psi_operation: semantic_vocabulary::OperationId,
    result: &terminal_psi::StructuralOperationResult,
) -> Result<(), LoweringError> {
    if live
        .views
        .insert(result.place, (psi_operation, result.structural_type))
        .is_some()
    {
        return Err(LoweringError::unsupported_control_flow(machine));
    }
    let home = super::aggregate_results::home(psi_operation, result, structural_types)?;
    if live.structural_homes.insert(result.place, home).is_some() {
        return Err(LoweringError::unsupported_control_flow(machine));
    }
    provenance.operations.push(psi_operation);
    Ok(())
}
