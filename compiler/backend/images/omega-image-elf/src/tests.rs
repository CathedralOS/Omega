use crate::emit_elf_aarch64_executable;
use omega_image::{FinalImage, FinalImageSection, FinalImageSymbol};

#[test]
fn emits_entry_address_from_final_image_entry_symbol() {
    let mut image = FinalImage {
        text: vec![0; 16],
        ..FinalImage::default()
    };
    let entry_symbol = image.symbols.insert(FinalImageSymbol {
        name: "_start".into(),
        section: FinalImageSection::Text,
        offset: 4,
        size: 4,
        ..FinalImageSymbol::default()
    });
    image.entry_symbol = entry_symbol;

    let output = emit_elf_aarch64_executable(image).expect("ELF image should emit");
    let entry_bytes: [u8; 8] = output.bytes[24..32].try_into().unwrap();

    assert_eq!(u64::from_le_bytes(entry_bytes), 0x401004);
}
