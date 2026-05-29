use super::super::offsets::{
    runtime_text_buffer_materialize_target_address_offset,
    runtime_text_indexed_buffer_materialize_buffer_address_offset,
};
use super::context::InstructionRelocationContext;
use super::runtime_text_append;
use super::runtime_text_read;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_text_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) {
    if runtime_text_append::collect_runtime_text_append_relocations(context, instruction) {
        return;
    }

    match instruction {
        SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, .. } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_region,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(8, source_symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer, .. } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment { buffer, .. } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::MaterializeRuntimeTextBuffer {
            buffer,
            target_region,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_buffer_materialize_target_address_offset(
                    context.input.target.architecture,
                ),
                target_symbol,
            );
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimePointee {
            buffer, ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let target_symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_buffer_materialize_target_address_offset(
                    context.input.target.architecture,
                ),
                target_symbol,
            );
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
            buffer,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let target_symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(target_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_indexed_buffer_materialize_buffer_address_offset(
                    context.input.target.architecture,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                buffer_symbol,
            );
        }
        SelectedInstructionKind::ReadRuntimeTextLine { .. } => {
            runtime_text_read::collect_runtime_text_read_relocations(context, instruction)
        }
        _ => {}
    }
}
