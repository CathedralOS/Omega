use crate::functions;
use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_core::diagnostics::Diagnostic;
use omega_machine_instructions::{MachineInstructionFunction, MachineInstructionPlan};

pub(crate) fn build_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionPlan, Diagnostic> {
    let mut machine_instructions = MachineInstructionPlan::with_capacity(
        assigned_target_operations.target,
        assigned_target_operations.functions.len(),
        assigned_target_operations.instructions.len(),
    );
    machine_instructions.values = assigned_target_operations.values.clone();
    machine_instructions.boundary_edges = assigned_target_operations.boundary_edges.clone();
    machine_instructions.ownership = assigned_target_operations.ownership.clone();

    for (_, function) in assigned_target_operations.functions.iter() {
        let function_instructions = functions::append_machine_instructions(
            assigned_target_operations,
            function,
            &mut machine_instructions.instructions,
        )?;

        machine_instructions
            .functions
            .insert(MachineInstructionFunction {
                source_key: function.source_key,
                instructions: function_instructions,
            });
    }

    Ok(machine_instructions)
}
