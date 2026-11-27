/*
We derive symbol and section spellings from compiler-private identities and the
declared target policy matrix (`object_target_policy`), and we provide fail-closed
symbol lookups (`object_symbol_handle_by_foreign_locator`,
`object_symbol_handle_by_import_locator`, `object_function_symbol`,
`object_function_reference`) that reject missing, malformed, or duplicate rows
by returning an invalid handle or `None`.
*/

use crate::{
    ObjectPlan, ObjectSymbolHandle, SectionKind, SymbolPlan, SymbolSection, object_target_policy,
};
use function_identity::MachineFunctionIdentity;
use target::{NativeTarget, NormalizedForeignLocator};

pub fn object_symbol_handle_by_foreign_locator(
    object: &ObjectPlan,
    locator: &NormalizedForeignLocator,
) -> ObjectSymbolHandle {
    object_symbol_handle_by_import_locator(
        object,
        target::NormalizedImportLocatorRef::Foreign(locator),
    )
}

pub fn object_symbol_handle_by_import_locator(
    object: &ObjectPlan,
    locator: target::NormalizedImportLocatorRef<'_>,
) -> ObjectSymbolHandle {
    let mut matches = object
        .layout
        .normalized_imports
        .iter()
        .filter(|import| import.locator.as_ref() == locator);
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

pub fn private_function_symbol_name(identity: MachineFunctionIdentity) -> Option<String> {
    let placement = identity.callback_thunk_placement_index()?;
    let caller = identity.caller();
    Some(format!(
        "__omega_callback_v{}_{}_p{}",
        caller.vocabulary_marker.get(),
        caller.program_fingerprint,
        placement,
    ))
}

pub fn normalized_foreign_import_symbol_name(locator: &NormalizedForeignLocator) -> String {
    format!(
        "__omega_foreign_import_{:016x}",
        locator.non_authoritative_compatibility_fingerprint()
    )
}

pub fn normalized_import_symbol_name(locator: target::NormalizedImportLocatorRef<'_>) -> String {
    match locator {
        target::NormalizedImportLocatorRef::Foreign(locator) => {
            normalized_foreign_import_symbol_name(locator)
        }
        target::NormalizedImportLocatorRef::Component(locator) => {
            use std::fmt::Write;
            let mut name = String::from("__omega_component_import_");
            for byte in locator.identity_digest() {
                write!(&mut name, "{byte:02x}").expect("writing a String");
            }
            name
        }
    }
}

pub fn object_symbol_name(object: &ObjectPlan, symbol: ObjectSymbolHandle) -> &str {
    if object.layout.symbols.is_valid(symbol) {
        object.layout.symbols.get(symbol).name.as_str()
    } else {
        ""
    }
}

pub fn object_function_symbol(
    object: &ObjectPlan,
    identity: MachineFunctionIdentity,
) -> Option<(ObjectSymbolHandle, &SymbolPlan)> {
    let (handle, symbol) = object_function_reference(object, identity)?;
    (symbol.section == SymbolSection::Section(SectionKind::Text) && symbol.size > 0)
        .then_some((handle, symbol))
}

pub fn object_function_reference(
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
    (symbol.kind == crate::SymbolKind::Function).then_some((binding.symbol, symbol))
}

pub fn entry_symbol_name(target: NativeTarget) -> String {
    object_target_policy(target).entry_symbol_name.to_owned()
}

pub fn section_name(target: NativeTarget, kind: SectionKind) -> String {
    let policy = object_target_policy(target);
    match kind {
        SectionKind::Text => policy.text_section_name,
        SectionKind::Data => policy.data_section_name,
        SectionKind::Bss => policy.bss_section_name,
    }
    .to_owned()
}

#[cfg(test)]
pub(crate) fn symbol_section_name(target: NativeTarget, section: SymbolSection) -> String {
    match section {
        SymbolSection::None => String::new(),
        SymbolSection::Section(kind) => section_name(target, kind),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        entry_symbol_name, object_symbol_handle_by_foreign_locator, section_name,
        symbol_section_name,
    };
    use crate::{NormalizedImportPlan, ObjectPlan, SectionKind, SymbolSection};
    use target::{
        Architecture, ForeignLocatorCandidate, NativeTarget, ObjectFormat, TargetProfile,
        normalize_foreign_locator,
    };

    #[test]
    fn private_symbol_binds_the_program_and_placement() {
        use function_identity::MachineFunctionIdentity;
        use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

        let name = |fingerprint, placement| {
            let caller = TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([fingerprint; 32]),
            };
            let identity = MachineFunctionIdentity::callback_thunk(caller, placement).unwrap();
            super::private_function_symbol_name(identity).unwrap()
        };
        let first = name(1, 7);
        assert_eq!(
            first,
            format!(
                "__omega_callback_v{}_{}_p7",
                VocabularyMarker::CURRENT.get(),
                "01".repeat(32)
            )
        );
        assert_ne!(first, name(2, 7));
        assert_ne!(first, name(1, 8));
        assert_eq!(
            super::private_function_symbol_name(MachineFunctionIdentity::default()),
            None
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
        let symbol = object
            .layout
            .insert_import("diagnostic-only".into(), &locator);
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
            locator: locator.clone().into(),
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
    fn windows_arm64_spells_the_coff_entry_symbol() {
        assert_eq!(entry_symbol_name(NativeTarget::windows_arm64()), "main");
        assert_eq!(
            section_name(NativeTarget::windows_arm64(), SectionKind::Text),
            ".text"
        );
    }

    #[test]
    fn intel_macos_spells_its_own_declared_mach_o_row() {
        assert_eq!(
            section_name(NativeTarget::macos_x64(), SectionKind::Text),
            "__TEXT,__text"
        );
        assert_eq!(entry_symbol_name(NativeTarget::macos_x64()), "_main");
    }
}
