use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_core::diagnostics::Diagnostic;
use omega_machine_instructions::{
    MachineInstructionCode, MachineInstructionFunction, MachineInstructionPlan,
};

use crate::functions;

pub(crate) fn build_machine_instruction_code(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionCode, Diagnostic> {
    let MachineInstructionPlan { mut code, .. } = MachineInstructionPlan::with_capacity(
        assigned_target_operations.target,
        assigned_target_operations.code.functions.len(),
        assigned_target_operations.code.instructions.len(),
    );

    for (_, function) in assigned_target_operations.code.functions.iter() {
        let function_instructions = functions::append_machine_instructions(
            assigned_target_operations,
            function,
            &mut code.instructions,
        )?;

        code.functions.insert(MachineInstructionFunction {
            source_key: function.source_key,
            instructions: function_instructions,
        });
    }

    Ok(code)
}
