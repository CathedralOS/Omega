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
            // mov eax, 42; ret. No imports, data, or relocations.
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
    let section_count = u16::from_le_bytes(bytes[pe_offset + 6..pe_offset + 8].try_into().unwrap());
    assert_eq!(section_count, 1, "only .text has storage to publish");
    let optional_size =
        u16::from_le_bytes(bytes[pe_offset + 20..pe_offset + 22].try_into().unwrap()) as usize;
    let sections_offset = pe_offset + 24 + optional_size;
    assert_eq!(&bytes[sections_offset..sections_offset + 8], b".text\0\0\0");
    assert_eq!(bytes.len(), 0x400);
}

#[test]
fn efi_application_image_is_an_import_free_subsystem_10_pe32_plus() {
    // `Subsystem::EfiApplication` evaluates to PE subsystem word 10. A UEFI
    // image receives its services through the firmware entry, never imports:
    // the header must carry PE32+ magic, subsystem 10, and empty import and
    // import-address-table directories.
    let output = emit_pe_x86_64_executable(returning_image(), 10).expect("emit image");
    let bytes = &output.bytes;
    let pe_offset = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
    let optional_header = pe_offset + 4 + 20;
    let word = |offset: usize| u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
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
                let optional_size =
                    u16::from_le_bytes(bytes[pe_offset + 20..pe_offset + 22].try_into().unwrap())
                        as usize;
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

/// A `.data` Absolute64 site gives the image a `.reloc` section whose bytes
/// must be the ones the single layout pass built while sizing that section:
/// the block's page RVA is the `.data` RVA the pass used, and the section
/// header and directory describe exactly those bytes. The padded variation
/// moves `.text` across a section boundary, so `.data` and the page entry
/// move together; bytes rebuilt against any other base show up as a page RVA
/// that disagrees with the section table.
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
            u16::from_le_bytes(bytes[pe_offset + 20..pe_offset + 22].try_into().unwrap()) as usize;
        // Each 40-byte section header is name, virtual size, RVA, raw size,
        // raw offset; emitted in file order.
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

        // One site pads to a 12-byte block: page RVA, block size, one DIR64
        // word (0xa000 | page offset), and a zero ABSOLUTE pad word.
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

        // The site itself was relocated to the entry address after patching.
        assert_eq!(
            output.final_data_bytes,
            (IMAGE_BASE + u64::from(TEXT_RVA)).to_le_bytes()
        );
    }
}
