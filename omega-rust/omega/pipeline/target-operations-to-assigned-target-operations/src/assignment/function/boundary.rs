use crate::assignment::shared::*;

pub(super) fn assign(
    function: &TargetFunction,
    operation: &TargetOperation,
    target: NativeTarget,
) -> Result<AssignedOperation, AssignmentError> {
    let architecture = target.architecture;
    Ok(match operation {
        TargetOperation::ReturnBoundaryPortReadU8 {
            psi_edge,
            psi_operation,
            source_value,
            boundary,
            execution,
            realization,
            arguments,
            completion_claim_sources,
            completion_receipts,
            call_plan,
            structural_parameters,
        } => {
            if architecture != Architecture::X86_64 {
                return Err(AssignmentError::BoundaryPortReadUnsupported {
                    machine: function.machine,
                    architecture,
                });
            }
            AssignedOperation::ReturnBoundaryPortReadU8 {
                psi_edge: *psi_edge,
                psi_operation: *psi_operation,
                source_value: *source_value,
                boundary: *boundary,
                execution: *execution,
                realization: *realization,
                arguments: arguments.clone(),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
                call_plan: call_plan.clone(),
                structural_parameters: structural_parameters.clone(),
            }
        }
        TargetOperation::ExitProcessI32 {
            constant_operation,
            psi_operation,
            nominal_return_edge,
            boundary,
            execution,
            realization,
            argument,
            completion_claim_sources,
            completion_receipts,
        } => {
            let expected_destination = match (target.object_format, architecture) {
                (target::ObjectFormat::Elf, Architecture::X86_64) => MachineRegister::X86Rdi,
                (target::ObjectFormat::Elf, Architecture::Aarch64) => MachineRegister::Aarch64X(0),
                _ => {
                    return Err(AssignmentError::LinuxExitGroupUnsupported {
                        machine: function.machine,
                        target,
                    });
                }
            };
            if argument.destination != expected_destination {
                return Err(AssignmentError::LinuxExitGroupArgumentMismatch(
                    function.machine,
                ));
            }
            AssignedOperation::ExitProcessI32 {
                constant_operation: *constant_operation,
                psi_operation: *psi_operation,
                nominal_return_edge: *nominal_return_edge,
                boundary: *boundary,
                execution: *execution,
                realization: *realization,
                argument: *argument,
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
            }
        }
        _ => unreachable!("boundary assignment receives a boundary carrier"),
    })
}
