use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_core::diagnostics::Diagnostic;
use omega_machine_instructions::MachineInstructionPlan;

pub fn build_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionPlan, Diagnostic> {
    let target_operations: omega_target_operations::InstructionPlan =
        assigned_target_operations.clone().into();
    let machine_program =
        omega_target_operations_to_machine_program::build_machine_program(&target_operations)?;

    Ok(machine_program.into())
}
