use crate::emit_elf_aarch64_executable;
use omega_image::{FinalImage, FinalImageSection, FinalImageSymbol};
use psi_arena::Handle;

#[test]
fn emits_entry_address_from_final_image_entry_symbol() {
    let mut image = FinalImage::with_capacity(
        FinalImage::default().target,
        omega_image::FinalImageMemory {
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
        offset: 4,
        size: 4,
        ..FinalImageSymbol::default()
    });
    image.symbol_table.entry_symbol = entry_symbol;

    let output = emit_elf_aarch64_executable(image).expect("ELF image should emit");
    let entry_bytes: [u8; 8] = output.bytes[24..32].try_into().unwrap();

    assert_eq!(u64::from_le_bytes(entry_bytes), 0x401004);
    assert_eq!(output.executable_regions.text_address, 0x401000);
    assert_eq!(output.executable_regions.text_byte_count, 16);
    assert_eq!(output.executable_regions.unclassified_gaps.len(), 1);
}
