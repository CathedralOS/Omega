//! Canonical custody for referenced ELF dynamic-import requests.
//!
//! This module deliberately stops before choosing a loader realization. It
//! binds each referenced import's exact final-image symbol handle and retained
//! physical locator to every relocation that consumes it. Raw versioned-ELF
//! bytes are never reconstructed from object-local symbol spellings. A future
//! ELF dynamic-link implementation can consume these rows without ambient
//! library lookup or independently pairing object, symbol, and version.

use omega_image::{
    FinalImage, FinalImageImportPlan, FinalImageRelocation, FinalImageSection,
    FinalImageSymbolHandle,
};
use omega_object_file::SymbolKind;
use omega_target::{ForeignLocatorCandidate, NormalizedForeignLocator, TargetProfile};
use psi_diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ElfImportLocator {
    StringBackedBootstrap {
        library: String,
        symbol: String,
    },
    Versioned {
        target_profile: TargetProfile,
        normalized_identity: u64,
        object: Vec<u8>,
        symbol: Vec<u8>,
        version: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfImportRequest {
    pub(crate) symbol_handle: FinalImageSymbolHandle,
    pub(crate) locator: ElfImportLocator,
    pub(crate) relocations: Vec<FinalImageRelocation>,
}

pub(crate) fn canonical_referenced_imports(
    image: &FinalImage,
) -> Result<Vec<ElfImportRequest>, Diagnostic> {
    let mut requests = Vec::new();
    let mut import_handles = Vec::new();
    let mut normalized_imports = Vec::<(FinalImageSymbolHandle, NormalizedForeignLocator)>::new();

    for (_, import) in image.symbol_table.imports.iter() {
        if import_handles.contains(&import.symbol_handle) {
            return Err(Diagnostic::error(
                "ELF final image retains duplicate import rows for one symbol handle",
            ));
        }
        import_handles.push(import.symbol_handle);
        let symbol = image
            .symbol_table
            .symbols
            .is_valid(import.symbol_handle)
            .then(|| image.symbol_table.symbols.get(import.symbol_handle))
            .ok_or_else(|| {
                Diagnostic::error("ELF final image import names an invalid symbol handle")
            })?;
        if symbol.kind != SymbolKind::Import
            || symbol.section != FinalImageSection::None
            || symbol.offset != 0
            || symbol.size != 0
        {
            return Err(Diagnostic::error(format!(
                "ELF import `{}` is not an unresolved zero-width import symbol",
                symbol.name
            )));
        }
        let locator = match &import.import {
            FinalImageImportPlan::StringBackedBootstrap { library } => {
                if library.is_empty()
                    || library.as_bytes().contains(&0)
                    || symbol.name.is_empty()
                    || symbol.name.as_bytes().contains(&0)
                {
                    return Err(Diagnostic::error(format!(
                        "ELF import `{}` lacks a canonical library/symbol spelling",
                        symbol.name
                    )));
                }
                ElfImportLocator::StringBackedBootstrap {
                    library: library.clone(),
                    symbol: symbol.name.clone(),
                }
            }
            FinalImageImportPlan::Normalized(locator) => {
                let ForeignLocatorCandidate::ElfVersioned {
                    object,
                    symbol,
                    version,
                } = locator.locator()
                else {
                    return Err(Diagnostic::error(format!(
                        "normalized non-ELF foreign locator 0x{:016x} reached ELF image planning",
                        locator.non_authoritative_compatibility_fingerprint(),
                    )));
                };
                if locator.target().native_target() != image.target
                    || !matches!(
                        locator.target(),
                        TargetProfile::LinuxArm64 | TargetProfile::LinuxX64
                    )
                {
                    return Err(Diagnostic::error(format!(
                        "versioned ELF foreign locator 0x{:016x} targets `{}` but ELF image planning targets {:?}",
                        locator.non_authoritative_compatibility_fingerprint(),
                        locator.target().target_name(),
                        image.target,
                    )));
                }
                if let Some((earlier_handle, earlier_locator)) =
                    normalized_imports.iter().find(|(_, earlier)| {
                        earlier.non_authoritative_compatibility_fingerprint()
                            == locator.non_authoritative_compatibility_fingerprint()
                    })
                {
                    let detail = if earlier_locator == locator {
                        "the same exact locator is attached to more than one import symbol"
                    } else {
                        "distinct locators collide on one normalized identity"
                    };
                    return Err(Diagnostic::error(format!(
                        "ELF normalized import identity 0x{:016x} is ambiguous between symbol handles {:?} and {:?}: {detail}",
                        locator.non_authoritative_compatibility_fingerprint(),
                        earlier_handle,
                        import.symbol_handle,
                    )));
                }
                normalized_imports.push((import.symbol_handle, locator.clone()));
                ElfImportLocator::Versioned {
                    target_profile: locator.target(),
                    normalized_identity: locator.non_authoritative_compatibility_fingerprint(),
                    object: object.clone(),
                    symbol: symbol.clone(),
                    version: version.clone(),
                }
            }
            FinalImageImportPlan::None => {
                return Err(Diagnostic::error(format!(
                    "ELF import `{}` has no retained physical import plan",
                    symbol.name
                )));
            }
        };

        let relocations = image
            .relocation_table
            .relocations
            .iter()
            .filter_map(|(_, relocation)| {
                (relocation.symbol_handle == import.symbol_handle).then(|| relocation.clone())
            })
            .collect::<Vec<_>>();
        if !relocations.is_empty() {
            requests.push(ElfImportRequest {
                symbol_handle: import.symbol_handle,
                locator,
                relocations,
            });
        }
    }

    for (_, relocation) in image.relocation_table.relocations.iter() {
        if !image
            .symbol_table
            .symbols
            .is_valid(relocation.symbol_handle)
        {
            continue;
        }
        let symbol = image.symbol_table.symbols.get(relocation.symbol_handle);
        if symbol.kind == SymbolKind::Import
            && !requests
                .iter()
                .any(|request| request.symbol_handle == relocation.symbol_handle)
        {
            return Err(Diagnostic::error(format!(
                "ELF relocation references import `{}` without one exact import row",
                symbol.name
            )));
        }
    }

    Ok(requests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_image::{
        FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSymbol,
    };
    use omega_object_file::RelocationKind;
    use omega_target::{
        ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator,
    };
    use psi_arena::Handle;

    fn imported_image() -> FinalImage {
        let mut image = FinalImage::with_capacity(
            FinalImage::default().target,
            FinalImageMemory {
                text: vec![0; 16],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            1,
            1,
            2,
        );
        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "omega_probe".into(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: imported,
            import: FinalImageImportPlan::StringBackedBootstrap {
                library: "libomega-probes.so".into(),
            },
        });
        for offset in [4, 12] {
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset,
                    byte_width: 4,
                    symbol_handle: imported,
                    addend: 0,
                    kind: RelocationKind::X86_64Relative32,
                });
        }
        image
    }

    #[test]
    fn canonical_request_retains_exact_library_symbol_and_relocation_sites() {
        let image = imported_image();
        let requests = canonical_referenced_imports(&image).expect("canonical request");

        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].locator,
            ElfImportLocator::StringBackedBootstrap {
                library: "libomega-probes.so".into(),
                symbol: "omega_probe".into(),
            }
        );
        assert_eq!(
            requests[0]
                .relocations
                .iter()
                .map(|relocation| relocation.offset)
                .collect::<Vec<_>>(),
            [4, 12]
        );
    }

    #[test]
    fn duplicate_or_unqualified_import_identity_rejects() {
        let mut duplicate = imported_image();
        let imported = duplicate
            .symbol_table
            .imports
            .iter()
            .next()
            .unwrap()
            .1
            .clone();
        duplicate.symbol_table.imports.insert(imported);
        assert!(canonical_referenced_imports(&duplicate).is_err());

        let mut unqualified = imported_image();
        let import_handle = unqualified.symbol_table.imports.iter().next().unwrap().0;
        unqualified
            .symbol_table
            .imports
            .get_mut(import_handle)
            .import = FinalImageImportPlan::StringBackedBootstrap {
            library: String::new(),
        };
        assert!(canonical_referenced_imports(&unqualified).is_err());
    }

    fn normalized_versioned_image(
        image_target: NativeTarget,
        target_profile: TargetProfile,
        object: &[u8],
        symbol: &[u8],
        version: &[u8],
    ) -> (FinalImage, omega_target::NormalizedForeignLocator) {
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::ElfVersioned {
                object: object.to_vec(),
                symbol: symbol.to_vec(),
                version: version.to_vec(),
            },
            target_profile,
        )
        .expect("valid versioned ELF locator");
        let mut image = FinalImage::with_capacity(
            image_target,
            FinalImageMemory {
                text: vec![0; 20],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            1,
            1,
            2,
        );
        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: format!(
                "__omega_foreign_import_{:016x}",
                locator.non_authoritative_compatibility_fingerprint()
            ),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: imported,
            import: FinalImageImportPlan::Normalized(locator.clone()),
        });
        for offset in [3, 15] {
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset,
                    byte_width: 4,
                    symbol_handle: imported,
                    addend: 0,
                    kind: RelocationKind::X86_64Relative32,
                });
        }
        (image, locator)
    }

    #[test]
    fn normalized_versioned_request_retains_raw_coordinates_profile_identity_and_sites() {
        let object = b"libraw\xff.so.6";
        let symbol = b"entry\xfe";
        let version = b"ABI_4\xfd";
        let (image, locator) = normalized_versioned_image(
            NativeTarget::linux_x64(),
            TargetProfile::LinuxX64,
            object,
            symbol,
            version,
        );

        let requests = canonical_referenced_imports(&image).expect("canonical versioned request");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].locator,
            ElfImportLocator::Versioned {
                target_profile: TargetProfile::LinuxX64,
                normalized_identity: locator.non_authoritative_compatibility_fingerprint(),
                object: object.to_vec(),
                symbol: symbol.to_vec(),
                version: version.to_vec(),
            }
        );
        assert_eq!(
            requests[0]
                .relocations
                .iter()
                .map(|relocation| relocation.offset)
                .collect::<Vec<_>>(),
            [3, 15]
        );

        let diagnostic = crate::emit_elf_x86_64_executable(image)
            .expect_err("versioned emission must stop before selecting a dynamic loader");
        assert!(diagnostic.message.contains("PT_INTERP"));
        assert!(diagnostic.message.contains("0x6c6962726177ff2e736f2e36"));
        assert!(diagnostic.message.contains("0x656e747279fe"));
        assert!(diagnostic.message.contains("0x4142495f34fd"));
        assert!(diagnostic.message.contains("2 exact relocation site(s)"));
    }

    #[test]
    fn normalized_version_mutation_changes_the_canonical_request() {
        let request = |version: &[u8]| {
            let (image, _) = normalized_versioned_image(
                NativeTarget::linux_x64(),
                TargetProfile::LinuxX64,
                b"libexact.so.1",
                b"entry",
                version,
            );
            canonical_referenced_imports(&image)
                .expect("canonical versioned request")
                .remove(0)
        };

        let first = request(b"ABI_1");
        let second = request(b"ABI_2");
        assert_ne!(first.locator, second.locator);
        assert_eq!(first.relocations, second.relocations);
    }

    #[test]
    fn duplicate_normalized_locator_or_target_drift_rejects() {
        let (mut duplicate, locator) = normalized_versioned_image(
            NativeTarget::linux_x64(),
            TargetProfile::LinuxX64,
            b"libexact.so.1",
            b"entry",
            b"ABI_1",
        );
        let second_symbol = duplicate.symbol_table.symbols.insert(FinalImageSymbol {
            name: "malformed_duplicate_locator".into(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        duplicate.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: second_symbol,
            import: FinalImageImportPlan::Normalized(locator),
        });
        let diagnostic = canonical_referenced_imports(&duplicate)
            .expect_err("one exact locator attached to two symbols must reject");
        assert!(diagnostic.message.contains("is ambiguous"));

        let (drifted, _) = normalized_versioned_image(
            NativeTarget::linux_arm64(),
            TargetProfile::LinuxX64,
            b"libexact.so.1",
            b"entry",
            b"ABI_1",
        );
        let diagnostic = canonical_referenced_imports(&drifted)
            .expect_err("locator profile/image target drift must reject");
        assert!(diagnostic.message.contains("ELF image planning targets"));
    }

    #[test]
    fn normalized_non_elf_case_and_missing_plan_reject() {
        let pe_locator = normalize_foreign_locator(
            ForeignLocatorCandidate::PeByName {
                library: b"exact.dll".to_vec(),
                export: b"entry".to_vec(),
            },
            TargetProfile::WindowsX64,
        )
        .expect("valid PE locator");
        let mut wrong_case = imported_image();
        wrong_case.target = NativeTarget::windows_x64();
        let import_handle = wrong_case.symbol_table.imports.iter().next().unwrap().0;
        wrong_case
            .symbol_table
            .imports
            .get_mut(import_handle)
            .import = FinalImageImportPlan::Normalized(pe_locator);
        let diagnostic = canonical_referenced_imports(&wrong_case)
            .expect_err("a PE locator must not be reinterpreted as ELF coordinates");
        assert!(diagnostic.message.contains("normalized non-ELF"));

        let mut missing = imported_image();
        let import_handle = missing.symbol_table.imports.iter().next().unwrap().0;
        missing.symbol_table.imports.get_mut(import_handle).import = FinalImageImportPlan::None;
        let diagnostic = canonical_referenced_imports(&missing)
            .expect_err("an import without a physical plan must reject");
        assert!(
            diagnostic
                .message
                .contains("no retained physical import plan")
        );
    }
}
