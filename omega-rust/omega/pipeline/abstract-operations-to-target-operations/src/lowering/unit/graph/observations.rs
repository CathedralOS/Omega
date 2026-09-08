//! Once-only scalar observations and live immutable descriptor establishments.

use super::{KnownUnitInteger, LiveDefinitions};
use crate::lowering::scalar::{
    KnownInteger, KnownScalar, byte_views, equal_integer, order_integer, scalar_parameter_location,
};
use crate::lowering::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    prepared: &super::super::setup::PreparedUnitFunction,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInUnitFunction(function.machine);
    let mut values = scalar_values(live, &prepared.scalar_parameters)?;
    match operation {
        AbstractOperation::ByteSequenceLength { source, .. }
        | AbstractOperation::ByteSequenceRead { source, .. }
        | AbstractOperation::ByteSequenceSubslice { source, .. } => {
            if !prepared
                .parameters
                .iter()
                .any(|parameter| parameter.place == *source)
                && !live.views.contains_key(source)
            {
                return Err(invalid());
            }
            if !matches!(operation, AbstractOperation::ByteSequenceSubslice { .. }) {
                byte_views::lower_byte_observation_with_lengths(
                    operation,
                    function,
                    structural_types,
                    &prepared.parameters,
                    &mut values,
                    &live.lengths,
                    &mut provenance.operations,
                )?;
            }
        }
        _ => {}
    }
    if let AbstractOperation::ByteSequenceSubslice {
        psi_operation,
        result,
        ..
    } = operation
    {
        let view = byte_views::view_for_place(
            function,
            structural_types,
            &prepared.parameters,
            &values,
            &live.lengths,
            result.place,
            &mut Vec::new(),
        )?;
        if live
            .views
            .insert(result.place, (*psi_operation, result.structural_type))
            .is_some()
        {
            return Err(invalid());
        }
        provenance.operations.push(*psi_operation);
        operations.push(TargetUnitOperation::ByteSequenceSubslice {
            result: result.clone(),
            view,
        });
        return Ok(());
    }
    let (psi_operation, result, scalar_type, expression) = match operation {
        AbstractOperation::ByteSequenceLength {
            psi_operation,
            result,
            source,
        } => {
            live.lengths.insert(result.value, *source);
            let expression = values
                .remove(&result.value)
                .ok_or_else(invalid)?
                .into_expression(result.value)?;
            (*psi_operation, result.value, result.scalar_type, expression)
        }
        AbstractOperation::ByteSequenceRead {
            psi_operation,
            result,
            ..
        } => {
            let expression = values
                .remove(&result.value)
                .ok_or_else(invalid)?
                .into_expression(result.value)?;
            (*psi_operation, result.value, result.scalar_type, expression)
        }
        AbstractOperation::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        }
        | AbstractOperation::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        }
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
        _ => return Err(invalid()),
    };
    let shape = match scalar_type {
        ScalarType::Boolean => ValueShape::integer(1, 1),
        ScalarType::Integer(integer) => {
            crate::lowering::scalar_abi::fixed_native_integer_shape(integer).ok_or_else(invalid)?
        }
        _ => return Err(invalid()),
    };
    let home = TargetUnitScalarHomeRequirement {
        defining_operation: psi_operation,
        source_value: result,
        scalar_type,
        shape,
    };
    match scalar_type {
        ScalarType::Boolean => {
            live.boolean_homes.insert(result, home);
        }
        ScalarType::Integer(_) => {
            live.integers.insert(result, KnownUnitInteger::Home(home));
        }
        _ => return Err(invalid()),
    }
    operations.push(TargetUnitOperation::ScalarDefinition {
        result_home: home,
        expression,
    });
    Ok(())
}

fn scalar_values(
    live: &LiveDefinitions,
    parameters: &[ScalarAbiValue],
) -> Result<BTreeMap<ValueId, KnownScalar>, LoweringError> {
    live.integers
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
        .collect()
}
