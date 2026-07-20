use super::bytes::{write_string, write_u32, write_u64};
use super::ids::{relocation_kind_id, section_kind_id};
use crate::{ObjectPlan, RelocationOrigin, RelocationPlan, object_symbol_name};

pub(super) fn write_relocations(
    bytes: &mut Vec<u8>,
    object: &ObjectPlan,
    relocations: &RelocationPlan,
) {
    write_u32(
        bytes,
        u32::try_from(relocations.record_count()).expect("relocation count overflow"),
    );

    for (_, relocation) in relocations.records() {
        let (origin_id, origin_symbol, selected_instruction_index) = match relocation.origin {
            RelocationOrigin::Instruction {
                function_symbol_handle,
                selected_instruction_index,
            } => (1, function_symbol_handle, selected_instruction_index),
            RelocationOrigin::Materialization {
                object_symbol_handle,
            } => (2, object_symbol_handle, 0),
        };
        write_u32(bytes, origin_id);
        write_string(bytes, object_symbol_name(object, origin_symbol));
        write_u32(bytes, selected_instruction_index);
        write_u32(bytes, section_kind_id(relocation.section));
        write_u64(
            bytes,
            u64::try_from(relocation.offset).expect("relocation section offset overflow"),
        );
        write_u32(
            bytes,
            u32::try_from(relocation.byte_width).expect("relocation byte width overflow"),
        );
        write_string(bytes, object_symbol_name(object, relocation.symbol_handle));
        write_u32(bytes, relocation_kind_id(relocation.kind));
    }
}
