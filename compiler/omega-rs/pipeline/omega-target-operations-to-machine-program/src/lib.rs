use omega_machine_program::MachineProgram;
use omega_target_operations::InstructionPlan;
use psi_diagnostics::Diagnostic;

mod builder;

pub fn build_machine_program(instructions: &InstructionPlan) -> Result<MachineProgram, Diagnostic> {
    builder::build_machine_program(instructions)
}
