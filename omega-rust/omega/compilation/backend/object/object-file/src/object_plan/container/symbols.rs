use super::bytes::{write_string, write_u32, write_u64};
use super::ids::symbol_kind_id;
use crate::{ObjectPlan, symbol_section_name};

pub(super) fn write_symbols(bytes: &mut Vec<u8>, object: &ObjectPlan) {
    write_u32(
        bytes,
        u32::try_from(object.layout.symbols.len()).expect("symbol count overflow"),
    );

    for (_, symbol) in object.layout.symbols.iter() {
        write_string(bytes, &symbol.name);
        write_string(bytes, &symbol_section_name(object.target, symbol.section));
        write_u64(
            bytes,
            u64::try_from(symbol.offset).expect("symbol offset overflow"),
        );
        write_u64(
            bytes,
            u64::try_from(symbol.size).expect("symbol size overflow"),
        );
        write_u32(bytes, symbol_kind_id(symbol.kind));
    }
}
