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

/// Resolve one exact normalized foreign locator to its object import symbol.
/// Missing, duplicate, or malformed rows fail closed; diagnostic symbol names
/// never participate in this join.
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

/// Stable diagnostic/linker-local label for an atomic foreign import. The
/// spelling is not the physical export name and grants no lookup authority.
pub fn normalized_foreign_import_symbol_name(locator: &NormalizedForeignLocator) -> String {
    format!(
        "__omega_foreign_import_{:016x}",
        locator.non_authoritative_compatibility_fingerprint()
    )
}

/// Physical symbolic import label, preserving the distinct linkage domains.
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

/// Resolve one exact compiler-private function identity to its validated text
/// symbol. Missing, duplicate, invalid, or non-function bindings fail closed.
pub fn object_function_symbol(
    object: &ObjectPlan,
    identity: MachineFunctionIdentity,
) -> Option<(ObjectSymbolHandle, &SymbolPlan)> {
    let (handle, symbol) = object_function_reference(object, identity)?;
    (symbol.section == SymbolSection::Section(SectionKind::Text) && symbol.size > 0)
        .then_some((handle, symbol))
}

/// Resolve an exact private identity in a linkable object, including an
/// undefined reference. Final image consumers require a defined text symbol
/// through `object_function_symbol`; an unresolved reference is not executable.
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
    fn intel_macos_spells_its_own_declared_mach_o_row() {
        // (x86-64, Mach-O) is declared in its own matrix row rather than
        // inheriting the AArch64 Mach-O arm; the spellings coincide.
        assert_eq!(
            section_name(NativeTarget::macos_x64(), SectionKind::Text),
            "__TEXT,__text"
        );
        assert_eq!(entry_symbol_name(NativeTarget::macos_x64()), "_main");
    }
}

/// Shared manifest envelope both immutable-lane containers write: magic,
/// version, the manifest identity bytes, then the manifest content. The
/// two formats differ only in the magic the caller supplies.
pub(crate) fn encode_manifest_envelope(
    magic: &[u8; 8],
    version: u32,
    identity: &[u8],
    content: &[u8],
) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(44_usize.saturating_add(content.len()));
    encoded.extend_from_slice(magic);
    crate::codec_bytes::push_encoded(&mut encoded, version.to_le_bytes());
    encoded.extend_from_slice(identity);
    encoded.extend_from_slice(content);
    encoded
}
