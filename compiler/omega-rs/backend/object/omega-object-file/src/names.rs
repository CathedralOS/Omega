use crate::{ObjectPlan, ObjectSymbolHandle, SectionKind, SymbolPlan, SymbolSection};
use omega_control_flow::MachineFunctionIdentity;
use omega_core::runtime_storage::RuntimeStorageRegion;
use omega_target::{NativeTarget, ObjectFormat};

pub fn object_symbol_handle_by_name(object: &ObjectPlan, symbol_name: &str) -> ObjectSymbolHandle {
    object
        .layout
        .symbols
        .iter()
        .find(|(_, symbol)| symbol.name == symbol_name)
        .map(|(handle, _)| handle)
        .unwrap_or_else(psi_arena::Handle::invalid)
}

pub fn object_symbol_name(object: &ObjectPlan, symbol: ObjectSymbolHandle) -> &str {
    if object.layout.symbols.is_valid(symbol) {
        object.layout.symbols.get(symbol).name.as_str()
    } else {
        ""
    }
}

/// Resolve one exact compiler-private function identity to its validated text
/// symbol. Missing, duplicate, invalid, or non-function bindings fail closed.
pub fn object_function_symbol(
    object: &ObjectPlan,
    identity: MachineFunctionIdentity,
) -> Option<(ObjectSymbolHandle, &SymbolPlan)> {
    if !identity.is_valid() {
        return None;
    }
    let mut matches = object
        .layout
        .function_symbols
        .iter()
        .filter(|(_, binding)| binding.identity == identity);
    let (_, binding) = matches.next()?;
    if matches.next().is_some() || !object.layout.symbols.is_valid(binding.symbol) {
        return None;
    }
    let symbol = object.layout.symbols.get(binding.symbol);
    (symbol.kind == crate::SymbolKind::Function
        && symbol.section == SymbolSection::Section(SectionKind::Text)
        && symbol.size > 0)
        .then_some((binding.symbol, symbol))
}

pub fn object_entry_symbol_name(object: &ObjectPlan) -> &str {
    object_symbol_name(object, object.layout.entry_symbol)
}

/// Stable object-local symbol for a non-entry lowered function.
///
/// Source spelling is deliberately absent: independently selected source and
/// import names may coincide, while compiler-private identity cannot.
pub fn private_function_symbol_name(identity: MachineFunctionIdentity) -> Option<String> {
    if !identity.is_valid() {
        return None;
    }
    let continuation = identity.associated_source_continuation();
    let role = if identity.source_key().is_some() {
        "source"
    } else if identity.program_storage_entry_continuation().is_some() {
        "program_storage_entry_wrapper"
    } else {
        return None;
    };
    Some(format!(
        "__omega_function_{role}_m{}_s{}_g{}",
        continuation.machine.arena_index(),
        continuation.state.arena_index(),
        continuation.segment_index,
    ))
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
