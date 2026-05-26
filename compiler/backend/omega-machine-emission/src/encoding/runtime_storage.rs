use crate::MachineEmissionContext;
use crate::branch_distances::byte_distance_to_next_runtime_write_end;
use crate::layout::LaidOutMachineInstruction;
use omega_assigned_target_operations::{
    RuntimeValueOperand, RuntimeValueOperandHandle, StateGuardOperator,
};
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;

fn validate_runtime_value_home(
    input: MachineEmissionContext<'_>,
    operand: RuntimeValueOperandHandle,
) -> Result<(), Diagnostic> {
    let Some(home) = input.assigned_target_operations.runtime_value_home(operand) else {
        return Err(Diagnostic::error(format!(
            "missing assigned runtime value home for operand #{} during machine emission",
            operand.arena_index()
        )));
    };
    let runtime_value = input
        .assigned_target_operations
        .runtime_value_operand(operand)
        .expect("assigned runtime value operand should exist after home validation");
    match &runtime_value.kind {
        RuntimeValueOperand::Immediate(_) => {
            if !matches!(
                home,
                omega_assigned_target_operations::AssignedValueHomeKind::Immediate
            ) {
                return Err(Diagnostic::error(
                    "immediate runtime value must keep an immediate assigned home",
                ));
            }
        }
        RuntimeValueOperand::Storage {
            region,
            byte_offset,
            byte_size,
        } => match region {
            omega_target_operations::RuntimeStorageRegion::Machine => {
                if !matches!(
                    home,
                    omega_assigned_target_operations::AssignedValueHomeKind::RuntimeStorage {
                        region: omega_target_operations::RuntimeStorageRegion::Machine,
                        byte_offset: home_offset,
                        byte_size: home_size,
                    } if home_offset == *byte_offset && home_size == *byte_size
                ) {
                    return Err(Diagnostic::error(
                        "machine runtime storage value must keep a matching runtime-storage home",
                    ));
                }
            }
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
                if !matches!(
                    home,
                    omega_assigned_target_operations::AssignedValueHomeKind::StackSlot {
                        byte_offset: home_offset,
                        byte_size: home_size,
                    } if home_offset == *byte_offset && home_size == *byte_size
                ) {
                    return Err(Diagnostic::error(
                        "runtime-frame value must lower through a matching stack-slot home",
                    ));
                }
            }
        },
        RuntimeValueOperand::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => {
            if !matches!(
                home,
                omega_assigned_target_operations::AssignedValueHomeKind::RuntimePointee {
                    pointer_byte_offset: home_pointer,
                    field_byte_offset: home_field,
                    byte_size: home_size,
                } if home_pointer == *pointer_byte_offset
                    && home_field == *field_byte_offset
                    && home_size == *byte_size
            ) {
                return Err(Diagnostic::error(
                    "runtime pointee value must keep a matching pointee home",
                ));
            }
        }
        RuntimeValueOperand::FrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            if !matches!(
                home,
                omega_assigned_target_operations::AssignedValueHomeKind::RuntimeFrameIndexed {
                    descriptor_offset: home_descriptor,
                    index_offset: home_index,
                    element_byte_size: home_element_size,
                    field_byte_offset: home_field,
                    byte_size: home_size,
                } if home_descriptor == *descriptor_offset
                    && home_index == *index_offset
                    && home_element_size == *element_byte_size
                    && home_field == *field_byte_offset
                    && home_size == *byte_size
            ) {
                return Err(Diagnostic::error(
                    "runtime frame-indexed value must keep a matching frame-indexed home",
                ));
            }
        }
        RuntimeValueOperand::FrameFixedIndexed {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            if !matches!(
                home,
                omega_assigned_target_operations::AssignedValueHomeKind::RuntimeFrameFixedIndexed {
                    descriptor_offset: home_descriptor,
                    element_index: home_index,
                    element_byte_size: home_element_size,
                    field_byte_offset: home_field,
                    byte_size: home_size,
                } if home_descriptor == *descriptor_offset
                    && home_index == *element_index
                    && home_element_size == *element_byte_size
                    && home_field == *field_byte_offset
                    && home_size == *byte_size
            ) {
                return Err(Diagnostic::error(
                    "runtime fixed frame-indexed value must keep a matching frame-indexed home",
                ));
            }
        }
        RuntimeValueOperand::Binary { .. } => {
            if !matches!(
                home,
                omega_assigned_target_operations::AssignedValueHomeKind::ScratchRegister { .. }
                    | omega_assigned_target_operations::AssignedValueHomeKind::StackSlot { .. }
            ) {
                return Err(Diagnostic::error(
                    "binary runtime value must lower through a scratch-register or stack-slot home",
                ));
            }
        }
    }

    Ok(())
}

pub(super) fn encode_runtime_value_compare(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    byte_size: usize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, left)?;
    validate_runtime_value_home(input, right)?;
    architecture::encode_runtime_value_compare(
        input.target.architecture,
        input.assigned_target_operations,
        left,
        right,
        byte_size,
        byte_distance_to_next_runtime_write_end(
            input,
            machine_instructions,
            machine_instruction_index,
        )?,
        operator,
    )
}

pub(super) fn encode_runtime_machine_integer_write(
    input: MachineEmissionContext<'_>,
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_machine_integer_write(
        input.target.architecture,
        byte_offset,
        byte_size,
        value,
    )
}

pub(super) fn encode_runtime_pointee_integer_write(
    input: MachineEmissionContext<'_>,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_pointee_integer_write(
        input.target.architecture,
        pointer_byte_offset,
        field_byte_offset,
        byte_size,
        value,
    )
}

pub(super) fn encode_runtime_storage_binary_write(
    input: MachineEmissionContext<'_>,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, left)?;
    validate_runtime_value_home(input, right)?;
    architecture::encode_runtime_storage_binary_write(
        input.target.architecture,
        input.assigned_target_operations,
        target_offset,
        byte_size,
        left,
        operator,
        right,
    )
}

pub(super) fn encode_runtime_pointee_binary_write(
    input: MachineEmissionContext<'_>,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, left)?;
    validate_runtime_value_home(input, right)?;
    architecture::encode_runtime_pointee_binary_write(
        input.target.architecture,
        input.assigned_target_operations,
        pointer_byte_offset,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    )
}

pub(super) fn encode_runtime_frame_indexed_integer_write(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_frame_indexed_integer_write(
        input.target.architecture,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
        value,
    )
}

pub(super) fn encode_runtime_machine_indexed_integer_write(
    input: MachineEmissionContext<'_>,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_machine_indexed_integer_write(
        input.target.architecture,
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
        value,
    )
}

pub(super) fn encode_runtime_frame_indexed_binary_write(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    validate_runtime_value_home(input, left)?;
    validate_runtime_value_home(input, right)?;
    architecture::encode_runtime_frame_indexed_binary_write(
        input.target.architecture,
        input.assigned_target_operations,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    )
}

pub(super) fn encode_runtime_machine_string_write(
    input: MachineEmissionContext<'_>,
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_machine_string_write(
        input.target.architecture,
        byte_offset,
        byte_length,
    )
}

pub(super) fn encode_runtime_pointee_string_write(
    input: MachineEmissionContext<'_>,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_pointee_string_write(
        input.target.architecture,
        pointer_byte_offset,
        field_byte_offset,
        byte_length,
    )
}

pub(super) fn encode_runtime_frame_indexed_string_write(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_frame_indexed_string_write(
        input.target.architecture,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_length,
    )
}

pub(super) fn encode_runtime_machine_indexed_string_write(
    input: MachineEmissionContext<'_>,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_machine_indexed_string_write(
        input.target.architecture,
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_length,
    )
}

pub(super) fn encode_runtime_storage_address_to_runtime_frame_write(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_address_to_runtime_frame_write(
        input.target.architecture,
        source_offset,
        target_offset,
    )
}

pub(super) fn encode_runtime_pointee_address_to_runtime_frame_write(
    input: MachineEmissionContext<'_>,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_pointee_address_to_runtime_frame_write(
        input.target.architecture,
        pointer_byte_offset,
        field_byte_offset,
        target_offset,
    )
}

pub(super) fn encode_runtime_frame_indexed_address_to_runtime_frame_write(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_frame_indexed_address_to_runtime_frame_write(
        input.target.architecture,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        target_offset,
    )
}

pub(super) fn encode_runtime_storage_copy(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy(
        input.target.architecture,
        source_offset,
        target_offset,
        byte_count,
    )
}

pub(super) fn encode_runtime_storage_copy_to_runtime_frame_indexed(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy_to_runtime_frame_indexed(
        input.target.architecture,
        source_offset,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_count,
    )
}

pub(super) fn encode_runtime_storage_copy_from_runtime_frame_indexed(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy_from_runtime_frame_indexed(
        input.target.architecture,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        target_offset,
        byte_count,
    )
}

pub(super) fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed(
        input.target.architecture,
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        target_offset,
        byte_count,
    )
}

pub(super) fn encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage(
        input.target.architecture,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        target_offset,
        byte_count,
    )
}

pub(super) fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage(
    input: MachineEmissionContext<'_>,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage(
        input.target.architecture,
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        target_offset,
        byte_count,
    )
}

pub(super) fn encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
    input: MachineEmissionContext<'_>,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
        input.target.architecture,
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        target_offset,
        byte_count,
    )
}

pub(super) fn encode_runtime_storage_copy_to_runtime_pointee(
    input: MachineEmissionContext<'_>,
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    architecture::encode_runtime_storage_copy_to_runtime_pointee(
        input.target.architecture,
        source_offset,
        pointer_byte_offset,
        field_byte_offset,
        byte_count,
    )
}
