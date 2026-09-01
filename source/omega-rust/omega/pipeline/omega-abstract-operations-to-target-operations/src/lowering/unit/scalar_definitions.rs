//! Scalar-definition lowering for attached Unit bodies.

use super::super::shared::*;
use super::scalar_call::{KnownUnitInteger, insert_known_unit_integer};

pub(super) fn validate_unit_scalar_definitions(
    function: &AbstractFunction,
) -> Result<(), LoweringError> {
    let has_ieee_float_fma = function.operations.iter().any(|operation| {
        matches!(
            operation,
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { .. }
        )
    });
    if has_ieee_float_fma
        && function.operations.iter().any(|operation| {
            !matches!(
                operation,
                AbstractOperation::IeeeFloatConstant { .. }
                    | AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { .. }
                    | AbstractOperation::CallUnit { .. }
                    | AbstractOperation::BoundaryCall { .. }
                    | AbstractOperation::ReturnUnit { .. }
            )
        })
    {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_integer_constant(
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

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_ieee_float_constant(
    psi_operation: OperationId,
    result: ValueId,
    value: psi_core::IeeeFloatValue,
    nonreturning_boundary: bool,
    ieee_float_constants: &mut BTreeMap<ValueId, (OperationId, psi_core::IeeeFloatValue)>,
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

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_ieee_float_fma(
    machine: MachineId,
    psi_operation: OperationId,
    result: ValueId,
    format: IeeeFloatFormat,
    left: ValueId,
    right: ValueId,
    addend: ValueId,
    nonreturning_boundary: bool,
    ieee_float_constants: &BTreeMap<ValueId, (OperationId, psi_core::IeeeFloatValue)>,
    ieee_float_fma: &BTreeMap<OperationId, TargetX86ScalarFmaSettlement>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    if nonreturning_boundary {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(machine));
    }
    let operand = |source: ValueId| {
        let Some((defining_operation, value)) = ieee_float_constants.get(&source).copied() else {
            return Err(LoweringError::IeeeFloatFmaOperandMismatch(source));
        };
        if value.format() != format {
            return Err(LoweringError::IeeeFloatFmaOperandMismatch(source));
        }
        Ok(TargetIeeeFloatFmaOperand {
            defining_operation,
            source_value: source,
            value,
        })
    };
    let settlement = ieee_float_fma
        .get(&psi_operation)
        .copied()
        .ok_or(LoweringError::MissingIeeeFloatFmaSettlement(psi_operation))?;
    operations.push(TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
        psi_operation,
        result,
        format,
        left: operand(left)?,
        right: operand(right)?,
        addend: operand(addend)?,
        settlement,
    });
    provenance.operations.push(psi_operation);
    Ok(())
}
