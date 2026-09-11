//! Scalar-definition lowering for attached Unit bodies.

use super::super::shared::*;
use super::scalar_call::{KnownUnitInteger, insert_known_unit_integer};

pub(in crate::lowering) fn lower_integer_widen(
    operation: &AbstractOperation,
    machine: MachineId,
    parameters: &[ScalarAbiValue],
    nonreturning_boundary: bool,
    values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::IntegerWiden {
        psi_operation,
        result,
        source_type,
        target_type,
        operand,
    } = operation
    else {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(machine));
    };
    let invalid = || LoweringError::IntegerWidenTypeMismatch(*result);
    if nonreturning_boundary || !source_type.can_widen_to(*target_type) {
        return Err(invalid());
    }
    let source_shape =
        super::super::scalar_abi::fixed_native_integer_shape(*source_type).ok_or_else(invalid)?;
    let result_shape =
        super::super::scalar_abi::fixed_native_integer_shape(*target_type).ok_or_else(invalid)?;
    let known = values
        .get(operand)
        .copied()
        .ok_or(LoweringError::UnknownValue(*operand))?;
    let operand_expression = match known {
        KnownUnitInteger::BlockParameter {
            block,
            value,
            scalar_type,
        } => {
            if value != *operand || scalar_type != *source_type {
                return Err(invalid());
            }
            TargetIntegerExpression::BlockParameter(target_operations::TargetScalarBlockValue {
                block,
                value,
                scalar_type: ScalarType::Integer(scalar_type),
            })
        }
        KnownUnitInteger::Parameter {
            parameter_index,
            scalar_type,
        } => {
            let parameter_index = usize::try_from(parameter_index).map_err(|_| invalid())?;
            let parameter = parameters.get(parameter_index).ok_or_else(invalid)?;
            if scalar_type != *source_type
                || parameter.value != *operand
                || parameter.scalar_type != ScalarType::Integer(*source_type)
            {
                return Err(invalid());
            }
            TargetIntegerExpression::Parameter {
                source_value: *operand,
                parameter_index,
                location: super::super::scalar::scalar_parameter_location(
                    &AbstractParameter {
                        value: *operand,
                        scalar_type: parameter.scalar_type,
                    },
                    &parameter.placement,
                )?,
            }
        }
        KnownUnitInteger::Immediate {
            scalar_type, value, ..
        } => {
            if scalar_type != *source_type {
                return Err(invalid());
            }
            TargetIntegerExpression::Immediate {
                source_value: *operand,
                value,
            }
        }
        KnownUnitInteger::Home(home) => {
            if home.source_value != *operand
                || home.scalar_type != ScalarType::Integer(*source_type)
                || home.shape != source_shape
            {
                return Err(invalid());
            }
            TargetIntegerExpression::ScalarHome(home)
        }
    };
    let result_home = TargetUnitScalarHomeRequirement {
        defining_operation: *psi_operation,
        source_value: *result,
        scalar_type: ScalarType::Integer(*target_type),
        shape: result_shape,
    };
    insert_known_unit_integer(values, *result, KnownUnitInteger::Home(result_home))?;
    operations.push(TargetUnitOperation::ScalarDefinition {
        result_home,
        expression: TargetScalarExpression::Integer {
            scalar_type: *target_type,
            expression: TargetIntegerExpression::IntegerWiden {
                psi_operation: *psi_operation,
                source_type: *source_type,
                operand: Box::new(operand_expression),
            },
        },
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_integer_constant(
    machine: MachineId,
    psi_operation: OperationId,
    result: ValueId,
    scalar_type: IntegerType,
    value: IntegerValue,
    nonreturning_boundary: bool,
    integer_constants: &mut BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    if nonreturning_boundary {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(machine));
    }
    if !scalar_type.admits(value) {
        return Err(LoweringError::IntegerConstantOutsideType(result));
    }
    insert_known_unit_integer(
        scalar_values,
        result,
        KnownUnitInteger::Immediate {
            defining_operation: psi_operation,
            scalar_type,
            value,
        },
    )?;
    if integer_constants
        .insert(result, (psi_operation, scalar_type, value))
        .is_some()
    {
        return Err(LoweringError::DuplicateValue(result));
    }
    operations.push(TargetUnitOperation::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value,
    });
    provenance.operations.push(psi_operation);
    Ok(())
}

pub(in crate::lowering) fn lower_boolean_constant(
    machine: MachineId,
    psi_operation: OperationId,
    result: ValueId,
    value: bool,
    nonreturning_boundary: bool,
    boolean_constants: &mut BTreeMap<ValueId, (OperationId, bool)>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    if nonreturning_boundary
        || boolean_constants
            .insert(result, (psi_operation, value))
            .is_some()
    {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(machine));
    }
    operations.push(TargetUnitOperation::BooleanConstant {
        psi_operation,
        result,
        value,
    });
    provenance.operations.push(psi_operation);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_ieee_float_constant(
    psi_operation: OperationId,
    result: ValueId,
    value: semantic_vocabulary::IeeeFloatValue,
    nonreturning_boundary: bool,
    ieee_float_constants: &mut BTreeMap<
        ValueId,
        (OperationId, semantic_vocabulary::IeeeFloatValue),
    >,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    if nonreturning_boundary
        || ieee_float_constants
            .insert(result, (psi_operation, value))
            .is_some()
    {
        return Err(LoweringError::DuplicateValue(result));
    }
    operations.push(TargetUnitOperation::IeeeFloatConstant {
        psi_operation,
        result,
        value,
    });
    provenance.operations.push(psi_operation);
    Ok(())
}
