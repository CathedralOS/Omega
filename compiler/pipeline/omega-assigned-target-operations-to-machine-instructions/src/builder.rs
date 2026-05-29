use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_core::diagnostics::Diagnostic;
use omega_machine_instructions::MachineInstructionPlan;

use crate::code::build_machine_instruction_code;

pub(crate) fn build_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionPlan, Diagnostic> {
    Ok(MachineInstructionPlan {
        target: assigned_target_operations.target,
        code: build_machine_instruction_code(assigned_target_operations)?,
        semantics: assigned_target_operations.semantics.clone(),
    })
}
