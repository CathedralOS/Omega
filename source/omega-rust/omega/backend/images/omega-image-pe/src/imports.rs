use crate::bytes::{write_i32_at, write_u16_at, write_u32_at, write_u64_at};
use crate::constants::IMAGE_BASE;
use crate::layout::align_to;
use omega_calling_conventions::{
    MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};
use omega_image::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalImage, FinalImageImportPlan,
    FinalImageLayout, FinalImageSection,
};
use omega_object_file::SymbolKind;
use omega_target::{Architecture, ForeignLocatorCandidate, ObjectFormat, TargetProfile};
use psi_diagnostics::Diagnostic;

const IMAGE_ORDINAL_FLAG64: u64 = 1 << 63;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PeImportLookup {
    ByName(Vec<u8>),
    ByOrdinal(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PeImportThunk {
    /// Object-local diagnostic label. Physical lookup never uses this field.
    pub(crate) symbol: String,
    pub(crate) library: Vec<u8>,
    pub(crate) lookup: PeImportLookup,
    pub(crate) text_offset: usize,
}

pub(crate) fn install_import_thunks(
    image: &mut FinalImage,
    subsystem: u16,
) -> Result<Vec<PeImportThunk>, Diagnostic> {
    if image.target.architecture != Architecture::X86_64
        || image.target.object_format != ObjectFormat::Coff
    {
        return Err(Diagnostic::error(
            "PE x86_64 import installation received a non-COFF/x86_64 image target",
        ));
    }
    let mut imports = Vec::with_capacity(image.symbol_table.imports.len());
    for (_, import) in image.symbol_table.imports.iter() {
        if !image.symbol_table.symbols.is_valid(import.symbol_handle) {
            return Err(Diagnostic::error(
                "PE final image import names an invalid symbol handle",
            ));
        }
        if imports
            .iter()
            .any(|(symbol, _)| *symbol == import.symbol_handle)
        {
            return Err(Diagnostic::error(
                "PE final image retains duplicate import rows for one symbol handle",
            ));
        }
        let symbol = image.symbol_table.symbols.get(import.symbol_handle);
        if symbol.kind != SymbolKind::Import
            || symbol.section != FinalImageSection::None
            || symbol.offset != 0
            || symbol.size != 0
        {
            return Err(Diagnostic::error(format!(
                "PE import `{}` is not an unresolved zero-width import symbol",
                symbol.name
            )));
        }
        imports.push((import.symbol_handle, import.import.clone()));
    }
    let mut thunks = Vec::new();

    for (symbol_handle, import) in imports {
        let symbol = image.symbol_table.symbols.get(symbol_handle).name.clone();
        let (library, lookup) = match import {
            FinalImageImportPlan::StringBackedBootstrap { library } => {
                let library = if library.is_empty() {
                    omega_calling_conventions::windows_import_library(&symbol)
                        .unwrap_or("KERNEL32.dll")
                        .as_bytes()
                        .to_vec()
                } else {
                    library.into_bytes()
                };
                (library, PeImportLookup::ByName(symbol.as_bytes().to_vec()))
            }
            FinalImageImportPlan::Normalized(locator) => {
                if locator.target() != TargetProfile::WindowsX64
                    || locator.target().native_target() != image.target
                    || matches!(subsystem, 10..=12)
                {
                    return Err(Diagnostic::error(format!(
                        "normalized foreign locator 0x{:016x} is not applicable to this PE image target",
                        locator.non_authoritative_compatibility_fingerprint(),
                    )));
                }
                match locator.locator() {
                    ForeignLocatorCandidate::PeByName { library, export } => {
                        (library.clone(), PeImportLookup::ByName(export.clone()))
                    }
                    ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                        (library.clone(), PeImportLookup::ByOrdinal(*ordinal))
                    }
                    ForeignLocatorCandidate::ElfVersioned { .. } => {
                        return Err(Diagnostic::error(format!(
                            "versioned ELF foreign locator 0x{:016x} cannot be emitted in a PE import table",
                            locator.non_authoritative_compatibility_fingerprint(),
                        )));
                    }
                }
            }
            FinalImageImportPlan::None => {
                return Err(Diagnostic::error(format!(
                    "PE import symbol `{symbol}` has no retained physical import plan"
                )));
            }
        };
        if library.is_empty() || library.contains(&0) {
            return Err(Diagnostic::error(format!(
                "PE import `{symbol}` has an invalid library byte coordinate"
            )));
        }
        if matches!(&lookup, PeImportLookup::ByName(export) if export.is_empty() || export.contains(&0))
        {
            return Err(Diagnostic::error(format!(
                "PE import `{symbol}` has an invalid export byte coordinate"
            )));
        }
        let text_offset = image.memory.text.len();
        image.memory.text.extend([0xff, 0x25, 0, 0, 0, 0]);

        let image_symbol = image.symbol_table.symbols.get_mut(symbol_handle);
        image_symbol.section = FinalImageSection::Text;
        image_symbol.offset = text_offset;
        image_symbol.size = 6;

        image.executable_regions.push(FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::ImportThunk,
            section_offset: text_offset,
            byte_count: 6,
            symbol: symbol.clone(),
            footprint: None,
        });

        thunks.push(PeImportThunk {
            symbol,
            library,
            lookup,
            text_offset,
        });
    }

    Ok(thunks)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PeImportTable {
    pub(crate) bytes: Vec<u8>,
    pub(crate) iat_rvas: Vec<u32>,
    pub(crate) import_directory_rva: u32,
    pub(crate) import_directory_size: usize,
    pub(crate) iat_rva: u32,
    pub(crate) iat_size: usize,
}

pub(crate) fn build_import_table(imports: &[PeImportThunk], rdata_rva: u32) -> PeImportTable {
    // No-host / EFI targets import NOTHING -- services arrive through a parameter
    // (the UEFI SystemTable), never the import table. Emit no import directory at
    // all (RVA/size 0) rather than a lone null descriptor, so the image is a clean
    // import-free PE32+. Mirrors the `.reloc` has_reloc gating in lib.rs; the
    // header threads these zeros straight through.
    if imports.is_empty() {
        return PeImportTable {
            bytes: Vec::new(),
            iat_rvas: Vec::new(),
            import_directory_rva: 0,
            import_directory_size: 0,
            iat_rva: 0,
            iat_size: 0,
        };
    }
    // MULTI-DLL import table: thunks are grouped by their exact retained raw
    // library bytes. Legacy catalog fallback is resolved before this boundary.
    // Layout, all offsets relative to rdata_rva:
    //   [descriptors: (n_dlls + 1) * 20]
    //   [ILT_dll1][ILT_dll2]...      (each (count+1) * 8)
    //   [IAT_dll1][IAT_dll2]...      (contiguous, so ONE directory entry covers all)
    //   [dll names][hint/name entries]
    // `iat_rvas` is returned in the INPUT thunk order (patch_import_thunks zips
    // it against the same slice).
    let mut libraries: Vec<(&[u8], Vec<usize>)> = Vec::new();
    for (index, import) in imports.iter().enumerate() {
        let library = import.library.as_slice();
        match libraries.iter_mut().find(|(name, _)| *name == library) {
            Some((_, members)) => members.push(index),
            None => libraries.push((library, vec![index])),
        }
    }

    let descriptor_table_size = (libraries.len() + 1) * 20;
    // Per-library ILT offsets, then the contiguous IAT region.
    let mut ilt_offsets = Vec::with_capacity(libraries.len());
    let mut cursor = descriptor_table_size;
    for (_, members) in &libraries {
        ilt_offsets.push(cursor);
        cursor += (members.len() + 1) * 8;
    }
    let iat_region_offset = cursor;
    let mut iat_offsets = Vec::with_capacity(libraries.len());
    for (_, members) in &libraries {
        iat_offsets.push(cursor);
        cursor += (members.len() + 1) * 8;
    }
    let iat_region_size = cursor - iat_region_offset;
    // DLL name strings.
    let mut dll_name_offsets = Vec::with_capacity(libraries.len());
    for (library, _) in &libraries {
        dll_name_offsets.push(cursor);
        cursor += library.len() + 1;
    }
    cursor = align_to(cursor, 2);
    // Hint/name entries, in INPUT thunk order.
    let mut hint_name_offsets = vec![None; imports.len()];
    for (index, import) in imports.iter().enumerate() {
        if let PeImportLookup::ByName(export) = &import.lookup {
            hint_name_offsets[index] = Some(cursor);
            cursor = align_to(cursor + 2 + export.len() + 1, 2);
        }
    }

    let mut bytes = vec![0; cursor];

    for (library_index, (library, members)) in libraries.iter().enumerate() {
        let descriptor_offset = library_index * 20;
        let ilt_rva = rdata_rva + ilt_offsets[library_index] as u32;
        let iat_rva = rdata_rva + iat_offsets[library_index] as u32;
        let dll_name_rva = rdata_rva + dll_name_offsets[library_index] as u32;
        write_u32_at(&mut bytes, descriptor_offset, ilt_rva);
        write_u32_at(&mut bytes, descriptor_offset + 12, dll_name_rva);
        write_u32_at(&mut bytes, descriptor_offset + 16, iat_rva);

        let name_offset = dll_name_offsets[library_index];
        bytes[name_offset..name_offset + library.len()].copy_from_slice(library);

        for (slot, import_index) in members.iter().enumerate() {
            let lookup = match (
                &imports[*import_index].lookup,
                hint_name_offsets[*import_index],
            ) {
                (PeImportLookup::ByName(_), Some(offset)) => u64::from(rdata_rva + offset as u32),
                (PeImportLookup::ByOrdinal(ordinal), None) => {
                    IMAGE_ORDINAL_FLAG64 | u64::from(*ordinal)
                }
                _ => unreachable!("PE import lookup layout must match its retained case"),
            };
            write_u64_at(&mut bytes, ilt_offsets[library_index] + slot * 8, lookup);
            write_u64_at(&mut bytes, iat_offsets[library_index] + slot * 8, lookup);
        }
    }

    // Hint/name entries + per-thunk IAT rvas in INPUT order.
    let mut iat_rvas = vec![0u32; imports.len()];
    for (library_index, (_, members)) in libraries.iter().enumerate() {
        for (slot, import_index) in members.iter().enumerate() {
            iat_rvas[*import_index] = rdata_rva + (iat_offsets[library_index] + slot * 8) as u32;
        }
    }
    for (index, import) in imports.iter().enumerate() {
        let (PeImportLookup::ByName(export), Some(name_offset)) =
            (&import.lookup, hint_name_offsets[index])
        else {
            continue;
        };
        write_u16_at(&mut bytes, name_offset, 0);
        let symbol_start = name_offset + 2;
        bytes[symbol_start..symbol_start + export.len()].copy_from_slice(export);
    }

    PeImportTable {
        bytes,
        iat_rvas,
        import_directory_rva: rdata_rva,
        import_directory_size: descriptor_table_size,
        iat_rva: rdata_rva + iat_region_offset as u32,
        iat_size: iat_region_size,
    }
}

pub(crate) fn patch_import_thunks(
    image: &mut FinalImage,
    layout: &FinalImageLayout,
    thunks: &[PeImportThunk],
    iat_rvas: &[u32],
) -> Result<(), Diagnostic> {
    for (thunk, iat_rva) in thunks.iter().zip(iat_rvas.iter().copied()) {
        let instruction_address = layout.text_address + thunk.text_offset as u64;
        let next_instruction = instruction_address + 6;
        let iat_address = IMAGE_BASE + u64::from(iat_rva);
        let delta = iat_address as i64 - next_instruction as i64;
        let displacement = i32::try_from(delta).map_err(|_| {
            Diagnostic::error(format!(
                "PE x86_64 import thunk for `{}` is out of range",
                thunk.symbol
            ))
        })?;
        write_i32_at(&mut image.memory.text, thunk.text_offset + 2, displacement)?;
    }

    Ok(())
}

/// Validate the final patched PE thunk opcode shape and attach its exact
/// architectural effect. `jmp [rip+disp32]` writes control flow but no GPR,
/// flags, stack, or vector state.
pub(crate) fn validate_import_thunk_footprints(
    image: &mut FinalImage,
    thunks: &[PeImportThunk],
) -> Result<(), Diagnostic> {
    for thunk in thunks {
        let end = thunk.text_offset.checked_add(6).ok_or_else(|| {
            Diagnostic::error(format!(
                "PE import thunk `{}` range overflows",
                thunk.symbol
            ))
        })?;
        let bytes = image
            .memory
            .text
            .get(thunk.text_offset..end)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "PE import thunk `{}` is out of final .text bounds",
                    thunk.symbol
                ))
            })?;
        if bytes[..2] != [0xff, 0x25] {
            return Err(Diagnostic::error(format!(
                "PE import thunk `{}` does not match jmp [rip+disp32]",
                thunk.symbol
            )));
        }
        let region = image
            .executable_regions
            .iter_mut()
            .find(|region| {
                region.origin == FinalExecutableRegionOrigin::ImportThunk
                    && region.section_offset == thunk.text_offset
                    && region.byte_count == 6
                    && region.symbol == thunk.symbol
            })
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "PE import thunk `{}` is missing its executable-region record",
                    thunk.symbol
                ))
            })?;
        region.footprint = Some(StateFootprintEvidence::new(
            RegisterSet::default(),
            MachineStateSet::new([MachineState::InstructionPointer]),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        PeImportThunk, build_import_table, install_import_thunks, validate_import_thunk_footprints,
    };
    use omega_image::{
        FinalExecutableRegionOrigin, FinalImage, FinalImageImport, FinalImageImportPlan,
        FinalImageSymbol,
    };
    use omega_target::{
        ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator,
    };
    use psi_arena::Handle;

    #[test]
    fn installed_import_thunks_enter_the_executable_region_inventory() {
        let mut image = FinalImage::with_capacity(
            NativeTarget::windows_x64(),
            Default::default(),
            Handle::invalid(),
            1,
            1,
            0,
        );
        let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "ExitProcess".into(),
            kind: omega_object_file::SymbolKind::Import,
            ..FinalImageSymbol::default()
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: symbol,
            import: FinalImageImportPlan::StringBackedBootstrap {
                library: "KERNEL32.dll".into(),
            },
        });

        let thunks = install_import_thunks(&mut image, 3).expect("valid bootstrap import");
        validate_import_thunk_footprints(&mut image, &thunks)
            .expect("installed PE thunk bytes should validate");

        assert_eq!(thunks.len(), 1);
        assert_eq!(image.executable_regions.len(), 1);
        assert_eq!(
            image.executable_regions[0].origin,
            FinalExecutableRegionOrigin::ImportThunk
        );
        assert_eq!(image.executable_regions[0].byte_count, 6);
        assert_eq!(image.executable_regions[0].symbol, "ExitProcess");
        assert!(image.executable_regions[0].footprint.is_some());
    }

    #[test]
    fn mutated_import_thunk_opcode_rejects_final_validation() {
        let mut image = FinalImage::with_capacity(
            NativeTarget::windows_x64(),
            Default::default(),
            Handle::invalid(),
            1,
            1,
            0,
        );
        let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "ExitProcess".into(),
            kind: omega_object_file::SymbolKind::Import,
            ..FinalImageSymbol::default()
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: symbol,
            import: FinalImageImportPlan::StringBackedBootstrap {
                library: "KERNEL32.dll".into(),
            },
        });
        let thunks = install_import_thunks(&mut image, 3).expect("valid bootstrap import");
        image.memory.text[0] = 0x90;

        let diagnostic = validate_import_thunk_footprints(&mut image, &thunks)
            .expect_err("mutated PE thunk opcode must reject");
        assert!(diagnostic.message.contains("jmp [rip+disp32]"));
    }

    #[test]
    fn no_host_target_emits_no_import_directory() {
        // A no-host / EFI target imports nothing: the directory must be absent
        // (RVA/size 0) and no import bytes emitted, so the PE header threads
        // zeros -- a clean import-free PE32+, not a lone null descriptor.
        let table = build_import_table(&[], 0x3000);
        assert!(table.bytes.is_empty());
        assert!(table.iat_rvas.is_empty());
        assert_eq!(table.import_directory_rva, 0);
        assert_eq!(table.import_directory_size, 0);
        assert_eq!(table.iat_rva, 0);
        assert_eq!(table.iat_size, 0);
    }

    #[test]
    fn hosted_target_still_builds_a_directory() {
        // Regression: with imports present the directory is non-empty and the
        // per-thunk IAT rvas are returned in input order.
        let thunks = vec![
            PeImportThunk {
                symbol: "ExitProcess".to_owned(),
                library: b"KERNEL32.dll".to_vec(),
                lookup: super::PeImportLookup::ByName(b"ExitProcess".to_vec()),
                text_offset: 0,
            },
            PeImportThunk {
                symbol: "GetStdHandle".to_owned(),
                library: b"KERNEL32.dll".to_vec(),
                lookup: super::PeImportLookup::ByName(b"GetStdHandle".to_vec()),
                text_offset: 6,
            },
        ];
        let table = build_import_table(&thunks, 0x3000);
        assert!(!table.bytes.is_empty());
        assert_eq!(table.iat_rvas.len(), 2);
        assert_eq!(table.import_directory_rva, 0x3000);
        assert!(table.import_directory_size > 0);
        assert!(table.iat_size > 0);
    }

    #[test]
    fn binding_carried_library_beats_the_catalog() {
        // A source external import names its own DLL; the catalog would
        // otherwise file `abs` under the KERNEL32 default.
        let thunks = vec![PeImportThunk {
            symbol: "abs".to_owned(),
            library: b"msvcrt.dll".to_vec(),
            lookup: super::PeImportLookup::ByName(b"abs".to_vec()),
            text_offset: 0,
        }];
        let table = build_import_table(&thunks, 0x3000);
        let bytes = &table.bytes;
        assert!(
            bytes
                .windows(b"msvcrt.dll".len())
                .any(|window| window == b"msvcrt.dll"),
            "import table should name the binding's DLL"
        );
        assert!(
            !bytes
                .windows(b"KERNEL32.dll".len())
                .any(|window| window == b"KERNEL32.dll"),
            "catalog default must not leak in when the binding names a library"
        );
    }

    fn image_with_normalized_import(
        target: NativeTarget,
        candidate: ForeignLocatorCandidate,
    ) -> FinalImage {
        let locator = normalize_foreign_locator(candidate, TargetProfile::WindowsX64)
            .expect("valid normalized PE locator");
        let mut image =
            FinalImage::with_capacity(target, Default::default(), Handle::invalid(), 1, 1, 0);
        let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: format!(
                "__omega_foreign_import_{:016x}",
                locator.non_authoritative_compatibility_fingerprint()
            ),
            kind: omega_object_file::SymbolKind::Import,
            ..FinalImageSymbol::default()
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: symbol,
            import: FinalImageImportPlan::Normalized(locator),
        });
        image
    }

    #[test]
    fn normalized_pe_name_emits_exact_non_utf8_coordinates_without_symbol_reconstruction() {
        let mut image = image_with_normalized_import(
            NativeTarget::windows_x64(),
            ForeignLocatorCandidate::PeByName {
                library: b"raw\xff.dll".to_vec(),
                export: b"entry\xfe".to_vec(),
            },
        );
        let thunks = install_import_thunks(&mut image, 3).expect("Windows PE import");
        assert_eq!(thunks.len(), 1);
        assert_eq!(thunks[0].library, b"raw\xff.dll");
        assert_eq!(
            thunks[0].lookup,
            super::PeImportLookup::ByName(b"entry\xfe".to_vec())
        );

        let table = build_import_table(&thunks, 0x3000);
        assert!(table.bytes.windows(8).any(|bytes| bytes == b"raw\xff.dll"));
        assert!(table.bytes.windows(6).any(|bytes| bytes == b"entry\xfe"));
        assert!(
            !table
                .bytes
                .windows("__omega_foreign_import_".len())
                .any(|bytes| bytes == b"__omega_foreign_import_"),
            "object-local labels must not become physical PE lookup bytes"
        );
    }

    #[test]
    fn normalized_pe_ordinal_sets_ordinal_flag_in_both_lookup_tables() {
        let mut image = image_with_normalized_import(
            NativeTarget::windows_x64(),
            ForeignLocatorCandidate::PeByOrdinal {
                library: b"ordinals.dll".to_vec(),
                ordinal: 17,
            },
        );
        let thunks = install_import_thunks(&mut image, 3).expect("Windows ordinal import");
        let table = build_import_table(&thunks, 0x3000);
        let ilt_offset =
            u32::from_le_bytes(table.bytes[0..4].try_into().unwrap()) as usize - 0x3000;
        let iat_offset =
            u32::from_le_bytes(table.bytes[16..20].try_into().unwrap()) as usize - 0x3000;
        let expected = super::IMAGE_ORDINAL_FLAG64 | 17;
        assert_eq!(
            u64::from_le_bytes(table.bytes[ilt_offset..ilt_offset + 8].try_into().unwrap()),
            expected
        );
        assert_eq!(
            u64::from_le_bytes(table.bytes[iat_offset..iat_offset + 8].try_into().unwrap()),
            expected
        );
        assert!(
            table
                .bytes
                .windows(12)
                .any(|bytes| bytes == b"ordinals.dll")
        );
    }

    #[test]
    fn locator_mutation_changes_pe_import_bytes_and_target_drift_rejects_before_installation() {
        let make_table = |export: &[u8]| {
            let mut image = image_with_normalized_import(
                NativeTarget::windows_x64(),
                ForeignLocatorCandidate::PeByName {
                    library: b"exact.dll".to_vec(),
                    export: export.to_vec(),
                },
            );
            let thunks = install_import_thunks(&mut image, 3).expect("Windows name import");
            build_import_table(&thunks, 0x3000).bytes
        };
        assert_ne!(make_table(b"entry_a"), make_table(b"entry_b"));

        let mut uefi = image_with_normalized_import(
            NativeTarget::uefi_x64(),
            ForeignLocatorCandidate::PeByOrdinal {
                library: b"forbidden.dll".to_vec(),
                ordinal: 9,
            },
        );
        let diagnostic = install_import_thunks(&mut uefi, 10)
            .expect_err("Windows imports must not leak into a UEFI PE image");
        assert!(diagnostic.message.contains("not applicable"));
        assert!(
            uefi.memory.text.is_empty(),
            "rejection must precede thunk mutation"
        );

        let mut missing = FinalImage::with_capacity(
            NativeTarget::windows_x64(),
            Default::default(),
            Handle::invalid(),
            1,
            1,
            0,
        );
        let symbol = missing.symbol_table.symbols.insert(FinalImageSymbol {
            name: "missing-plan".into(),
            kind: omega_object_file::SymbolKind::Import,
            ..FinalImageSymbol::default()
        });
        missing.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: symbol,
            import: FinalImageImportPlan::None,
        });
        let diagnostic = install_import_thunks(&mut missing, 3)
            .expect_err("an import symbol without retained coordinates must reject");
        assert!(
            diagnostic
                .message
                .contains("no retained physical import plan")
        );
        assert!(missing.memory.text.is_empty());
    }
}
