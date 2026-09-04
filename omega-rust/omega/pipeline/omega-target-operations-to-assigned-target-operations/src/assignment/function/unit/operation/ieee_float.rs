//! IEEE-float FMA settlement and operand-custody assignment.

use super::{
    Architecture, AssignedIeeeFloatFmaOperand, AssignedUnitOperation, AssignmentError, MachineId,
    MachineRegister, NativeTarget, TargetIeeeFloatFmaOperand, TargetUnitOperation,
};

pub(super) fn assign_fused_multiply_add(
    machine: MachineId,
    operation: &TargetUnitOperation,
    preceding_operations: &[TargetUnitOperation],
    target: NativeTarget,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
        psi_operation,
        result,
        format,
        left,
        right,
        addend,
        settlement,
    } = operation
    else {
        unreachable!("IEEE-float FMA assignment receives only its exact operation")
    };
    let assign_operand = |operand: TargetIeeeFloatFmaOperand,
                          register: MachineRegister|
     -> Result<AssignedIeeeFloatFmaOperand, AssignmentError> {
        let matches = preceding_operations
            .iter()
            .filter(|preceding| {
                matches!(preceding,
                    TargetUnitOperation::IeeeFloatConstant {
                        psi_operation,
                        result,
                        value,
                    } if *psi_operation == operand.defining_operation
                        && *result == operand.source_value
                        && *value == operand.value)
            })
            .count();
        if matches != 1 || operand.value.format() != *format {
            return Err(AssignmentError::IeeeFloatFmaCustodyMismatch {
                machine,
                operation: *psi_operation,
            });
        }
        Ok(AssignedIeeeFloatFmaOperand {
            defining_operation: operand.defining_operation,
            source_value: operand.source_value,
            value: operand.value,
            register,
        })
    };
    if target.architecture != Architecture::X86_64
        || settlement.terminal_operation != *psi_operation
        || settlement.format != *format
        || settlement.provider.profile().native_target() != target
    {
        return Err(AssignmentError::IeeeFloatFmaCustodyMismatch {
            machine,
            operation: *psi_operation,
        });
    }
    Ok(AssignedUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
        psi_operation: *psi_operation,
        result: *result,
        format: *format,
        left: assign_operand(*left, MachineRegister::X86Xmm(0))?,
        right: assign_operand(*right, MachineRegister::X86Xmm(2))?,
        addend: assign_operand(*addend, MachineRegister::X86Xmm(1))?,
        destination: MachineRegister::X86Xmm(0),
        settlement: *settlement,
    })
}
