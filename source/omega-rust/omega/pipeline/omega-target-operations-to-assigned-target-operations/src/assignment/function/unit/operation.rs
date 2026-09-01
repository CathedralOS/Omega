use super::{foreign_call, scalar_call};
use crate::assignment::shared::*;

pub(super) fn assign(
    machine: MachineId,
    operation: &TargetUnitOperation,
    preceding_operations: &[TargetUnitOperation],
    target: NativeTarget,
    assigned_scalar_homes: &mut BTreeMap<ValueId, AssignedUnitScalarHome>,
    next_scalar_home: &mut u32,
) -> Result<AssignedUnitOperation, AssignmentError> {
    Ok(match operation {
        TargetUnitOperation::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            bytes,
        } => AssignedUnitOperation::EstablishByteSequenceLiteral {
            psi_operation: *psi_operation,
            place: place.clone(),
            structural_type: structural_type.clone(),
            bytes: bytes.clone(),
        },
        TargetUnitOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => AssignedUnitOperation::IntegerConstant {
            psi_operation: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            value: *value,
        },
        TargetUnitOperation::IeeeFloatConstant {
            psi_operation,
            result,
            value,
        } => AssignedUnitOperation::IeeeFloatConstant {
            psi_operation: *psi_operation,
            result: *result,
            value: *value,
        },
        TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
            psi_operation,
            result,
            format,
            left,
            right,
            addend,
            settlement,
        } => {
            let assign_operand =
                |operand: TargetIeeeFloatFmaOperand,
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
            AssignedUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
                psi_operation: *psi_operation,
                result: *result,
                format: *format,
                left: assign_operand(*left, MachineRegister::X86Xmm(0))?,
                right: assign_operand(*right, MachineRegister::X86Xmm(2))?,
                addend: assign_operand(*addend, MachineRegister::X86Xmm(1))?,
                destination: MachineRegister::X86Xmm(0),
                settlement: *settlement,
            }
        }
        TargetUnitOperation::EstablishTrivialAffineLocal {
            psi_operation,
            place,
            structural_type,
        } => AssignedUnitOperation::EstablishTrivialAffineLocal {
            psi_operation: *psi_operation,
            place: place.clone(),
            structural_type: structural_type.clone(),
        },
        TargetUnitOperation::Call {
            psi_operation,
            callee,
            arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => AssignedUnitOperation::Call {
            psi_operation: *psi_operation,
            callee: *callee,
            result: None,
            copies: arguments
                .iter()
                .map(|argument| AssignedAggregateCopy {
                    place: argument.place,
                    access: argument.access,
                    path: argument.path.clone(),
                    root_structural_type: argument.root_structural_type,
                    structural_type: argument.structural_type,
                    shape: argument.shape,
                    source_byte_offset: argument.source_byte_offset,
                    fixed_array_length: argument.fixed_array_length,
                    element_stride: argument.element_stride,
                    source: argument.source.clone(),
                    destination: argument.destination.clone(),
                })
                .collect(),
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        },
        TargetUnitOperation::ScalarCall {
            psi_operation,
            callee,
            call_plan,
            result_home,
            arguments,
            requirement_obligations,
            crash_continuations,
        } => scalar_call::assign(
            machine,
            *psi_operation,
            *callee,
            call_plan,
            *result_home,
            arguments,
            requirement_obligations,
            crash_continuations,
            preceding_operations,
            target,
            assigned_scalar_homes,
            next_scalar_home,
        )?,
        TargetUnitOperation::InstalledProviderCall {
            psi_operation,
            boundary,
            ..
        } => {
            return Err(
                AssignmentError::InstalledProviderCallRequiresOptimizedLane {
                    machine: machine,
                    operation: *psi_operation,
                    boundary: *boundary,
                },
            );
        }
        TargetUnitOperation::NormalizedForeignCall {
            psi_operation,
            boundary,
            provider_execution,
            binding,
            scalar_arguments,
            result_home,
        } => {
            let (scalar_arguments, result_home) = foreign_call::assign(
                *psi_operation,
                binding,
                target,
                scalar_arguments,
                *result_home,
                preceding_operations,
                assigned_scalar_homes,
                next_scalar_home,
            )?;
            AssignedUnitOperation::NormalizedForeignCall {
                psi_operation: *psi_operation,
                boundary: *boundary,
                provider_execution: *provider_execution,
                binding: binding.clone(),
                scalar_arguments,
                result_home,
            }
        }
        TargetUnitOperation::PortWrite {
            psi_operation,
            service,
            port,
            value,
        } => AssignedUnitOperation::PortWrite {
            psi_operation: *psi_operation,
            service: *service,
            port: *port,
            value: *value,
        },
        TargetUnitOperation::BoundarySettlement {
            psi_operation,
            boundary,
            execution,
            realization,
            scalar_arguments,
            arguments,
            byte_sequence_arguments,
            completion_claim_sources,
            completion_receipts,
        } => AssignedUnitOperation::BoundarySettlement {
            psi_operation: *psi_operation,
            boundary: *boundary,
            execution: *execution,
            realization: *realization,
            scalar_arguments: scalar_arguments.clone(),
            arguments: arguments.clone(),
            byte_sequence_arguments: byte_sequence_arguments.clone(),
            completion_claim_sources: completion_claim_sources.clone(),
            completion_receipts: completion_receipts.clone(),
        },
        TargetUnitOperation::Return {
            psi_edge,
            cleanup_actions,
        } => AssignedUnitOperation::Return {
            psi_edge: *psi_edge,
            cleanup_actions: cleanup_actions.clone(),
        },
    })
}
