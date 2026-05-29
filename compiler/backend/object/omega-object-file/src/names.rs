use crate::{ObjectPlan, ObjectSymbolHandle, SectionKind, SymbolSection};
use omega_target::{NativeTarget, ObjectFormat};
use omega_target_operations::RuntimeStorageRegion;

pub fn object_symbol_handle_by_name(object: &ObjectPlan, symbol_name: &str) -> ObjectSymbolHandle {
    object
        .symbols
        .iter()
        .find(|(_, symbol)| symbol.name == symbol_name)
        .map(|(handle, _)| handle)
        .unwrap_or_else(omega_core::arena::Handle::invalid)
}

pub fn object_symbol_name(object: &ObjectPlan, symbol: ObjectSymbolHandle) -> &str {
    if object.symbols.is_valid(symbol) {
        object.symbols.get(symbol).name.as_str()
    } else {
        ""
    }
}

pub fn object_entry_symbol_name(object: &ObjectPlan) -> &str {
    object_symbol_name(object, object.entry_symbol)
}

pub fn entry_symbol_name(target: NativeTarget) -> String {
    match target.object_format {
        ObjectFormat::MachO => "_main".to_owned(),
        ObjectFormat::Elf | ObjectFormat::Coff => "main".to_owned(),
    }
}

pub fn section_name(target: NativeTarget, kind: SectionKind) -> String {
    match (target.object_format, kind) {
        (ObjectFormat::MachO, SectionKind::Text) => "__TEXT,__text".to_owned(),
        (ObjectFormat::MachO, SectionKind::Data) => "__DATA,__data".to_owned(),
        (ObjectFormat::MachO, SectionKind::Bss) => "__DATA,__bss".to_owned(),
        (_, SectionKind::Text) => ".text".to_owned(),
        (_, SectionKind::Data) => ".data".to_owned(),
        (_, SectionKind::Bss) => ".bss".to_owned(),
    }
}

pub fn symbol_section_name(target: NativeTarget, section: SymbolSection) -> String {
    match section {
        SymbolSection::None => String::new(),
        SymbolSection::Section(kind) => section_name(target, kind),
    }
}

pub fn machine_storage_symbol_name(machine_name: &str) -> String {
    format!("omega_machine_{machine_name}_storage")
}

pub fn runtime_frame_storage_symbol_name() -> String {
    "omega_runtime_frame_storage".to_owned()
}

pub fn storage_region_symbol_name(
    region: RuntimeStorageRegion,
    entry_machine_name: &str,
) -> String {
    match region {
        RuntimeStorageRegion::Machine => machine_storage_symbol_name(entry_machine_name),
        RuntimeStorageRegion::RuntimeFrame => runtime_frame_storage_symbol_name(),
    }
}
