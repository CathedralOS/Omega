use omega_assigned_target_operations::{
    AssignedValueHomeKind, RuntimeValueOperandHandle, SelectedInstructionKind,
};
use psi_arena::Handle;
use psi_diagnostics::Diagnostic;

pub(super) fn ensure_runtime_value_homes(
    assigned_target_operations: &omega_assigned_target_operations::AssignedTargetOperationPlan,
    selected_instruction_handle: Handle<omega_assigned_target_operations::SelectedInstruction>,
    kind: &SelectedInstructionKind,
) -> Result<(), Diagnostic> {
    let selected_instruction = assigned_target_operations
        .code
        .instructions
        .get(selected_instruction_handle);
    for handle in [
        first_runtime_value_handle(kind),
        second_runtime_value_handle(kind),
    ]
    .into_iter()
    .flatten()
    {
        let home_handle = assigned_target_operations.runtime_value_home_handle(handle);
        if !home_handle.is_valid() {
            return Err(Diagnostic::error(format!(
                "missing assigned value home for {:?} in {:?} statement {}",
                handle, selected_instruction.source_key, selected_instruction.source_statement
            )));
        }
        let home = assigned_target_operations
            .runtime_value_home(handle)
            .expect("validated assigned runtime value home should exist");

        if matches!(
            assigned_target_operations
                .runtime_value_operand(handle)
                .expect("validated assigned runtime value operand should exist")
                .kind,
            omega_assigned_target_operations::RuntimeValueOperand::Binary { .. }
        ) && !matches!(home, AssignedValueHomeKind::ScratchRegister { .. })
        {
            return Err(Diagnostic::error(format!(
                "binary runtime value {:?} in {:?} statement {} must lower through a scratch register home",
                handle, selected_instruction.source_key, selected_instruction.source_statement
            )));
        }
    }

    Ok(())
}

fn first_runtime_value_handle(kind: &SelectedInstructionKind) -> Option<RuntimeValueOperandHandle> {
    match kind {
        SelectedInstructionKind::CompareRuntimeValues { left, .. }
        | SelectedInstructionKind::WritePlaceBinary { left, .. } => Some(*left),
        _ => None,
    }
}

fn second_runtime_value_handle(
    kind: &SelectedInstructionKind,
) -> Option<RuntimeValueOperandHandle> {
    match kind {
        SelectedInstructionKind::CompareRuntimeValues { right, .. }
        | SelectedInstructionKind::WritePlaceBinary { right, .. } => Some(*right),
        _ => None,
    }
}
