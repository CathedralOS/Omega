//! Exercise the emitted storage and dyld-rebased pointer, without a C loader.
//! This validates image backing only, not a ProgramEntry receiver grant.

#[test]
fn relocated_macho_bss_is_zero_filled_and_writable() {
    use image::{
        FinalImage, FinalImageMemory, FinalImageRelocation, FinalImageSection, FinalImageSymbol,
        FinalImageSymbolHandle,
    };
    use object_file::{RelocationKind, SymbolKind};

    let text: Vec<u8> = [
        0x9000_0001u32, // adrp x1, pointer@page
        0x9100_0021,    // add x1, x1, pointer@pageoff
        0xf940_0021,    // ldr x1, [x1] -- dyld-rebased pointer to BSS
        0xf940_0020,    // ldr x0, [x1]
        0xb500_00e0,    // cbnz x0, failure
        0xd280_04a0,    // mov x0, #37
        0xf900_0020,    // str x0, [x1]
        0xf940_0020,    // ldr x0, [x1]
        0xd280_0030,    // mov x16, #1
        0xd400_1001,    // svc #0x80 -- Darwin exit
        0xd420_0000,    // brk #0
        0xd280_0020,    // failure: mov x0, #1
        0xd280_0030,    // mov x16, #1
        0xd400_1001,    // svc #0x80
        0xd420_0000,    // brk #0
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect();
    let text_size = text.len();
    let mut image = FinalImage::with_capacity(
        target::NativeTarget::macos_arm64(),
        FinalImageMemory {
            text,
            data: vec![0; 13],
            bss_size: 16,
            bss_alignment: 0x10000,
        },
        FinalImageSymbolHandle::invalid(),
        3,
        0,
        3,
    );
    image.symbol_table.entry_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "entry".into(),
        section: FinalImageSection::Text,
        size: text_size,
        kind: SymbolKind::Function,
        ..Default::default()
    });
    let pointer = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "pointer".into(),
        section: FinalImageSection::Data,
        size: 8,
        ..Default::default()
    });
    let storage = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "storage".into(),
        section: FinalImageSection::Bss,
        size: 16,
        ..Default::default()
    });
    for (section, offset, byte_width, symbol_handle, kind) in [
        (
            FinalImageSection::Text,
            0,
            4,
            pointer,
            RelocationKind::Aarch64Page21,
        ),
        (
            FinalImageSection::Text,
            4,
            4,
            pointer,
            RelocationKind::Aarch64PageOffset12,
        ),
        (
            FinalImageSection::Data,
            0,
            8,
            storage,
            RelocationKind::Absolute64,
        ),
    ] {
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section,
                offset,
                byte_width,
                symbol_handle,
                kind,
                addend: 0,
            });
    }
    let output = image_macho::emit_macho_aarch64_executable(image).expect("relocated BSS image");
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    super::hosted_exit_runtime::assert_exit(&output.bytes, 37);
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = output;
        eprintln!("SKIP: Mach-O BSS native execution requires macOS AArch64");
    }
}
