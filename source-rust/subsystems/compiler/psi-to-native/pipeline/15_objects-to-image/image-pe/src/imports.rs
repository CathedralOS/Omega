/*
We install each import's encoded thunk onto `.text` (x86-64 `jmp [rip+disp32]`,
AArch64 `adrp x16; ldr x16, [x16]; br x16`), build the deterministic `.rdata`
import directory, import lookup tables, hint/name table, and IAT for both
by-name and by-ordinal imports, point each thunk at its IAT slot once layout is
fixed, and re-verify that every thunk kept its canonical form through general
relocation patching.
*/

use crate::bytes::{write_u16_at, write_u32_at, write_u64_at};
use crate::constants::IMAGE_BASE;
use diagnostics::Diagnostic;
use image::align_to;
use image::{FinalImage, FinalImageImportPlan, FinalImageLayout, FinalImageSection};
use object_file::SymbolKind;
use target::{Architecture, ForeignLocatorCandidate, NativeTarget, ObjectFormat};

const IMAGE_ORDINAL_FLAG64: u64 = 1 << 63;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PeImportLookup {
    ByName(Vec<u8>),
    ByOrdinal(u16),
}

/// The instruction set a PE32+ image carries, which fixes its COFF machine
/// word and the shape of its import thunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PeIsa {
    X86_64,
    Aarch64,
}

impl PeIsa {
    pub(crate) fn of(target: NativeTarget) -> Result<Self, Diagnostic> {
        match (target.object_format, target.architecture) {
            (ObjectFormat::Coff, Architecture::X86_64) => Ok(Self::X86_64),
            (ObjectFormat::Coff, Architecture::Aarch64) => Ok(Self::Aarch64),
            _ => Err(Diagnostic::error(
                "PE emission received a non-COFF image target",
            )),
        }
    }

    /// `IMAGE_FILE_MACHINE_AMD64` or `IMAGE_FILE_MACHINE_ARM64`.
    pub(crate) const fn machine(self) -> u16 {
        match self {
            Self::X86_64 => 0x8664,
            Self::Aarch64 => 0xaa64,
        }
    }

    pub(crate) const fn format_name(self) -> &'static str {
        match self {
            Self::X86_64 => "pe64-x86_64-executable",
            Self::Aarch64 => "pe64-arm64-executable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PeImportThunk {
    pub(crate) symbol: String,
    pub(crate) library: Vec<u8>,
    pub(crate) lookup: PeImportLookup,
    pub(crate) text_offset: usize,
}

pub(crate) fn install_import_thunks(
    image: &mut FinalImage,
    subsystem: u16,
) -> Result<Vec<PeImportThunk>, Diagnostic> {
    PeIsa::of(image.target)?;
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
            FinalImageImportPlan::Normalized(target::NormalizedImportLocator::Foreign(locator)) => {
                if !locator.target().imports_through_pe()
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
                    ForeignLocatorCandidate::MachODylibSymbol { .. } => {
                        return Err(Diagnostic::error(format!(
                            "Mach-O foreign locator 0x{:016x} cannot be emitted in a PE import table",
                            locator.non_authoritative_compatibility_fingerprint(),
                        )));
                    }
                }
            }
            FinalImageImportPlan::Normalized(target::NormalizedImportLocator::Component(
                component,
            )) => {
                if !component.target().imports_through_pe()
                    || component.target().native_target() != image.target
                {
                    return Err(Diagnostic::error(
                        "component locator is not applicable to this PE image target",
                    ));
                }
                (
                    component.image_filename().into_bytes(),
                    PeImportLookup::ByName(component.export_symbol().into_bytes()),
                )
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
        let text_offset = image.install_slot_thunk("PE", symbol_handle)?;

        thunks.push(PeImportThunk {
            symbol,
            library,
            lookup,
            text_offset,
        });
    }

    image.validate_import_program()?;
    Ok(thunks)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PeImportTable {
    pub(crate) bytes: Vec<u8>,
    pub(crate) iat_table_offsets: Vec<usize>,
    pub(crate) import_directory_rva: u32,
    pub(crate) import_directory_size: usize,
    pub(crate) iat_rva: u32,
    pub(crate) iat_size: usize,
}

pub(crate) fn build_import_table(imports: &[PeImportThunk], rdata_rva: u32) -> PeImportTable {
    if imports.is_empty() {
        return PeImportTable {
            bytes: Vec::new(),
            iat_table_offsets: Vec::new(),
            import_directory_rva: 0,
            import_directory_size: 0,
            iat_rva: 0,
            iat_size: 0,
        };
    }
    let mut libraries: Vec<(&[u8], Vec<usize>)> = Vec::new();
    for (index, import) in imports.iter().enumerate() {
        let library = import.library.as_slice();
        match libraries.iter_mut().find(|(name, _)| *name == library) {
            Some((_, members)) => members.push(index),
            None => libraries.push((library, vec![index])),
        }
    }

    let descriptor_table_size = (libraries.len() + 1) * 20;
    let mut ilt_offsets = Vec::with_capacity(libraries.len());
    // PE32+ lookup and address tables hold 8-byte entries. Twenty-byte
    // descriptors leave an odd library count 4 bytes short, and an AArch64
    // thunk can only load an 8-byte-aligned IAT slot.
    let mut cursor = align_to(descriptor_table_size, 8);
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
    let mut dll_name_offsets = Vec::with_capacity(libraries.len());
    for (library, _) in &libraries {
        dll_name_offsets.push(cursor);
        cursor += library.len() + 1;
    }
    cursor = align_to(cursor, 2);
    let mut hint_name_offsets = vec![None; imports.len()];
    for (index, import) in imports.iter().enumerate() {
        if let PeImportLookup::ByName(export) = &import.lookup {
            hint_name_offsets[index] = Some(cursor);
            cursor = align_to(cursor + 2 + export.len() + 1, 2);
        }
    }

    let mut bytes = vec![0; cursor];

    let mut iat_table_offsets = vec![0usize; imports.len()];
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
            let slot_offset = iat_offsets[library_index] + slot * 8;
            iat_table_offsets[*import_index] = slot_offset;
            write_u64_at(&mut bytes, ilt_offsets[library_index] + slot * 8, lookup);
            write_u64_at(&mut bytes, slot_offset, lookup);
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
        iat_table_offsets,
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
    table: &PeImportTable,
) -> Result<(), Diagnostic> {
    for (thunk, &slot_offset) in thunks.iter().zip(table.iat_table_offsets.iter()) {
        let instruction_address = layout.text_address + thunk.text_offset as u64;
        let iat_address = IMAGE_BASE + u64::from(table.import_directory_rva) + slot_offset as u64;
        image.patch_slot_thunk(
            "PE",
            &thunk.symbol,
            thunk.text_offset,
            instruction_address,
            iat_address,
        )?;
    }
    Ok(())
}

pub(crate) fn validate_import_thunk_footprints(
    image: &FinalImage,
    thunks: &[PeImportThunk],
) -> Result<(), Diagnostic> {
    for thunk in thunks {
        image.validate_slot_thunk("PE", &thunk.symbol, thunk.text_offset)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        PeImportThunk, build_import_table, install_import_thunks, validate_import_thunk_footprints,
    };
    use arena::Handle;
    use image::{
        FinalExecutableRegionOrigin, FinalImage, FinalImageImport, FinalImageImportPlan,
        FinalImageSymbol,
    };
    use target::{ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator};

    fn image_with_import(name: &str, import: FinalImageImportPlan) -> FinalImage {
        let mut image = FinalImage::with_capacity(
            NativeTarget::windows_x64(),
            Default::default(),
            Handle::invalid(),
            1,
            1,
            0,
        );
        let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: name.into(),
            kind: object_file::SymbolKind::Import,
            ..FinalImageSymbol::default()
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: symbol,
            import,
        });
        image::test_support::install_imports(&mut image);
        image
    }

    fn kernel32_locator(export: &str) -> target::NormalizedImportLocator {
        normalize_foreign_locator(
            ForeignLocatorCandidate::PeByName {
                library: b"KERNEL32.dll".to_vec(),
                export: export.as_bytes().to_vec(),
            },
            TargetProfile::WindowsX64,
        )
        .expect("valid KERNEL32 locator")
        .into()
    }

    fn kernel32_import_image(export: &str) -> FinalImage {
        image_with_import(
            export,
            FinalImageImportPlan::Normalized(kernel32_locator(export)),
        )
    }

    #[test]
    fn installed_import_thunks_enter_the_executable_region_inventory() {
        let mut image = kernel32_import_image("ExitProcess");

        let thunks = install_import_thunks(&mut image, 3).expect("valid KERNEL32 import");
        validate_import_thunk_footprints(&image, &thunks)
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
    fn image_writer_cannot_generate_or_substitute_an_import_body() {
        let complete = kernel32_import_image("ExitProcess");
        let mut missing = complete.clone();
        missing.import_program.imports.clear();
        let error = install_import_thunks(&mut missing, 3).unwrap_err();
        assert!(error.message.contains("previously encoded body"));
        assert!(missing.memory.text.is_empty());

        let mut substituted = complete.clone();
        substituted.import_program.imports[0].identity = kernel32_locator("GetLastError");
        assert!(
            install_import_thunks(&mut substituted, 3)
                .unwrap_err()
                .message
                .contains("previously encoded body")
        );

        let mut changed_effects = complete;
        changed_effects.import_program.imports[0]
            .body
            .resources
            .effects = calling_conventions::StateEffectsEvidence::empty();
        let thunks = install_import_thunks(&mut changed_effects, 3).unwrap();
        assert!(
            validate_import_thunk_footprints(&changed_effects, &thunks)
                .unwrap_err()
                .message
                .contains("effects differ")
        );
    }

    #[test]
    fn mutated_import_thunk_opcode_rejects_final_validation() {
        let mut image = kernel32_import_image("ExitProcess");
        let thunks = install_import_thunks(&mut image, 3).expect("valid KERNEL32 import");
        image.memory.text[0] = 0x90;

        let diagnostic = validate_import_thunk_footprints(&image, &thunks)
            .expect_err("mutated PE thunk opcode must reject");
        assert!(diagnostic.message.contains("canonical slot branch"));
    }

    #[test]
    fn arm64_import_thunk_loads_its_iat_slot_through_x16() {
        let mut image = image_with_normalized_import(
            NativeTarget::windows_arm64(),
            ForeignLocatorCandidate::PeByName {
                library: b"kernel32.dll".to_vec(),
                export: b"Sleep".to_vec(),
            },
        );
        let thunks = install_import_thunks(&mut image, 3).expect("Windows ARM64 name import");
        assert_eq!(image.executable_regions[0].byte_count, 12);
        let table = build_import_table(&thunks, 0x3000);
        let layout = image::FinalImageLayout {
            text_address: crate::constants::IMAGE_BASE + 0x1000,
            data_address: 0,
            bss_address: 0,
        };
        super::patch_import_thunks(&mut image, &layout, &thunks, &table).expect("in range");
        validate_import_thunk_footprints(&image, &thunks).expect("canonical ARM64 thunk");
        let word = |index: usize| {
            u32::from_le_bytes(
                image.memory.text[index * 4..index * 4 + 4]
                    .try_into()
                    .unwrap(),
            )
        };
        // The IAT slot sits in .rdata at RVA 0x3000 + its table offset: two
        // pages after the thunk's page.
        let slot = 0x3000 + table.iat_table_offsets[0] as u32;
        assert_eq!(slot & 0xfff, slot - 0x3000);
        assert_eq!(word(0), 0xd000_0010, "adrp x16, +2 pages");
        assert_eq!(word(1), 0xf940_0210 | (((slot & 0xfff) / 8) << 10));
        assert_eq!(word(2), 0xd61f_0200, "br x16");

        image.memory.text[8] = 0xc0;
        assert!(
            validate_import_thunk_footprints(&image, &thunks)
                .unwrap_err()
                .message
                .contains("canonical slot branch")
        );
    }

    #[test]
    fn lookup_and_address_tables_start_on_eight_byte_boundaries() {
        for libraries in 1..=3usize {
            let thunks = (0..libraries)
                .map(|index| PeImportThunk {
                    symbol: format!("import{index}"),
                    library: format!("library{index}.dll").into_bytes(),
                    lookup: super::PeImportLookup::ByOrdinal(1),
                    text_offset: index * 12,
                })
                .collect::<Vec<_>>();
            let table = build_import_table(&thunks, 0x3000);
            assert!(table.iat_table_offsets.iter().all(|offset| offset % 8 == 0));
            assert_eq!(table.iat_rva % 8, 0);
        }
    }

    #[test]
    fn windows_imports_stay_on_their_own_architecture() {
        let mut crossed = image_with_normalized_import(
            NativeTarget::windows_x64(),
            ForeignLocatorCandidate::PeByName {
                library: b"kernel32.dll".to_vec(),
                export: b"Sleep".to_vec(),
            },
        );
        crossed.target = NativeTarget::windows_arm64();
        assert!(
            install_import_thunks(&mut crossed, 3)
                .unwrap_err()
                .message
                .contains("not applicable")
        );
    }

    #[test]
    fn no_host_target_emits_no_import_directory() {
        let table = build_import_table(&[], 0x3000);
        assert!(table.bytes.is_empty());
        assert!(table.iat_table_offsets.is_empty());
        assert_eq!(table.import_directory_rva, 0);
        assert_eq!(table.import_directory_size, 0);
        assert_eq!(table.iat_rva, 0);
        assert_eq!(table.iat_size, 0);
    }

    #[test]
    fn hosted_target_still_builds_a_directory() {
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
        assert_eq!(table.iat_table_offsets.len(), 2);
        assert_eq!(table.import_directory_rva, 0x3000);
        assert!(table.import_directory_size > 0);
        assert!(table.iat_size > 0);
    }

    #[test]
    fn binding_carried_library_beats_the_catalog() {
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
        let profile = if target == NativeTarget::windows_arm64() {
            TargetProfile::WindowsArm64
        } else {
            TargetProfile::WindowsX64
        };
        let locator =
            normalize_foreign_locator(candidate, profile).expect("valid normalized PE locator");
        let mut image =
            FinalImage::with_capacity(target, Default::default(), Handle::invalid(), 1, 1, 0);
        let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: format!(
                "__omega_foreign_import_{:016x}",
                locator.non_authoritative_compatibility_fingerprint()
            ),
            kind: object_file::SymbolKind::Import,
            ..FinalImageSymbol::default()
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: symbol,
            import: FinalImageImportPlan::Normalized((locator).into()),
        });
        image::test_support::install_imports(&mut image);
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

        let mut missing = image_with_import("missing-plan", FinalImageImportPlan::None);
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
