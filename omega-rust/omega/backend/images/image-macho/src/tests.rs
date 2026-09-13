//! Check loader-visible storage against the layout used for symbol relocation.
//! A planner-only assertion misses a section writer that recomputes placement.

use arena::Handle;
use image::{
    FinalImage, FinalImageMemory, FinalImageRelocation, FinalImageSection, FinalImageSymbol,
};
use object_file::{RelocationKind, SymbolKind};

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn wide(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn segment<'image>(bytes: &'image [u8], name: &[u8; 16]) -> &'image [u8] {
    let mut cursor = 32;
    let mut matching = None;
    for _ in 0..word(bytes, 16) {
        let end = cursor + word(bytes, cursor + 4) as usize;
        if word(bytes, cursor) == 0x19 && bytes.get(cursor + 8..cursor + 24) == Some(name) {
            assert!(matching.replace(&bytes[cursor..end]).is_none());
        }
        cursor = end;
    }
    assert_eq!(cursor, 32 + word(bytes, 20) as usize);
    matching.expect("one exact segment")
}

fn storage_image(data_size: usize, bss_size: usize, bss_alignment: usize) -> FinalImage {
    let mut image = FinalImage::with_capacity(
        target::NativeTarget::macos_arm64(),
        FinalImageMemory {
            text: 0xd65f_03c0u32.to_le_bytes().to_vec(), // ret
            data: vec![0; data_size],
            bss_size,
            bss_alignment,
        },
        Handle::invalid(),
        2,
        0,
        1,
    );
    image.symbol_table.entry_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "entry".into(),
        section: FinalImageSection::Text,
        size: 4,
        kind: SymbolKind::Function,
        ..Default::default()
    });
    if data_size >= 8 && bss_size > 0 {
        let storage = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "storage".into(),
            section: FinalImageSection::Bss,
            size: bss_size,
            ..Default::default()
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Data,
                byte_width: 8,
                symbol_handle: storage,
                kind: RelocationKind::Absolute64,
                ..Default::default()
            });
    }
    image
}

#[test]
fn emitted_bss_matches_relocated_storage_and_writable_segment() {
    for (data_size, bss_size, alignment) in [
        (13, 24, 16),
        (13, 0x4000, 0x4000),
        (16, 24, 16),
        (0, 24, 16),
        (0, 0x8000, 0x10000),
    ] {
        let output =
            super::emit_macho_aarch64_executable(storage_image(data_size, bss_size, alignment))
                .expect("emit storage image");
        let data = segment(&output.bytes, b"__DATA\0\0\0\0\0\0\0\0\0\0");
        let (sections, remainder) = data[72..].as_chunks::<80>();
        assert!(remainder.is_empty());
        assert_eq!(sections.len(), word(data, 64) as usize);
        let bss = sections
            .iter()
            .find(|section| &section[..16] == b"__bss\0\0\0\0\0\0\0\0\0\0\0")
            .expect("BSS section");
        let bss_address = wide(bss, 32);
        assert_eq!(
            bss_address, output.final_image_layout.bss_address,
            "loader and relocation disagree for data={data_size}, alignment={alignment}"
        );
        assert_eq!(bss_address % alignment as u64, 0);
        assert_eq!(wide(bss, 40), bss_size as u64);
        assert_eq!(word(bss, 48), 0, "zero-fill has no file payload");
        assert_eq!(word(bss, 52), alignment.trailing_zeros());
        assert_eq!(word(bss, 64), 1, "S_ZEROFILL");
        assert_eq!(word(data, 56), 3, "maximum read/write, non-executable");
        assert_eq!(word(data, 60), 3, "initial read/write, non-executable");
        assert_eq!(wide(data, 48), data_size as u64);
        assert!(bss_address >= wide(data, 24) + data_size as u64);
        assert!(bss_address + bss_size as u64 <= wide(data, 24) + wide(data, 32));
        let linkedit = segment(&output.bytes, b"__LINKEDIT\0\0\0\0\0\0");
        assert!(wide(linkedit, 24) >= bss_address + bss_size as u64);
        if data_size >= 8 {
            assert_eq!(wide(&output.final_data_bytes, 0), bss_address);
            let file_offset = wide(data, 40) as usize;
            assert_eq!(wide(&output.bytes, file_offset), bss_address);
        }
    }
}

fn validate_mapping(output: &image::ExecutableImageOutput) -> Result<(), diagnostics::Diagnostic> {
    super::validate_macho_aarch64_loader_mapping(
        &output.bytes,
        output.final_image_layout,
        &output.final_text_bytes,
        &output.final_data_bytes,
        output.bss_bytes,
    )
}

fn segment_offset(bytes: &[u8], name: &[u8; 16]) -> usize {
    segment(bytes, name).as_ptr() as usize - bytes.as_ptr() as usize
}

#[test]
fn loader_mapping_accepts_storage_variations() {
    for (data_size, bss_size, alignment) in [
        (0, 0, 1),
        (13, 0, 8),
        (13, 0, 0x10000),
        (0, 24, 16),
        (13, 24, 16),
        (13, 0x4000, 0x4000),
        (0, 0x8000, 0x10000),
    ] {
        let output =
            super::emit_macho_aarch64_executable(storage_image(data_size, bss_size, alignment))
                .expect("emit storage variation");
        validate_mapping(&output).expect("exact loader mapping");
    }
}

#[test]
fn loader_mapping_accepts_eager_import_storage() {
    for name in ["_write", "_objc_msgSend"] {
        let mut image = storage_image(13, 24, 16);
        image.memory.text[..4].copy_from_slice(&0x9400_0000u32.to_le_bytes());
        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: name.into(),
            kind: SymbolKind::Import,
            ..Default::default()
        });
        image.symbol_table.imports.insert(image::FinalImageImport {
            symbol_handle: imported,
            import: image::FinalImageImportPlan::StringBackedBootstrap {
                library: String::new(),
            },
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Text,
                byte_width: 4,
                symbol_handle: imported,
                kind: RelocationKind::Aarch64Branch26,
                ..Default::default()
            });
        let output = super::emit_macho_aarch64_executable(image).expect("emit eager import");
        validate_mapping(&output).expect("eager import loader mapping");
    }
}

#[test]
fn loader_mapping_rejects_segment_and_zero_fill_corruption() {
    let original = super::emit_macho_aarch64_executable(storage_image(13, 24, 16))
        .expect("emit storage image");
    let data_offset = segment_offset(&original.bytes, b"__DATA\0\0\0\0\0\0\0\0\0\0");
    let text_offset = segment_offset(&original.bytes, b"__TEXT\0\0\0\0\0\0\0\0\0\0");
    let linkedit_offset = segment_offset(&original.bytes, b"__LINKEDIT\0\0\0\0\0\0");
    for mutation in 0..10 {
        let mut output = original.clone();
        match mutation {
            0 => output.bytes[data_offset + 68..data_offset + 72]
                .copy_from_slice(&1u32.to_le_bytes()),
            1 => output.bytes[data_offset + 40..data_offset + 48]
                .copy_from_slice(&0u64.to_le_bytes()),
            2 => {
                // Reuse PAGEZERO's command without moving any retained section.
                output.bytes[40..56].copy_from_slice(b"__ALIAS\0\0\0\0\0\0\0\0\0");
                output.bytes[56..64]
                    .copy_from_slice(&output.final_image_layout.data_address.to_le_bytes());
            }
            3 => output.bytes[data_offset + 60..data_offset + 64]
                .copy_from_slice(&7u32.to_le_bytes()),
            4 => output.bytes[text_offset + 24..text_offset + 32]
                .copy_from_slice(&0u64.to_le_bytes()),
            5 => output.bytes[linkedit_offset + 24..linkedit_offset + 32]
                .copy_from_slice(&output.final_image_layout.data_address.to_le_bytes()),
            6 => {
                let bss_file_offset = wide(&output.bytes, data_offset + 40) as usize
                    + (output.final_image_layout.bss_address
                        - output.final_image_layout.data_address) as usize;
                output.bytes[bss_file_offset] = 1;
            }
            7 => {
                let file_offset = wide(&output.bytes, data_offset + 40) as usize;
                output.bytes[file_offset] ^= 1;
            }
            8 => output.bytes[data_offset + 32..data_offset + 40]
                .copy_from_slice(&u64::MAX.to_le_bytes()),
            _ => output.bytes[text_offset + 68..text_offset + 72]
                .copy_from_slice(&1u32.to_le_bytes()),
        }
        assert!(validate_mapping(&output).is_err(), "mutation {mutation}");
    }
}

#[test]
fn loader_mapping_rejects_malformed_command_envelopes() {
    let original =
        super::emit_macho_aarch64_executable(storage_image(0, 24, 16)).expect("emit storage image");
    for (offset, value) in [
        (0, 0),
        (12, 1),
        (24, 0),
        (28, 1),
        (16, u32::MAX),
        (16, 1),
        (20, u32::MAX),
        (36, 0),
        (36, 7),
        (36, u32::MAX),
        (32, 0x8000_0034),
    ] {
        let mut output = original.clone();
        output.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(
            validate_mapping(&output).is_err(),
            "offset {offset}, value {value}"
        );
    }
    for length in [0, 7, 31, 40, original.bytes.len() - 1] {
        let mut output = original.clone();
        output.bytes.truncate(length);
        assert!(
            validate_mapping(&output).is_err(),
            "truncated length {length}"
        );
    }
}
