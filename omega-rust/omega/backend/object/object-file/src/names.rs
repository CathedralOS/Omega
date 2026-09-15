//! Symbol spellings derived from compiler-private identity rather than source
//! names, and the lookups that answer with an invalid handle when two rows match.
//!
//! The target-derived spellings — the process-entry name and each
//! `SectionKind` name — resolve through the declared
//! [`object_target_policy`](crate::object_target_policy) matrix rather than
//! reading the object format alone, so an undeclared
//! (architecture, object-format) pair fails closed instead of inheriting
//! whichever format arm happened to match.

use crate::{
    ObjectPlan, ObjectSymbolHandle, SectionKind, SymbolPlan, SymbolSection, object_target_policy,
};
use function_identity::MachineFunctionIdentity;
use target::{NativeTarget, NormalizedForeignLocator};

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
    object_target_policy(target)
        .expect("no object policy is declared for this (architecture, object-format) pair")
        .entry_symbol_name
        .to_owned()
}

pub fn section_name(target: NativeTarget, kind: SectionKind) -> String {
    let policy = object_target_policy(target)
        .expect("no object policy is declared for this (architecture, object-format) pair");
    match kind {
        SectionKind::Text => policy.text_section_name,
        SectionKind::Data => policy.data_section_name,
        SectionKind::Bss => policy.bss_section_name,
    }
    .to_owned()
}

pub fn symbol_section_name(target: NativeTarget, section: SymbolSection) -> String {
    match section {
        SymbolSection::None => String::new(),
        SymbolSection::Section(kind) => section_name(target, kind),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        entry_symbol_name, object_symbol_handle_by_foreign_locator, private_function_symbol_name,
        section_name, symbol_section_name,
    };
    use crate::{
        NormalizedImportPlan, ObjectPlan, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
    };
    use function_identity::{MachineFunctionIdentity, StateKey};
    use target::{
        Architecture, ForeignLocatorCandidate, NativeTarget, ObjectFormat, TargetProfile,
        normalize_foreign_locator,
    };

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

    #[test]
    fn target_derived_names_follow_the_declared_pair_rows() {
        for (target, entry, text, data, bss) in [
            (
                NativeTarget::linux_arm64(),
                "main",
                ".text",
                ".data",
                ".bss",
            ),
            (
                NativeTarget::macos_arm64(),
                "_main",
                "__TEXT,__text",
                "__DATA,__data",
                "__DATA,__bss",
            ),
            (NativeTarget::linux_x64(), "main", ".text", ".data", ".bss"),
            (
                NativeTarget::windows_x64(),
                "main",
                ".text",
                ".data",
                ".bss",
            ),
            (NativeTarget::uefi_x64(), "main", ".text", ".data", ".bss"),
        ] {
            assert_eq!(entry_symbol_name(target), entry);
            assert_eq!(section_name(target, SectionKind::Text), text);
            assert_eq!(section_name(target, SectionKind::Data), data);
            assert_eq!(section_name(target, SectionKind::Bss), bss);
            assert_eq!(
                symbol_section_name(target, SymbolSection::Section(SectionKind::Text)),
                text
            );
        }
        // A section-less symbol derives no spelling and never consults the
        // matrix.
        assert_eq!(
            symbol_section_name(
                NativeTarget {
                    architecture: Architecture::Aarch64,
                    object_format: ObjectFormat::Coff,
                    pointer_size: 8,
                    pointer_alignment: 8,
                },
                SymbolSection::None
            ),
            ""
        );
    }

    #[test]
    #[should_panic(expected = "no object policy is declared")]
    fn entry_symbol_name_fails_closed_on_undeclared_pair() {
        // (Aarch64, COFF) used to inherit "main" from the `Elf | Coff` arm.
        let _ = entry_symbol_name(NativeTarget {
            architecture: Architecture::Aarch64,
            object_format: ObjectFormat::Coff,
            pointer_size: 8,
            pointer_alignment: 8,
        });
    }

    #[test]
    #[should_panic(expected = "no object policy is declared")]
    fn section_name_fails_closed_on_wrong_architecture_pair() {
        // (x86-64, Mach-O) used to inherit "__TEXT,__text" from the MachO arm.
        let _ = section_name(
            NativeTarget {
                architecture: Architecture::X86_64,
                object_format: ObjectFormat::MachO,
                pointer_size: 8,
                pointer_alignment: 8,
            },
            SectionKind::Text,
        );
    }
}
