use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_machine_instructions::{
    MachineInstruction, MachineInstructionFunction, MachineInstructionPlan,
};

mod shapes;

use shapes::lower_machine_instruction_kind;

pub fn build_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionPlan, Diagnostic> {
    let mut machine_instructions = MachineInstructionPlan::with_capacity(
        assigned_target_operations.target,
        assigned_target_operations.functions.len(),
        assigned_target_operations.instructions.len(),
    );

    for (_, function) in assigned_target_operations.functions.iter() {
        let function_instructions = append_machine_instructions(
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

fn append_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
    function: &omega_assigned_target_operations::AssignedTargetOperationFunction,
    output_instructions: &mut Arena<MachineInstruction>,
) -> Result<HandleSpan<MachineInstruction>, Diagnostic> {
    let Some(selected_instructions) = assigned_target_operations
        .instructions
        .span(function.instructions)
    else {
        return Ok(HandleSpan::empty());
    };

    output_instructions.try_insert_many(selected_instructions.iter().enumerate().map(
        |(selected_offset, selected_instruction)| {
            let selected_instruction_index = function
                .instructions
                .start()
                .arena_index()
                .checked_add(u32::try_from(selected_offset).expect("selected instruction overflow"))
                .expect("selected instruction overflow");
            let selected_instruction_handle =
                omega_core::arena::Handle::from_arena_index(selected_instruction_index);

            Ok(MachineInstruction {
                selected_instruction_index,
                source_kind: selected_instruction.kind.clone(),
                kind: lower_machine_instruction_kind(
                    assigned_target_operations,
                    selected_instruction_handle,
                    &selected_instruction.kind,
                )?,
                primary_home: primary_home_handle(assigned_target_operations, &selected_instruction.kind),
                secondary_home: secondary_home_handle(assigned_target_operations, &selected_instruction.kind),
            })
        },
    ))
}

fn primary_home_handle(
    assigned_target_operations: &AssignedTargetOperationPlan,
    kind: &omega_assigned_target_operations::SelectedInstructionKind,
) -> omega_assigned_target_operations::AssignedValueHomeHandle {
    first_runtime_value_handle(kind)
        .map(|handle| assigned_target_operations.runtime_value_home_handle(handle))
        .filter(|handle| handle.is_valid())
        .unwrap_or_else(omega_assigned_target_operations::AssignedValueHomeHandle::invalid)
}

fn secondary_home_handle(
    assigned_target_operations: &AssignedTargetOperationPlan,
    kind: &omega_assigned_target_operations::SelectedInstructionKind,
) -> omega_assigned_target_operations::AssignedValueHomeHandle {
    second_runtime_value_handle(kind)
        .map(|handle| assigned_target_operations.runtime_value_home_handle(handle))
        .filter(|handle| handle.is_valid())
        .unwrap_or_else(omega_assigned_target_operations::AssignedValueHomeHandle::invalid)
}

fn first_runtime_value_handle(
    kind: &omega_assigned_target_operations::SelectedInstructionKind,
) -> Option<omega_assigned_target_operations::RuntimeValueOperandHandle> {
    match kind {
        omega_assigned_target_operations::SelectedInstructionKind::CompareRuntimeValues { left, .. }
        | omega_assigned_target_operations::SelectedInstructionKind::WriteRuntimeStorageBinary { left, .. }
        | omega_assigned_target_operations::SelectedInstructionKind::WriteRuntimePointeeBinary { left, .. }
        | omega_assigned_target_operations::SelectedInstructionKind::WriteRuntimeFrameIndexedBinary { left, .. } => Some(*left),
        _ => None,
    }
}

fn second_runtime_value_handle(
    kind: &omega_assigned_target_operations::SelectedInstructionKind,
) -> Option<omega_assigned_target_operations::RuntimeValueOperandHandle> {
    match kind {
        omega_assigned_target_operations::SelectedInstructionKind::CompareRuntimeValues { right, .. }
        | omega_assigned_target_operations::SelectedInstructionKind::WriteRuntimeStorageBinary { right, .. }
        | omega_assigned_target_operations::SelectedInstructionKind::WriteRuntimePointeeBinary { right, .. }
        | omega_assigned_target_operations::SelectedInstructionKind::WriteRuntimeFrameIndexedBinary { right, .. } => Some(*right),
        _ => None,
    }
}
