/*
We extract canonical referenced ELF import requests (`ElfImportRequest`) from a
`FinalImage`. We validate the import rows, then return an `ElfImportRequest`
for each import with a relocation. Each request retains the symbol handle,
locator and consuming relocations.
*/

use diagnostics::Diagnostic;
use image::{
    FinalImage, FinalImageImportPlan, FinalImageRelocation, FinalImageSection,
    FinalImageSymbolHandle,
};
use object_file::SymbolKind;
use target::{ForeignLocatorCandidate, NormalizedImportLocator, TargetProfile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ElfImportLocator {
    Component {
        locator: target::NormalizedComponentLocator,
        object: Vec<u8>,
        symbol: Vec<u8>,
    },
    Versioned {
        target_profile: TargetProfile,
        compatibility_report_identity: u64,
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
    let imports = &image.symbol_table.imports;
    let mut requests = Vec::with_capacity(imports.len());
    // The first row for a handle is the one that the import validator sees first.
    let mut by_symbol: Vec<_> = imports
        .iter()
        .enumerate()
        .map(|(row, (_, import))| {
            (
                import.symbol_handle.arena_index(),
                import.symbol_handle.generation(),
                row,
            )
        })
        .collect();
    by_symbol.sort_unstable();
    let row_for = |handle: FinalImageSymbolHandle| {
        let key = (handle.arena_index(), handle.generation());
        let index = by_symbol.partition_point(|&(slot, generation, _)| (slot, generation) < key);
        by_symbol
            .get(index)
            .filter(|&&(slot, generation, _)| (slot, generation) == key)
            .map(|&(_, _, row)| row)
    };
    let mut normalized_imports = std::collections::BTreeMap::new();

    for (row, (_, import)) in imports.iter().enumerate() {
        if row_for(import.symbol_handle) != Some(row) {
            return Err(Diagnostic::error(
                "ELF final image retains duplicate import rows for one symbol handle",
            ));
        }
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
            FinalImageImportPlan::Normalized(locator) => {
                let normalized = match locator {
                    NormalizedImportLocator::Foreign(foreign) => {
                        let ForeignLocatorCandidate::ElfVersioned {
                            object,
                            symbol,
                            version,
                        } = foreign.locator()
                        else {
                            return Err(Diagnostic::error(
                                "normalized non-ELF foreign locator reached ELF image planning",
                            ));
                        };
                        ElfImportLocator::Versioned {
                            target_profile: foreign.target(),
                            compatibility_report_identity: foreign
                                .non_authoritative_compatibility_fingerprint(),
                            object: object.clone(),
                            symbol: symbol.clone(),
                            version: version.clone(),
                        }
                    }
                    NormalizedImportLocator::Component(component) => ElfImportLocator::Component {
                        locator: component.clone(),
                        object: format!("$ORIGIN/{}", component.image_filename()).into_bytes(),
                        symbol: component.export_symbol().into_bytes(),
                    },
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
                if let Some((earlier_handle, earlier_locator)) = normalized_imports.insert(
                    locator.non_authoritative_compatibility_fingerprint(),
                    (import.symbol_handle, locator),
                ) {
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
                normalized
            }
            FinalImageImportPlan::None => {
                return Err(Diagnostic::error(format!(
                    "ELF import `{}` has no retained physical import plan",
                    symbol.name
                )));
            }
        };

        requests.push(ElfImportRequest {
            symbol_handle: import.symbol_handle,
            locator,
            relocations: Vec::new(),
        });
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
        if symbol.kind == SymbolKind::Import {
            let row = row_for(relocation.symbol_handle).ok_or_else(|| {
                Diagnostic::error(format!(
                    "ELF relocation references import `{}` without one exact import row",
                    symbol.name
                ))
            })?;
            requests[row].relocations.push(relocation.clone());
        }
    }
    requests.retain(|request| !request.relocations.is_empty());
    Ok(requests)
}

#[cfg(test)]
mod tests {
    use super::{
        ElfImportLocator, FinalImage, FinalImageSection, SymbolKind, canonical_referenced_imports,
    };
    use image::{FinalImageImport, FinalImageImportPlan, FinalImageSymbol};
    use target::{ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator};

    fn imported_image() -> FinalImage {
        normalized_versioned_image(
            NativeTarget::linux_x64(),
            TargetProfile::LinuxX64,
            b"libomega-probes.so",
            b"omega_probe",
            b"OMEGA_1",
        )
        .0
    }

    #[test]
    fn duplicate_import_rows_reject() {
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
    }

    #[test]
    fn shared_component_bytes_preserve_import_sites_with_interleaved_relocations() {
        use crate::test_support::{UTF8_IMPORTS, dynamic_import_inputs, text_entry};
        use crate::{ElfComponentExport, emit_elf_shared_component};

        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let (mut image, _, _) =
                dynamic_import_inputs(target, &UTF8_IMPORTS, "import", "valid", "valid");
            text_entry(&mut image, 0);
            let exports = vec![ElfComponentExport {
                symbol: image.symbol_table.entry_symbol,
                name: b"component_entry".to_vec(),
                slot_commitment: [1; 32],
            }];
            let grouped = emit_elf_shared_component(image.clone(), exports.clone()).unwrap();
            // Move the first import's second site past the second import's site.
            let sites = &mut image.relocation_table.relocations;
            let handles: Vec<_> = sites.iter().map(|(handle, _)| handle).collect();
            let second = sites.get(handles[1]).clone();
            let third = std::mem::replace(sites.get_mut(handles[2]), second);
            *sites.get_mut(handles[1]) = third;
            let interleaved = emit_elf_shared_component(image, exports).unwrap();
            assert_eq!(&grouped.contents()[16..18], &3_u16.to_le_bytes());
            assert_eq!(interleaved.contents(), grouped.contents());
            assert_eq!(interleaved.final_text_bytes(), grouped.final_text_bytes());
        }
    }

    #[test]
    fn stale_relocations_do_not_bind_to_a_reused_symbol_slot() {
        let mut image = imported_image();
        let row = image.symbol_table.imports.iter().next().unwrap().0;
        let stale = image.symbol_table.imports.get(row).symbol_handle;
        let symbol = image.symbol_table.symbols.get(stale).clone();
        assert!(image.symbol_table.symbols.free(stale));
        let live = image.symbol_table.symbols.insert(symbol);
        assert_eq!(live.arena_index(), stale.arena_index());
        assert_ne!(live.generation(), stale.generation());
        assert_eq!(
            canonical_referenced_imports(&image).unwrap_err().message,
            "ELF final image import names an invalid symbol handle"
        );
        image.symbol_table.imports.get_mut(row).symbol_handle = live;
        assert!(canonical_referenced_imports(&image).unwrap().is_empty());

        let site = image.relocation_table.relocations.iter().next().unwrap().0;
        image
            .relocation_table
            .relocations
            .get_mut(site)
            .symbol_handle = live;
        assert!(image.symbol_table.imports.free(row));
        assert_eq!(
            canonical_referenced_imports(&image).unwrap_err().message,
            format!(
                "ELF relocation references import `{}` without one exact import row",
                image.symbol_table.symbols.get(live).name
            )
        );
    }

    fn normalized_versioned_image(
        image_target: NativeTarget,
        target_profile: TargetProfile,
        object: &[u8],
        symbol: &[u8],
        version: &[u8],
    ) -> (FinalImage, target::NormalizedForeignLocator) {
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::ElfVersioned {
                object: object.to_vec(),
                symbol: symbol.to_vec(),
                version: version.to_vec(),
            },
            target_profile,
        )
        .expect("valid versioned ELF locator");
        let mut image = crate::test_support::relocation_image(image_target, 20);
        crate::test_support::import_with_text_relocations(
            &mut image,
            &format!(
                "__omega_foreign_import_{:016x}",
                locator.non_authoritative_compatibility_fingerprint()
            ),
            FinalImageImportPlan::Normalized((locator.clone()).into()),
            &[3, 15],
        );
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
                compatibility_report_identity: locator
                    .non_authoritative_compatibility_fingerprint(),
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

        let diagnostic = crate::emit_elf_executable(image)
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
            import: FinalImageImportPlan::Normalized((locator).into()),
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
            .import = FinalImageImportPlan::Normalized((pe_locator).into());
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
