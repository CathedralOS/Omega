use crate::functions;
use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_core::diagnostics::Diagnostic;
use omega_machine_instructions::{MachineInstructionFunction, MachineInstructionPlan};

pub(crate) fn build_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionPlan, Diagnostic> {
    let mut machine_instructions = MachineInstructionPlan::with_capacity(
        assigned_target_operations.target,
        assigned_target_operations.code.functions.len(),
        assigned_target_operations.code.instructions.len(),
    );
    machine_instructions.semantics = assigned_target_operations.semantics.clone();

    for (_, function) in assigned_target_operations.code.functions.iter() {
        let function_instructions = functions::append_machine_instructions(
            assigned_target_operations,
            function,
            &mut machine_instructions.code.instructions,
        )?;

        machine_instructions
            .code
            .functions
            .insert(MachineInstructionFunction {
                source_key: function.source_key,
                instructions: function_instructions,
            });
    }

    Ok(machine_instructions)
}
