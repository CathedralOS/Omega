use super::super::offsets::{
    runtime_frame_indexed_string_data_address_offset,
    runtime_machine_indexed_string_data_address_offset,
    runtime_machine_indexed_string_runtime_frame_address_offset,
    string_descriptor_machine_address_offset, string_descriptor_pointee_address_offset,
    string_descriptor_runtime_frame_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_storage_string_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::WriteRuntimeMachineString { data, .. } => {
            let data_symbol = context.data_object_symbol_handle(*data);
            context.insert_data_address_at_instruction_start(data_symbol);
            context.insert_data_address_at_relative_offset(
                string_descriptor_machine_address_offset(context.input.target.architecture),
                context.machine_storage_symbol_handle(),
            );
            true
        }
        SelectedInstructionKind::WriteRuntimeFrameString { data, .. } => {
            let data_symbol = context.data_object_symbol_handle(*data);
            context.insert_data_address_at_instruction_start(data_symbol);
            context.insert_data_address_at_relative_offset(
                string_descriptor_runtime_frame_address_offset(context.input.target.architecture),
                context.runtime_frame_symbol_handle(),
            );
            true
        }
        SelectedInstructionKind::WriteRuntimePointeeString { data, .. } => {
            let data_symbol = context.data_object_symbol_handle(*data);
            context.insert_data_address_at_instruction_start(data_symbol);
            context.insert_data_address_at_relative_offset(
                string_descriptor_pointee_address_offset(context.input.target.architecture),
                context.runtime_frame_symbol_handle(),
            );
            true
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedString {
            data,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
            let data_symbol = context.data_object_symbol_handle(*data);
            context.insert_data_address_at_relative_offset(
                runtime_frame_indexed_string_data_address_offset(
                    context.input.target.architecture,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                data_symbol,
            );
            true
        }
        SelectedInstructionKind::WriteRuntimeMachineIndexedString {
            data,
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            context.insert_data_address_at_relative_offset(
                runtime_machine_indexed_string_runtime_frame_address_offset(
                    context.input.target.architecture,
                    *base_byte_offset,
                ),
                context.runtime_frame_symbol_handle(),
            );
            let data_symbol = context.data_object_symbol_handle(*data);
            context.insert_data_address_at_relative_offset(
                runtime_machine_indexed_string_data_address_offset(
                    context.input.target.architecture,
                    *base_byte_offset,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                data_symbol,
            );
            true
        }
        _ => false,
    }
}
