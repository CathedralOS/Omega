//! Symbol spellings derived from compiler-private identity rather than source
//! names, and the lookups that answer with an invalid handle when two rows match.

use crate::{ObjectPlan, ObjectSymbolHandle, SectionKind, SymbolPlan, SymbolSection};
use function_identity::MachineFunctionIdentity;
use target::{NativeTarget, NormalizedForeignLocator, ObjectFormat};

pub fn object_symbol_handle_by_name(object: &ObjectPlan, symbol_name: &str) -> ObjectSymbolHandle {
    object
        .layout
        .symbols
        .iter()
        .find(|(_, symbol)| symbol.name == symbol_name)
        .map(|(handle, _)| handle)
        .unwrap_or_else(arena::Handle::invalid)
}

/// Resolve one exact normalized foreign locator to its object import symbol.
/// Missing, duplicate, or malformed rows fail closed; diagnostic symbol names
/// never participate in this join.
pub fn object_symbol_handle_by_foreign_locator(
    object: &ObjectPlan,
    locator: &NormalizedForeignLocator,
) -> ObjectSymbolHandle {
    let mut matches = object
        .layout
        .normalized_imports
        .iter()
        .filter(|import| &import.locator == locator);
    let Some(import) = matches.next() else {
        return arena::Handle::invalid();
    };
    if matches.next().is_some() {
        return arena::Handle::invalid();
    }
    if !object.layout.symbols.is_valid(import.symbol) {
        return arena::Handle::invalid();
    }
    let symbol = object.layout.symbols.get(import.symbol);
    if symbol.kind == crate::SymbolKind::Import
        && symbol.section == SymbolSection::None
        && symbol.offset == 0
        && symbol.size == 0
    {
        import.symbol
    } else {
        arena::Handle::invalid()
    }
}

/// Stable diagnostic/linker-local label for an atomic foreign import. The
/// spelling is not the physical export name and grants no lookup authority.
pub fn normalized_foreign_import_symbol_name(locator: &NormalizedForeignLocator) -> String {
    format!(
        "__omega_foreign_import_{:016x}",
        locator.non_authoritative_compatibility_fingerprint()
    )
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
        "__omega_function_{role}_m{}_mg{}_s{}_sg{}_segment{}",
        continuation.machine.arena_index(),
        continuation.machine.generation(),
        continuation.state.arena_index(),
        continuation.state.generation(),
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

#[cfg(test)]
mod tests {
    use super::{object_symbol_handle_by_foreign_locator, private_function_symbol_name};
    use crate::{NormalizedImportPlan, ObjectPlan, SymbolKind, SymbolPlan, SymbolSection};
    use function_identity::{MachineFunctionIdentity, StateKey};
    use target::{ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator};

    #[test]
    fn private_function_names_bind_role_handles_generations_and_segment() {
        let continuation = StateKey {
            machine: arena::Handle::from_parts(1, 2),
            state: arena::Handle::from_parts(3, 4),
            segment_index: 5,
        };
        let source = MachineFunctionIdentity::source(continuation);
        let source_name = private_function_symbol_name(source).expect("source private name");
        assert_eq!(
            source_name,
            "__omega_function_source_m1_mg2_s3_sg4_segment5"
        );

        for drifted in [
            StateKey {
                machine: arena::Handle::from_parts(1, 3),
                ..continuation
            },
            StateKey {
                state: arena::Handle::from_parts(3, 5),
                ..continuation
            },
            StateKey {
                segment_index: 6,
                ..continuation
            },
        ] {
            assert_ne!(
                source_name,
                private_function_symbol_name(MachineFunctionIdentity::source(drifted))
                    .expect("drifted source private name")
            );
        }
        assert_ne!(
            source_name,
            private_function_symbol_name(
                MachineFunctionIdentity::program_storage_entry_wrapper(continuation)
                    .expect("wrapper identity")
            )
            .expect("wrapper private name")
        );
        assert!(
            private_function_symbol_name(
                MachineFunctionIdentity::callback_thunk(continuation, 0)
                    .expect("callback identity")
            )
            .is_none()
        );
    }

    #[test]
    fn foreign_locator_lookup_joins_exact_coordinates_and_rejects_duplicates() {
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::PeByName {
                library: b"raw\xff.dll".to_vec(),
                export: b"entry".to_vec(),
            },
            TargetProfile::WindowsX64,
        )
        .expect("valid locator");
        let mutated = normalize_foreign_locator(
            ForeignLocatorCandidate::PeByName {
                library: b"raw\xff.dll".to_vec(),
                export: b"entry2".to_vec(),
            },
            TargetProfile::WindowsX64,
        )
        .expect("valid mutated locator");
        let mut object = ObjectPlan::with_capacity(NativeTarget::windows_x64(), 0, 1);
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: "diagnostic-only".into(),
            section: SymbolSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
            import_library: String::new(),
        });
        object.layout.normalized_imports.push(NormalizedImportPlan {
            symbol,
            locator: locator.clone(),
        });
        assert_eq!(
            object_symbol_handle_by_foreign_locator(&object, &locator),
            symbol
        );
        assert!(
            !object_symbol_handle_by_foreign_locator(&object, &mutated).is_valid(),
            "coordinate mutation must not fall back to diagnostic spelling"
        );

        object.layout.normalized_imports.push(NormalizedImportPlan {
            symbol,
            locator: locator.clone(),
        });
        assert!(
            !object_symbol_handle_by_foreign_locator(&object, &locator).is_valid(),
            "ambiguous exact rows must fail closed"
        );
    }
}
