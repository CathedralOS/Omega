use super::super::offsets::{
    bounded_buffer_source_append_frame_address_offset,
    runtime_frame_indexed_string_data_address_offset,
    runtime_machine_indexed_string_data_address_offset,
    runtime_machine_indexed_string_runtime_frame_address_offset,
    string_descriptor_machine_address_offset, string_descriptor_pointee_address_offset,
    string_descriptor_runtime_frame_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_target::Architecture;
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
        SelectedInstructionKind::WriteRuntimeMachineBoundedBuffer {
            target_in_frame,
            ..
        } => {
            // The carrier encoder's leading instruction is `mov r15, imm64` (the
            // storage base); content is immediate, so the base is the ONLY
            // relocation. A frame-resident target (a `let`-local struct's carrier
            // field) patches it to the runtime frame base instead of machine storage.
            let base = if *target_in_frame {
                context.runtime_frame_symbol_handle()
            } else {
                context.machine_storage_symbol_handle()
            };
            context.insert_data_address_at_instruction_start(base);
            true
        }
        SelectedInstructionKind::AppendRuntimeMachineBoundedBufferSource {
            source_in_frame,
            ..
        } => {
            // The target carrier is machine-resident, addressed off the leading
            // base materialization. A frame-local source adds a second base
            // (the runtime frame) right after it -- `mov r14, imm64` on x86_64,
            // an `adrp`+`add` pair on aarch64 -- at the arch-aware offset.
            context.insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            if *source_in_frame {
                context.insert_data_address_at_relative_offset(
                    bounded_buffer_source_append_frame_address_offset(
                        context.input.target.architecture,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            }
            true
        }
        SelectedInstructionKind::AppendRuntimeMachineBoundedBufferLiteral { .. } => {
            // The literal bytes are immediates; the target carrier is machine-
            // resident off the leading `mov r15, imm64` base -- the only reloc.
            context.insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
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
        SelectedInstructionKind::WriteRuntimePointeeBoundedBuffer { .. } => {
            // The slice pointer lives in the runtime frame (`mov r15, imm64`
            // leading instruction, then `mov r15, [r15 + ptr]`); the carrier bytes
            // are immediates, so the frame base is the only relocation.
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
            true
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedString {
            data,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            let data_symbol = context.data_object_symbol_handle(*data);
            match context.input.target.architecture {
                Architecture::X86_64 => {
                    // Text rung 1c: the encoder delegates through the place
                    // materializer -- data at the instruction start, the
                    // frame base at +10 (the direct string writes' shared
                    // positions).
                    context.insert_data_address_at_instruction_start(data_symbol);
                    context.insert_data_address_at_relative_offset(
                        string_descriptor_runtime_frame_address_offset(
                            context.input.target.architecture,
                        ),
                        context.runtime_frame_symbol_handle(),
                    );
                }
                Architecture::Aarch64 => {
                    context.insert_data_address_at_instruction_start(
                        context.runtime_frame_symbol_handle(),
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_frame_indexed_string_data_address_offset(
                            context.input.target.architecture,
                            *element_byte_size,
                            *field_byte_offset,
                        ),
                        data_symbol,
                    );
                }
            }
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
