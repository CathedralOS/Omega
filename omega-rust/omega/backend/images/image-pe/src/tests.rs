use crate::emit_pe_x86_64_executable;
use arena::Handle;
use image::{
    FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageSection,
    FinalImageSymbol,
};
use object_file::SymbolKind;
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
