/*
We emit 64-bit PE32+ executables (`emit_pe_x86_64_executable`) and shared
component DLLs (`emit_pe_shared_component`) for Windows x86-64 from a
format-neutral `FinalImage`. PE emission maintains two alignments
simultaneously (`SECTION_ALIGNMENT = 0x1000` in memory and
`FILE_ALIGNMENT = 0x200` on disk) over `IMAGE_BASE = 0x1_4000_0000` with `.text`
anchored at RVA `0x1000` and optional `.rdata`, `.data`, `.reloc`, and `.bss`
sections emitted only when non-empty.

We execute the eight emission steps in a strict dependency order:
1. Install six-byte `jmp [rip+disp32]` (`ff 25 00 00 00 00`) import thunks at
   the end of `.text`.
2. Compute the `.rdata` RVA immediately after `.text`.
3. Build the `.rdata` import directory, lookup tables, and IAT against that RVA.
4. Plan all section RVAs, raw file offsets, and base-relocation (`.reloc`)
   blocks in a single layout pass.
5. Patch each import thunk's RIP-relative displacement to its IAT slot.
6. Apply `FinalImage` relocations across `.text` and `.data`.
7. Re-verify import thunk opcodes and state-footprint evidence after relocation.
8. Place executable, initialized-data, and `.rdata` import-slot inventories.
*/

/*
We place the import directory and IAT inside `.rdata` and reference them via
the PE optional header's data directories rather than emitting a separate
`.idata` section, matching modern Windows linker layout and allowing a single
section-planning pass. Base relocations in `.reloc` record only site offsets
(`IMAGE_REL_BASED_DIR64`), making `.reloc` independent of the relocated values
computed in step 6 and enabling `IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE` (ASLR)
whenever relocation records are present.
*/

/*
`image` supplies `FinalImage`, `apply_relocations`, and region placement;
`image-emission` invokes `emit_pe_x86_64_executable` for direct Windows
executables and `emit_pe_shared_component` for DLL components. The Windows
subsystem word (`console`, `gui`, or `efi_application`) is passed as a
parameter so all PE32+ targets share this exact container pipeline.
*/

use diagnostics::Diagnostic;
use image::{
    ExecutableImageOutput, FinalDataRegion, FinalDataRegionOrigin, FinalImage,
    PlacedDataRegionInventory, apply_relocations, place_data_extent, place_data_regions,
    place_executable_regions,
};

mod bytes {
    use diagnostics::Diagnostic;
    pub(crate) use image::codec_bytes::{write_u16, write_u32, write_u64};

    pub(crate) fn write_u16_at(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn write_u32_at(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn write_u64_at(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn write_i32_at(
        bytes: &mut [u8],
        offset: usize,
        value: i32,
    ) -> Result<(), Diagnostic> {
        let end = offset
            .checked_add(4)
            .ok_or_else(|| Diagnostic::error("PE x86_64 patch offset overflow"))?;
        let Some(slice) = bytes.get_mut(offset..end) else {
            return Err(Diagnostic::error(format!(
                "PE x86_64 patch offset {offset} is outside text section"
            )));
        };

        slice.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }
}

mod constants {
    pub(crate) const DOS_HEADER_SIZE: usize = 0x80;
    pub(crate) const COFF_HEADER_SIZE: usize = 20;
    pub(crate) const OPTIONAL_HEADER_SIZE: usize = 240;
    pub(crate) const SECTION_HEADER_SIZE: usize = 40;
    pub(crate) const IMAGE_BASE: u64 = 0x1_4000_0000;
    pub(crate) const SECTION_ALIGNMENT: usize = 0x1000;
    pub(crate) const FILE_ALIGNMENT: usize = 0x200;
    pub(crate) const TEXT_RVA: u32 = 0x1000;
}

mod entry_symbol {
    use crate::constants::TEXT_RVA;
    use diagnostics::Diagnostic;
    use image::FinalImage;

    pub(crate) fn pe_entry_rva(image: &FinalImage) -> Result<u32, Diagnostic> {
        Ok(TEXT_RVA + image::final_image_entry_symbol(image, "PE")?.offset as u32)
    }

    #[cfg(test)]
    mod tests {
        use super::pe_entry_rva;
        use crate::constants::TEXT_RVA;
        use arena::Handle;
        use image::{FinalImage, FinalImageSection, FinalImageSymbol};

        #[test]
        fn resolves_pe_entry_rva_from_final_image_entry_symbol() {
            let mut image = FinalImage::with_capacity(
                FinalImage::default().target,
                image::FinalImageMemory {
                    text: vec![0; 16],
                    ..Default::default()
                },
                Handle::invalid(),
                0,
                0,
                0,
            );
            let entry_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: "_start".into(),
                section: FinalImageSection::Text,
                offset: 6,
                size: 4,
                ..FinalImageSymbol::default()
            });
            image.symbol_table.entry_symbol = entry_symbol;

            assert_eq!(pe_entry_rva(&image).unwrap(), TEXT_RVA + 6);
        }
    }
}

mod headers;
mod imports;
mod layout {
    pub(crate) use image::align_to;

    pub(crate) fn align_to_u32(value: u32, alignment: usize) -> u32 {
        let alignment = u32::try_from(alignment.max(1)).expect("alignment overflow");
        value.div_ceil(alignment) * alignment
    }
}

mod sections;
mod shared_component;

pub use shared_component::{
    PeComponentExport, PeSharedComponentEmissionError, PeSharedComponentImage,
    emit_pe_shared_component,
};

use constants::{IMAGE_BASE, TEXT_RVA};
use entry_symbol::pe_entry_rva;
use headers::{PeHeaderInput, write_dos_header, write_pe_headers, write_section_header};
use imports::{
    build_import_table, install_import_thunks, patch_import_thunks,
    validate_import_thunk_footprints,
};
use layout::align_to;
use sections::{plan_pe_sections, rdata_rva_after_text};

pub(crate) enum PeImageKind {
    Executable,
    SharedComponent { exports: Vec<PeComponentExport> },
}

pub fn emit_pe_x86_64_executable(
    image: FinalImage,
    subsystem: u16,
) -> Result<ExecutableImageOutput, Diagnostic> {
    emit_pe_x86_64(image, subsystem, PeImageKind::Executable)
}

pub(crate) fn emit_pe_x86_64(
    mut image: FinalImage,
    subsystem: u16,
    kind: PeImageKind,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let import_thunks = install_import_thunks(&mut image, subsystem)?;
    let rdata_rva = rdata_rva_after_text(&image);
    let import_table = build_import_table(&import_thunks, rdata_rva);
    let (export_bytes, export_directory_rva, export_directory_size) = match &kind {
        PeImageKind::Executable => (Vec::new(), 0, 0),
        PeImageKind::SharedComponent { exports } => {
            let aligned = align_to(import_table.bytes.len(), 4);
            let directory_rva = shared_component::directory_rva_of(rdata_rva, aligned)?;
            let bytes = shared_component::export_directory_bytes(&image, exports, directory_rva)?;
            let size = bytes.len();
            (bytes, directory_rva, size)
        }
    };
    let mut rdata_bytes = import_table.bytes.clone();
    if !export_bytes.is_empty() {
        rdata_bytes.resize(align_to(rdata_bytes.len(), 4), 0);
        rdata_bytes.extend_from_slice(&export_bytes);
    }
    let sections = plan_pe_sections(&image, rdata_bytes.len());
    let layout = sections.final_image_layout();

    patch_import_thunks(&mut image, &layout, &import_thunks, &import_table)?;
    apply_relocations(&mut image, &layout, "PE direct executable")?;
    validate_import_thunk_footprints(&mut image, &import_thunks)?;
    let executable_regions = place_executable_regions(&image, layout)?;
    let data_regions = place_data_regions(&image, layout)?;

    let import_data_regions = if import_table.bytes.is_empty() {
        PlacedDataRegionInventory::empty()
    } else {
        let slots = import_thunks
            .iter()
            .zip(import_table.iat_table_offsets.iter().copied())
            .map(|(thunk, table_offset)| FinalDataRegion {
                origin: FinalDataRegionOrigin::ImportBindingSlot,
                section_offset: table_offset,
                byte_count: 8,
                symbol: thunk.symbol.clone(),
            })
            .collect();
        place_data_extent(
            &import_table.bytes,
            slots,
            IMAGE_BASE + u64::from(sections.rdata_rva),
            "import data",
        )?
    };

    let entry_rva = match &kind {
        PeImageKind::Executable => pe_entry_rva(&image)?,
        PeImageKind::SharedComponent { .. } => 0,
    };
    let file_characteristics = match &kind {
        PeImageKind::Executable => 0x0022,
        PeImageKind::SharedComponent { .. } => 0x2022,
    };

    let (reloc_directory_rva, reloc_directory_size) = if sections.has_reloc {
        (sections.reloc_rva, sections.reloc.bytes.len())
    } else {
        (0, 0)
    };

    let mut bytes = Vec::new();
    write_dos_header(&mut bytes);
    write_pe_headers(
        &mut bytes,
        PeHeaderInput {
            section_count: sections.section_count,
            entry_rva,
            size_of_code: sections.text_raw_size,
            size_of_initialized_data: sections.rdata_raw_size + sections.data_raw_size,
            size_of_image: sections.size_of_image,
            size_of_headers: sections.headers_size,
            import_directory_rva: import_table.import_directory_rva,
            import_directory_size: import_table.import_directory_size,
            iat_rva: import_table.iat_rva,
            iat_size: import_table.iat_size,
            reloc_directory_rva,
            reloc_directory_size,
            has_reloc: sections.has_reloc,
            subsystem,
            file_characteristics,
            export_directory_rva,
            export_directory_size,
        },
    );
    write_section_header(
        &mut bytes,
        ".text",
        sections.text_virtual_size,
        TEXT_RVA,
        sections.text_raw_size,
        sections.text_raw,
        0x6000_0020,
    );
    if sections.has_rdata {
        write_section_header(
            &mut bytes,
            ".rdata",
            sections.rdata_virtual_size,
            sections.rdata_rva,
            sections.rdata_raw_size,
            sections.rdata_raw,
            0x4000_0040,
        );
    }
    if sections.has_data {
        write_section_header(
            &mut bytes,
            ".data",
            image.memory.data.len(),
            sections.data_rva,
            sections.data_raw_size,
            sections.data_raw,
            0xc000_0040,
        );
    }
    if sections.has_reloc {
        write_section_header(
            &mut bytes,
            ".reloc",
            sections.reloc.bytes.len(),
            sections.reloc_rva,
            sections.reloc_raw_size,
            sections.reloc_raw,
            0x4200_0040,
        );
    }
    if sections.has_bss {
        write_section_header(
            &mut bytes,
            ".bss",
            image.memory.bss_size,
            sections.bss_rva,
            0,
            0,
            0xc000_0080,
        );
    }

    bytes.resize(sections.text_raw, 0);
    bytes.extend(&image.memory.text);
    bytes.resize(sections.text_raw + sections.text_raw_size, 0);
    if sections.has_rdata {
        bytes.resize(sections.rdata_raw, 0);
        bytes.extend(&rdata_bytes);
        bytes.resize(sections.rdata_raw + sections.rdata_raw_size, 0);
    }
    if sections.has_data {
        bytes.resize(sections.data_raw, 0);
        bytes.extend(&image.memory.data);
        bytes.resize(sections.data_raw + sections.data_raw_size, 0);
    }
    if sections.has_reloc {
        bytes.resize(sections.reloc_raw, 0);
        bytes.extend(&sections.reloc.bytes);
        bytes.resize(sections.reloc_raw + sections.reloc_raw_size, 0);
    }

    Ok(ExecutableImageOutput {
        final_image_layout: layout,
        final_text_bytes: image.memory.text.clone(),
        final_data_bytes: image.memory.data.clone(),
        final_import_data_bytes: import_table.bytes,
        bytes,
        file_name: "omega-program.exe".to_owned(),
        format: "pe64-x86_64-executable".to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
        data_regions,
        import_data_regions,
    })
}

#[cfg(test)]
mod tests {
    use crate::constants::{IMAGE_BASE, TEXT_RVA};
    use crate::emit_pe_x86_64_executable;
    use arena::Handle;
    use image::{
        FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSection, FinalImageSymbol,
    };
    use object_file::{RelocationKind, SymbolKind};
    use target::NativeTarget;

    fn returning_image() -> FinalImage {
        let mut image = FinalImage::with_capacity(
            NativeTarget::windows_x64(),
            FinalImageMemory {
                text: vec![0xb8, 42, 0, 0, 0, 0xc3],
                ..Default::default()
            },
            Handle::invalid(),
            1,
            0,
            0,
        );
        image.symbol_table.entry_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "entry".into(),
            section: FinalImageSection::Text,
            size: image.memory.text.len(),
            ..Default::default()
        });
        image
    }

    #[test]
    fn import_free_image_omits_empty_read_only_section() {
        let output = emit_pe_x86_64_executable(returning_image(), 3).expect("emit image");
        let bytes = &output.bytes;
        let pe_offset = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
        let section_count =
            u16::from_le_bytes(bytes[pe_offset + 6..pe_offset + 8].try_into().unwrap());
        assert_eq!(section_count, 1, "only .text has storage to publish");
        let optional_size =
            u16::from_le_bytes(bytes[pe_offset + 20..pe_offset + 22].try_into().unwrap()) as usize;
        let sections_offset = pe_offset + 24 + optional_size;
        assert_eq!(&bytes[sections_offset..sections_offset + 8], b".text\0\0\0");
        assert_eq!(bytes.len(), 0x400);
    }

    #[test]
    fn efi_application_image_is_an_import_free_subsystem_10_pe32_plus() {
        let output = emit_pe_x86_64_executable(returning_image(), 10).expect("emit image");
        let bytes = &output.bytes;
        let pe_offset = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
        let optional_header = pe_offset + 4 + 20;
        let word =
            |offset: usize| u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
        assert_eq!(word(optional_header), 0x20b, "PE32+ optional header magic");
        assert_eq!(word(optional_header + 68), 10, "EFI_APPLICATION subsystem");
        let directory = |index: usize| {
            let offset = optional_header + 112 + index * 8;
            (
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
                u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()),
            )
        };
        assert_eq!(directory(1), (0, 0), "no import directory");
        assert_eq!(directory(12), (0, 0), "no import address table");
    }

    #[test]
    fn optional_sections_publish_only_their_owned_ranges() {
        for has_imports in [false, true] {
            for has_data in [false, true] {
                for has_bss in [false, true] {
                    let mut image = returning_image();
                    let mut names: Vec<&[u8; 8]> = vec![b".text\0\0\0"];
                    if has_imports {
                        let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
                            name: "ExitProcess".into(),
                            kind: SymbolKind::Import,
                            ..Default::default()
                        });
                        image.symbol_table.imports.insert(FinalImageImport {
                            symbol_handle,
                            import: FinalImageImportPlan::StringBackedBootstrap {
                                library: "KERNEL32.dll".into(),
                            },
                        });
                        names.push(b".rdata\0\0");
                    }
                    if has_data {
                        image.memory.data = vec![7; 5];
                        names.push(b".data\0\0\0");
                    }
                    if has_bss {
                        image.memory.bss_size = 13;
                        names.push(b".bss\0\0\0\0");
                    }
                    let output = emit_pe_x86_64_executable(image, 3).expect("emit image");
                    let bytes = &output.bytes;
                    let read_u32 = |offset| {
                        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
                    };
                    let pe_offset = read_u32(0x3c);
                    let section_count =
                        u16::from_le_bytes(bytes[pe_offset + 6..pe_offset + 8].try_into().unwrap());
                    assert_eq!(usize::from(section_count), names.len());
                    let optional_size = u16::from_le_bytes(
                        bytes[pe_offset + 20..pe_offset + 22].try_into().unwrap(),
                    ) as usize;
                    let size_of_image = read_u32(pe_offset + 24 + 56);
                    let mut virtual_end = 0;
                    for (section_index, name) in names.iter().enumerate() {
                        let header = pe_offset + 24 + optional_size + 40 * section_index;
                        assert_eq!(&bytes[header..header + 8], *name);
                        let virtual_size = read_u32(header + 8);
                        let virtual_address = read_u32(header + 12);
                        assert!(virtual_size > 0);
                        assert!(virtual_address >= virtual_end);
                        virtual_end = virtual_address + virtual_size;
                        assert!(virtual_end <= size_of_image);
                        let raw_size = read_u32(header + 16);
                        let raw_offset = read_u32(header + 20);
                        assert!(raw_offset + raw_size <= bytes.len());
                    }
                }
            }
        }
    }

    #[test]
    fn import_free_image_runs_on_windows() {
        if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            eprintln!("SKIP: requires an x86-64 Windows loader");
            return;
        }
        let output = emit_pe_x86_64_executable(returning_image(), 3).expect("emit image");
        let path = std::env::temp_dir().join(format!("omega-pe-return-{}.exe", std::process::id()));
        std::fs::write(&path, &output.bytes).expect("write executable");
        let result = std::process::Command::new(&path).output();
        std::fs::remove_file(&path).expect("remove executable");
        let result = result.expect("Windows must load an import-free PE");
        assert_eq!(result.status.code(), Some(42), "{result:?}");
    }

    #[test]
    fn emitted_reloc_section_matches_the_planned_data_rva() {
        for (text_padding, expected_data_rva) in [(0usize, 0x2000usize), (0x1200, 0x3000)] {
            let mut image = returning_image();
            image.memory.text.resize(6 + text_padding, 0x90);
            image.memory.data = vec![0; 8];
            let entry = image.symbol_table.entry_symbol;
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Data,
                    offset: 0,
                    byte_width: 8,
                    symbol_handle: entry,
                    addend: 0,
                    kind: RelocationKind::Absolute64,
                });

            let output = emit_pe_x86_64_executable(image, 3).expect("emit image");
            let bytes = &output.bytes;
            let u32_at = |offset: usize| {
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
            };
            let pe_offset = u32_at(0x3c);
            let section_count =
                u16::from_le_bytes(bytes[pe_offset + 6..pe_offset + 8].try_into().unwrap());
            assert_eq!(section_count, 3, "text, data and the carried reloc table");
            let optional_size =
                u16::from_le_bytes(bytes[pe_offset + 20..pe_offset + 22].try_into().unwrap())
                    as usize;
            let section = |index: usize| {
                let header = pe_offset + 24 + optional_size + 40 * index;
                (
                    &bytes[header..header + 8],
                    u32_at(header + 8),
                    u32_at(header + 12),
                    u32_at(header + 20),
                )
            };
            assert_eq!(section(0).0, b".text\0\0\0");
            assert_eq!(section(1).0, b".data\0\0\0");
            assert_eq!(section(2).0, b".reloc\0\0");
            let data_rva = section(1).2;
            assert_eq!(data_rva, expected_data_rva, "the padded .text moves .data");

            let (_, reloc_virtual_size, reloc_rva, reloc_raw) = section(2);
            assert_eq!(reloc_virtual_size, 12);
            let directory = pe_offset + 24 + 112 + 5 * 8;
            assert_eq!(
                (u32_at(directory), u32_at(directory + 4)),
                (reloc_rva, reloc_virtual_size)
            );
            assert_eq!(reloc_rva, data_rva + 0x1000);
            assert_eq!(u32_at(reloc_raw), data_rva, "page RVA is the planned RVA");
            assert_eq!(u32_at(reloc_raw + 4), 12);
            assert_eq!(
                u16::from_le_bytes(bytes[reloc_raw + 8..reloc_raw + 10].try_into().unwrap()),
                0xa000 | (data_rva as u16 & 0xFFF)
            );

            assert_eq!(
                output.final_data_bytes,
                (IMAGE_BASE + u64::from(TEXT_RVA)).to_le_bytes()
            );
        }
    }
}
