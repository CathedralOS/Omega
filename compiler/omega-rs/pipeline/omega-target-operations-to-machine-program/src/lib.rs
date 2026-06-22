use omega_core::diagnostics::Diagnostic;
use omega_machine_program::MachineProgram;
use omega_target_operations::InstructionPlan;

mod builder;

pub fn build_machine_program(instructions: &InstructionPlan) -> Result<MachineProgram, Diagnostic> {
    builder::build_machine_program(instructions)
}
