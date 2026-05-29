use crate::{ObjectPlan, RelocationPlan, SectionKind, object_symbol_name, symbol_section_name};
use bytes::{write_string, write_u32, write_u64};
use ids::{architecture_id, object_format_id, relocation_kind_id, symbol_kind_id};
use omega_target::NativeTarget;

mod bytes;
mod ids;

pub struct ObjectContainerInput<'a> {
    pub target: NativeTarget,
    pub object: &'a ObjectPlan,
    pub relocations: &'a RelocationPlan,
    pub text_bytes: &'a [u8],
    pub data_bytes: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectContainerOutput {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub relocations: usize,
}

pub fn emit_omega_object_container(input: ObjectContainerInput<'_>) -> ObjectContainerOutput {
    let bss_bytes = bss_size(input.object);

    let mut bytes = Vec::new();
    bytes.extend(b"OMGOBJ\0\0");
    write_u32(&mut bytes, 2);
    write_u32(&mut bytes, architecture_id(input.target.architecture));
    write_u32(&mut bytes, object_format_id(input.target.object_format));
    write_u64(
        &mut bytes,
        u64::try_from(input.text_bytes.len()).expect("text size overflow"),
    );
    write_u64(
        &mut bytes,
        u64::try_from(input.data_bytes.len()).expect("data size overflow"),
    );
    write_u64(
        &mut bytes,
        u64::try_from(bss_bytes).expect("bss size overflow"),
    );

    write_symbols(&mut bytes, input.object);
    write_relocations(&mut bytes, input.object, input.relocations);

    bytes.extend(input.text_bytes);
    bytes.extend(input.data_bytes);

    ObjectContainerOutput {
        bytes,
        file_name: "omega-backend.omgobj".to_owned(),
        format: "omega-backend-object-container".to_owned(),
        text_bytes: input.text_bytes.len(),
        data_bytes: input.data_bytes.len(),
        bss_bytes,
        symbols: input.object.symbols.len(),
        relocations: input.relocations.records.len(),
    }
}

fn bss_size(object: &ObjectPlan) -> usize {
    object
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Bss)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

fn write_symbols(bytes: &mut Vec<u8>, object: &ObjectPlan) {
    write_u32(
        bytes,
        u32::try_from(object.symbols.len()).expect("symbol count overflow"),
    );

    for (_, symbol) in object.symbols.iter() {
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

fn write_relocations(bytes: &mut Vec<u8>, object: &ObjectPlan, relocations: &RelocationPlan) {
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
