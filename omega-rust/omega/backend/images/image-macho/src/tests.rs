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
