use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_machine_instructions::MachineInstructionPlan;
use psi_diagnostics::Diagnostic;

mod builder;
mod code;
mod functions;
mod outgoing_stack_frames;
mod semantics;
mod shapes;
#[cfg(test)]
mod tests;

pub fn build_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionPlan, Diagnostic> {
    builder::build_machine_instructions(assigned_target_operations)
}
