use crate::diagnostics::Diagnostic;
use crate::native::object::{SectionKind, SymbolKind};
use crate::native::platform_object::emit_target_object;
use crate::native::plan::NativePlan;
use crate::native::relocations::RelocationKind;
use crate::native::target::{Architecture, ObjectFormat};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedNativeObject {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub relocations: usize,
}

pub fn emit_native_object(native_plan: &NativePlan) -> Result<EmittedNativeObject, Diagnostic> {
    if native_plan.machine_code.bytes.len() != native_plan.machine_code.byte_count {
        return Err(Diagnostic::error(format!(
            "cannot emit native object for {:?}: encoded {} machine byte(s), planned {} byte(s)",
            native_plan.target,
            native_plan.machine_code.bytes.len(),
            native_plan.machine_code.byte_count
        )));
    }

    if let Some(emitted_object) = emit_target_object(native_plan) {
        return emitted_object;
    }

    emit_omega_native_container(native_plan)
}

fn emit_omega_native_container(
    native_plan: &NativePlan,
) -> Result<EmittedNativeObject, Diagnostic> {
    let text_bytes = native_plan.machine_code.bytes.storage_slice();
    let data_bytes = native_plan.data.bytes.storage_slice();
    let bss_bytes = bss_size(native_plan);

    let mut bytes = Vec::new();
    bytes.extend(b"OMGOBJ\0\0");
    write_u32(&mut bytes, 2);
    write_u32(&mut bytes, architecture_id(native_plan.target.architecture));
    write_u32(
        &mut bytes,
        object_format_id(native_plan.target.object_format),
    );
    write_u64(
        &mut bytes,
        u64::try_from(text_bytes.len()).expect("text size overflow"),
    );
    write_u64(
        &mut bytes,
        u64::try_from(data_bytes.len()).expect("data size overflow"),
    );
    write_u64(
        &mut bytes,
        u64::try_from(bss_bytes).expect("bss size overflow"),
    );

    write_symbols(&mut bytes, native_plan);
    write_relocations(&mut bytes, native_plan);

    bytes.extend(text_bytes);
    bytes.extend(data_bytes);

    Ok(EmittedNativeObject {
        bytes,
        file_name: "omega-native.omgobj".to_owned(),
        format: "omega-native-object-container".to_owned(),
        text_bytes: text_bytes.len(),
        data_bytes: data_bytes.len(),
        bss_bytes,
        symbols: native_plan.object.symbols.len(),
        relocations: native_plan.relocations.records.len(),
    })
}

fn write_symbols(bytes: &mut Vec<u8>, native_plan: &NativePlan) {
    write_u32(
        bytes,
        u32::try_from(native_plan.object.symbols.len()).expect("symbol count overflow"),
    );

    for (_, symbol) in native_plan.object.symbols.iter() {
        write_string(bytes, &symbol.name);
        write_string(bytes, symbol.section.as_deref().unwrap_or(""));
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

fn write_relocations(bytes: &mut Vec<u8>, native_plan: &NativePlan) {
    write_u32(
        bytes,
        u32::try_from(native_plan.relocations.records.len()).expect("relocation count overflow"),
    );

    for (_, relocation) in native_plan.relocations.records.iter() {
        write_string(bytes, &relocation.function_symbol);
        write_u32(bytes, relocation.selected_instruction_index);
        write_u64(
            bytes,
            u64::try_from(relocation.text_offset).expect("relocation text offset overflow"),
        );
        write_u32(
            bytes,
            u32::try_from(relocation.byte_width).expect("relocation byte width overflow"),
        );
        write_string(bytes, &relocation.symbol);
        write_u32(bytes, relocation_kind_id(relocation.kind));
    }
}

fn bss_size(native_plan: &NativePlan) -> usize {
    native_plan
        .object
        .sections
        .iter()
        .find(|(_, section)| section.kind == SectionKind::Bss)
        .map(|(_, section)| section.size)
        .unwrap_or(0)
}

fn architecture_id(architecture: Architecture) -> u32 {
    match architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }
}

fn object_format_id(object_format: ObjectFormat) -> u32 {
    match object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }
}

fn symbol_kind_id(symbol_kind: SymbolKind) -> u32 {
    match symbol_kind {
        SymbolKind::Function => 1,
        SymbolKind::Import => 2,
        SymbolKind::Object => 3,
    }
}

fn relocation_kind_id(relocation_kind: RelocationKind) -> u32 {
    match relocation_kind {
        RelocationKind::Aarch64Page21 => 1,
        RelocationKind::Aarch64PageOffset12 => 2,
        RelocationKind::Aarch64Branch26 => 3,
        RelocationKind::X86_64Absolute64 => 4,
        RelocationKind::X86_64Relative32 => 5,
    }
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    write_u32(
        bytes,
        u32::try_from(value.len()).expect("object string length overflow"),
    );
    bytes.extend(value.as_bytes());
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}
