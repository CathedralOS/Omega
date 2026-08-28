use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_machine_instructions::MachineInstructionPlan;
use psi_diagnostics::Diagnostic;

use crate::code::build_machine_instruction_code;
use crate::semantics::build_machine_instruction_semantic_summary;

pub(crate) fn build_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionPlan, Diagnostic> {
    Ok(MachineInstructionPlan::with_roots(
        assigned_target_operations.target,
        build_machine_instruction_code(assigned_target_operations)?,
        build_machine_instruction_semantic_summary(assigned_target_operations),
    ))
}
