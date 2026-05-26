use omega_core::diagnostics::Diagnostic;
use omega_machine_program::MachineProgram;
use omega_target_operations::InstructionPlan;

pub(crate) fn build_machine_program(
    instructions: &InstructionPlan,
) -> Result<MachineProgram, Diagnostic> {
    let assigned_target_operations =
        omega_target_operations_to_assigned_target_operations::build_assigned_target_operations(
            instructions,
        );
    let machine_instructions =
        omega_assigned_target_operations_to_machine_instructions::build_machine_instructions(
            &assigned_target_operations,
        )?;

    Ok(MachineProgram::from(machine_instructions))
}
