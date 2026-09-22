//! Exercise the emitted storage and dyld-rebased pointer, without a C loader.
//! This validates image backing only, not a ProgramEntry receiver grant.

fn ordinary_macho_image() -> (
    image_emission::ObjectArtifact,
    image_emission::ExecutableImage,
) {
    let mut plan = super::two_function_plan();
    plan.target = target::NativeTarget::macos_arm64();
    for function in &mut plan.functions {
        function.bytes = [0x5280_00e0u32, 0xd65f_03c0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
    }
    let object = image_emission::build_object_artifact(&plan).expect("ordinary Mach-O object");
    let original =
        image_emission::emit_direct_executable_image(&object, 3).expect("ordinary image");
    image_emission::validate_direct_executable_image(&object, &original)
        .expect("valid image replays");
    (object, original)
}

#[test]
fn macho_replay_rejects_replaced_pagezero_mapping() {
    let (object, original) = ordinary_macho_image();

    let mut substituted = original.clone();
    let output = substituted.output_mut_for_test();
    let text_address = output.final_image_layout.text_address;
    let pagezero = &mut output.bytes[32..104];
    assert_eq!(&pagezero[8..24], b"__PAGEZERO\0\0\0\0\0\0");
    // Keep all command offsets and the named __TEXT command unchanged while
    // replacing the reserved interval with a second loader-visible mapping.
    pagezero[8..24].copy_from_slice(b"__ALIAS\0\0\0\0\0\0\0\0\0");
    pagezero[24..32].copy_from_slice(&(text_address & !0x3fff).to_le_bytes());
    pagezero[32..40].copy_from_slice(&0x4000u64.to_le_bytes());
    pagezero[40..48].copy_from_slice(&0x4000u64.to_le_bytes());
    pagezero[48..56].copy_from_slice(&0x4000u64.to_le_bytes());
    pagezero[56..60].copy_from_slice(&3u32.to_le_bytes());
    pagezero[60..64].copy_from_slice(&3u32.to_le_bytes());
    assert!(
        image_emission::validate_direct_executable_image(&object, &substituted).is_err(),
        "replaying function bytes cannot excuse a substituted loader mapping"
    );
}

#[test]
fn macho_replay_rejects_loader_header_and_payload_drift() {
    let (object, original) = ordinary_macho_image();
    // The supported writer fixes the preferred text segment immediately above
    // the four-gigabyte reserved interval. Read that base from its own command.
    let base = u64::from_le_bytes(original.output().bytes[64..72].try_into().unwrap());
    let text_file_offset =
        usize::try_from(original.output().final_image_layout.text_address - base)
            .expect("text file offset");
    for (name, byte_offset, replacement) in [
        ("file type", 12, 6u32.to_le_bytes().to_vec()),
        ("header flags", 24, 0u32.to_le_bytes().to_vec()),
        ("pagezero rights", 32 + 60, 3u32.to_le_bytes().to_vec()),
        ("text flags", 104 + 68, 1u32.to_le_bytes().to_vec()),
        ("text bytes", text_file_offset, vec![0xff]),
    ] {
        let mut changed = original.clone();
        changed.output_mut_for_test().bytes[byte_offset..byte_offset + replacement.len()]
            .copy_from_slice(&replacement);
        assert!(
            image_emission::validate_direct_executable_image(&object, &changed).is_err(),
            "ordinary native replay must reject changed {name}"
        );
    }
}

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
    image.data_regions.push(image::FinalDataRegion {
        origin: image::FinalDataRegionOrigin::CompilerData,
        section_offset: 0,
        byte_count: image.memory.data.len(),
        symbol: String::new(),
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
