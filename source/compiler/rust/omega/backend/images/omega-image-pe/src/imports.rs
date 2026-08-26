use crate::bytes::{write_i32_at, write_u16_at, write_u32_at, write_u64_at};
use crate::constants::IMAGE_BASE;
use crate::layout::align_to;
use omega_calling_conventions::{
    MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};
use omega_image::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalImage, FinalImageLayout,
    FinalImageSection,
};
use psi_diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PeImportThunk {
    pub(crate) symbol: String,
    /// Library named by the external binding. Empty requests the compiler's
    /// target import catalog for legacy built-in imports; no authored provider
    /// row or ambient library guess is reconstructed here.
    pub(crate) library: String,
    pub(crate) text_offset: usize,
}

pub(crate) fn install_import_thunks(image: &mut FinalImage) -> Vec<PeImportThunk> {
    let imports = image
        .symbol_table
        .imports
        .iter()
        .filter_map(|(_, import)| {
            image
                .symbol_table
                .symbols
                .is_valid(import.symbol_handle)
                .then_some((import.symbol_handle, import.library.clone()))
        })
        .collect::<Vec<_>>();
    let mut thunks = Vec::new();

    for (symbol_handle, library) in imports {
        let symbol = image.symbol_table.symbols.get(symbol_handle).name.clone();
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
            text_offset,
        });
    }

    thunks
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
    // MULTI-DLL import table: thunks are grouped by their catalog library
    // (default KERNEL32.dll -- the historical single-DLL behavior for symbols
    // outside the Windows catalog). Layout, all offsets relative to rdata_rva:
    //   [descriptors: (n_dlls + 1) * 20]
    //   [ILT_dll1][ILT_dll2]...      (each (count+1) * 8)
    //   [IAT_dll1][IAT_dll2]...      (contiguous, so ONE directory entry covers all)
    //   [dll names][hint/name entries]
    // `iat_rvas` is returned in the INPUT thunk order (patch_import_thunks zips
    // it against the same slice).
    let mut libraries: Vec<(&str, Vec<usize>)> = Vec::new();
    for (index, import) in imports.iter().enumerate() {
        // The binding's own library wins; the per-target catalog is the
        // fallback for thunks written without one (default KERNEL32.dll --
        // the historical single-DLL behavior for symbols outside the catalog).
        let library = if import.library.is_empty() {
            omega_calling_conventions::windows_import_library(&import.symbol)
                .unwrap_or("KERNEL32.dll")
        } else {
            import.library.as_str()
        };
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
    let mut hint_name_offsets = vec![0usize; imports.len()];
    for (index, import) in imports.iter().enumerate() {
        hint_name_offsets[index] = cursor;
        cursor = align_to(cursor + 2 + import.symbol.len() + 1, 2);
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
        bytes[name_offset..name_offset + library.len()].copy_from_slice(library.as_bytes());

        for (slot, import_index) in members.iter().enumerate() {
            let hint_name_rva = rdata_rva + hint_name_offsets[*import_index] as u32;
            write_u64_at(
                &mut bytes,
                ilt_offsets[library_index] + slot * 8,
                u64::from(hint_name_rva),
            );
            write_u64_at(
                &mut bytes,
                iat_offsets[library_index] + slot * 8,
                u64::from(hint_name_rva),
            );
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
        let name_offset = hint_name_offsets[index];
        write_u16_at(&mut bytes, name_offset, 0);
        let symbol_start = name_offset + 2;
        bytes[symbol_start..symbol_start + import.symbol.len()]
            .copy_from_slice(import.symbol.as_bytes());
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
        FinalExecutableRegionOrigin, FinalImage, FinalImageImport, FinalImageSymbol,
    };
    use psi_arena::Handle;

    #[test]
    fn installed_import_thunks_enter_the_executable_region_inventory() {
        let mut image = FinalImage::with_capacity(
            FinalImage::default().target,
            Default::default(),
            Handle::invalid(),
            1,
            1,
            0,
        );
        let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "ExitProcess".into(),
            ..FinalImageSymbol::default()
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: symbol,
            library: "KERNEL32.dll".into(),
        });

        let thunks = install_import_thunks(&mut image);
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
            FinalImage::default().target,
            Default::default(),
            Handle::invalid(),
            1,
            1,
            0,
        );
        let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "ExitProcess".into(),
            ..FinalImageSymbol::default()
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: symbol,
            library: "KERNEL32.dll".into(),
        });
        let thunks = install_import_thunks(&mut image);
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
                library: String::new(),
                text_offset: 0,
            },
            PeImportThunk {
                symbol: "GetStdHandle".to_owned(),
                library: String::new(),
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
            library: "msvcrt.dll".to_owned(),
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
}
