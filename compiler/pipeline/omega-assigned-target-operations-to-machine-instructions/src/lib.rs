use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_core::diagnostics::Diagnostic;
use omega_machine_instructions::MachineInstructionPlan;

mod builder;
mod functions;
mod shapes;
#[cfg(test)]
mod tests;

pub fn build_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionPlan, Diagnostic> {
    builder::build_machine_instructions(assigned_target_operations)
}
