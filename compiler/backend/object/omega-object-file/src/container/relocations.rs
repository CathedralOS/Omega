use super::bytes::{write_string, write_u32, write_u64};
use super::ids::relocation_kind_id;
use crate::{ObjectPlan, RelocationPlan, object_symbol_name};

pub(super) fn write_relocations(
    bytes: &mut Vec<u8>,
    object: &ObjectPlan,
    relocations: &RelocationPlan,
) {
    write_u32(
        bytes,
        u32::try_from(relocations.records.len()).expect("relocation count overflow"),
    );

    for (_, relocation) in relocations.records.iter() {
        write_string(
            bytes,
            object_symbol_name(object, relocation.function_symbol_handle),
        );
        write_u32(bytes, relocation.selected_instruction_index);
        write_u64(
            bytes,
            u64::try_from(relocation.text_offset).expect("relocation text offset overflow"),
        );
        write_u32(
            bytes,
            u32::try_from(relocation.byte_width).expect("relocation byte width overflow"),
        );
        write_string(bytes, object_symbol_name(object, relocation.symbol_handle));
        write_u32(bytes, relocation_kind_id(relocation.kind));
    }
}
