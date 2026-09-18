//! FinalImage-level loader regression: segment correspondence is not fixup
//! correspondence. This fixture supplies no source or installed-root authority.

use image::{
    ExecutableImageOutput, FinalDataRegion, FinalDataRegionOrigin, FinalExecutableRegion,
    FinalExecutableRegionOrigin, FinalImage, FinalImageImport, FinalImageImportPlan,
    FinalImageMemory, FinalImageRelocation, FinalImageSection, FinalImageSymbol,
    FinalImageSymbolHandle,
};
use object_file::{RelocationKind, SymbolKind};

fn fixup_image(with_import: bool) -> ExecutableImageOutput {
    let mut image = FinalImage::with_capacity(
        target::NativeTarget::macos_arm64(),
        FinalImageMemory {
            text: [0x9400_0000u32, 0xd65f_03c0]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
            data: vec![0; 16],
            bss_size: 32,
            bss_alignment: 16,
        },
        FinalImageSymbolHandle::invalid(),
        3,
        1,
        2,
    );
    image.symbol_table.entry_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "entry".into(),
        section: FinalImageSection::Text,
        size: 8,
        kind: SymbolKind::Function,
        ..Default::default()
    });
    // Classify the complete compiler-authored text so the placed executable
    // inventory starts with no unclassified gap.
    image.executable_regions.push(FinalExecutableRegion {
        origin: FinalExecutableRegionOrigin::CompilerFunction,
        section_offset: 0,
        byte_count: image.memory.text.len(),
        symbol: "entry".into(),
        footprint: None,
    });
    image.data_regions.push(FinalDataRegion {
        origin: FinalDataRegionOrigin::CompilerData,
        section_offset: 0,
        byte_count: image.memory.data.len(),
        symbol: String::new(),
    });
    let storage = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "storage".into(),
        section: FinalImageSection::Bss,
        size: 32,
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
    if with_import {
        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "diagnostic-import-name".into(),
            kind: SymbolKind::Import,
            ..Default::default()
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: imported,
            import: FinalImageImportPlan::Normalized(
                target::normalize_foreign_locator(
                    target::ForeignLocatorCandidate::MachODylibSymbol {
                        install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                        symbol: b"_write".to_vec(),
                    },
                    target::TargetProfile::MacosArm64,
                )
                .expect("normalized physical import"),
            ),
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
    }
    image_macho::emit_macho_aarch64_executable(image).expect("emit pointer and storage fixture")
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn command_offset(bytes: &[u8], kind: u32) -> usize {
    let mut cursor = 32;
    for _ in 0..word(bytes, 16) {
        if word(bytes, cursor) == kind {
            return cursor;
        }
        cursor += word(bytes, cursor + 4) as usize;
    }
    panic!("missing command {kind:x}");
}

fn validate_mapping(output: &ExecutableImageOutput) -> Result<(), diagnostics::Diagnostic> {
    image_macho::validate_macho_aarch64_loader_mapping(
        &output.bytes,
        output.final_image_layout,
        &output.final_text_bytes,
        &output.final_data_bytes,
        output.bss_bytes,
    )
}

fn validate_fixups(
    output: &ExecutableImageOutput,
    with_import: bool,
) -> Result<(), diagnostics::Diagnostic> {
    let rebases = [image_macho::MachoRebasePointer {
        data_offset: 0,
        // The original relocation names the BSS symbol at offset zero. Never
        // reconstruct this expectation from the potentially corrupted pointer.
        preferred_address: output.final_image_layout.bss_address,
    }];
    let imports = [image_macho::MachoImportPointer {
        data_offset: 16,
        install_name: b"/usr/lib/libSystem.B.dylib",
        symbol: b"_write",
    }];
    image_macho::validate_macho_aarch64_loader_fixups(
        &output.bytes,
        &output.final_data_bytes,
        &rebases,
        if with_import { &imports } else { &[] },
    )
}

#[test]
fn macho_fixup_replay_rejects_rebase_into_bss() {
    let mut output = fixup_image(false);
    validate_mapping(&output).expect("original mapping");
    validate_fixups(&output, false).expect("original fixups");
    let command = command_offset(&output.bytes, 0x8000_0022);
    let rebase_offset = word(&output.bytes, command + 8) as usize;
    assert_eq!(
        &output.bytes[rebase_offset..rebase_offset + 5],
        &[0x11, 0x22, 0, 0x51, 0]
    );
    let bss_offset = output.final_image_layout.bss_address - output.final_image_layout.data_address;
    assert_eq!(bss_offset, 16);
    output.bytes[rebase_offset + 2] = bss_offset as u8;
    validate_mapping(&output).expect("mapping alone cannot exclude loader writes");
    assert!(
        validate_fixups(&output, false).is_err(),
        "dyld must not rebase zero-fill storage instead of the declared pointer"
    );
}

#[test]
fn macho_fixup_replay_rejects_bind_into_bss() {
    let mut output = fixup_image(true);
    validate_mapping(&output).expect("original mapping");
    validate_fixups(&output, true).expect("original fixups");
    let command = command_offset(&output.bytes, 0x8000_0022);
    let bind_offset = word(&output.bytes, command + 16) as usize;
    let bind_size = word(&output.bytes, command + 20) as usize;
    let stream = &mut output.bytes[bind_offset..bind_offset + bind_size];
    assert_eq!(
        &stream[..9],
        &[0x11, 0x40, b'_', b'w', b'r', b'i', b't', b'e', 0]
    );
    assert_eq!(&stream[9..], &[0x51, 0x72, 16, 0x90, 0]);
    let bss_offset = output.final_image_layout.bss_address - output.final_image_layout.data_address;
    assert_eq!(bss_offset, 32);
    stream[11] = bss_offset as u8;
    validate_mapping(&output).expect("mapping alone cannot exclude loader writes");
    assert!(
        validate_fixups(&output, true).is_err(),
        "dyld must not bind an import into zero-fill storage"
    );
}

#[test]
fn macho_fixup_replay_rejects_substituted_import_symbol() {
    let mut output = fixup_image(true);
    validate_mapping(&output).expect("original mapping");
    validate_fixups(&output, true).expect("original fixups");
    let command = command_offset(&output.bytes, 0x8000_0022);
    let bind_offset = word(&output.bytes, command + 16) as usize;
    output.bytes[bind_offset + 2..bind_offset + 8].copy_from_slice(b"_close");
    validate_mapping(&output).expect("mapping does not select the physical import");
    assert!(
        validate_fixups(&output, true).is_err(),
        "same-width import spelling cannot redirect a selected call"
    );
}

#[test]
fn macho_fixup_replay_rejects_substituted_preferred_pointer() {
    let mut output = fixup_image(false);
    validate_mapping(&output).expect("original mapping");
    validate_fixups(&output, false).expect("original fixups");
    let replacement = output.final_image_layout.text_address.to_le_bytes();
    output.final_data_bytes[..8].copy_from_slice(&replacement);
    // The closed writer maps DATA at its preferred offset from the image base.
    let data_file_offset = (output.final_image_layout.data_address - 0x1_0000_0000) as usize;
    output.bytes[data_file_offset..data_file_offset + 8].copy_from_slice(&replacement);
    validate_mapping(&output).expect("matching file bytes do not prove the relocation target");
    assert!(
        validate_fixups(&output, false).is_err(),
        "relocation slot must retain its exact internal target"
    );
}

#[test]
fn macho_fixup_replay_rejects_omitted_overlapping_and_unsupported_operations() {
    let original = fixup_image(true);
    validate_fixups(&original, true).expect("original fixups");
    let command = command_offset(&original.bytes, 0x8000_0022);
    let rebase_offset = word(&original.bytes, command + 8) as usize;
    let bind_offset = word(&original.bytes, command + 16) as usize;
    for (name, offset, byte) in [
        ("omitted rebase", rebase_offset, 0),
        ("unclaimed initialized site", rebase_offset + 2, 8),
        ("repeated rebase", rebase_offset + 3, 0x52),
        ("missing rebase termination", rebase_offset + 4, 0x51),
        ("overflowing or truncated offset", rebase_offset + 2, 0x80),
        ("special library ordinal", bind_offset, 0x10),
        ("missing library ordinal", bind_offset, 0x12),
        ("weak symbol mode", bind_offset + 1, 0x41),
        ("non-pointer bind", bind_offset + 9, 0x52),
        ("unaccepted addend mode", bind_offset + 9, 0x60),
        ("text segment write", bind_offset + 10, 0x71),
        ("bind overlaps rebase", bind_offset + 11, 0),
        ("unaligned bind", bind_offset + 11, 17),
        ("omitted bind", bind_offset + 12, 0),
        ("missing bind termination", bind_offset + 13, 0x90),
    ] {
        let mut changed = original.clone();
        changed.bytes[offset] = byte;
        validate_mapping(&changed).expect("segment correspondence still holds");
        assert!(validate_fixups(&changed, true).is_err(), "{name}");
    }
    for (name, offset, value) in [
        ("omitted rebase range", command + 12, 0),
        ("overlapping streams", command + 16, rebase_offset as u32),
        ("excessive bind range", command + 20, u32::MAX),
        ("weak bind range", command + 24, bind_offset as u32),
        ("lazy bind range", command + 32, bind_offset as u32),
        ("export range", command + 40, bind_offset as u32),
    ] {
        let mut changed = original.clone();
        changed.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        validate_mapping(&changed).expect("segment correspondence still holds");
        assert!(validate_fixups(&changed, true).is_err(), "{name}");
    }
}

#[test]
fn macho_fixup_object_projection_retains_addends_and_normalized_import_targets() {
    use object_file::{
        NormalizedImportPlan, ObjectPlan, RelocationPlan, RelocationRecord, SectionKind,
        SectionPlan, SymbolPlan, SymbolSection,
    };

    let target = target::NativeTarget::macos_arm64();
    let text = [
        0x9400_0000u32, // bl imported getpid through its eager-bound thunk.
        0xd280_04a0,    // mov x0, #37.
        0xd280_0030,    // mov x16, #1.
        0xd400_1001,    // svc #0x80 -- Darwin exit.
        0xd420_0000,    // brk #0.
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect::<Vec<_>>();
    let data = [0u8; 16];
    let mut object = ObjectPlan::with_capacity(target, 3, 4);
    for (kind, size, alignment) in [
        (SectionKind::Text, text.len(), 4),
        (SectionKind::Data, data.len(), 8),
        (SectionKind::Bss, 32, 16),
    ] {
        object.layout.sections.insert(SectionPlan {
            kind,
            size,
            alignment,
        });
    }
    let entry = object.layout.symbols.insert(SymbolPlan {
        name: "entry".into(),
        section: SymbolSection::Section(SectionKind::Text),
        offset: 0,
        size: text.len(),
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    object.layout.entry_symbol = entry;
    let storage = object.layout.symbols.insert(SymbolPlan {
        name: "storage-interior".into(),
        section: SymbolSection::Section(SectionKind::Bss),
        offset: 8,
        size: 8,
        kind: SymbolKind::Object,
        import_library: String::new(),
    });
    // An unreferenced import must not consume a thunk or pointer-slot ordinal.
    object.layout.symbols.insert(SymbolPlan {
        name: "_close".into(),
        section: SymbolSection::None,
        offset: 0,
        size: 0,
        kind: SymbolKind::Import,
        import_library: String::new(),
    });
    let imported = object.layout.symbols.insert(SymbolPlan {
        name: "diagnostic-name-is-not-the-physical-symbol".into(),
        section: SymbolSection::None,
        offset: 0,
        size: 0,
        kind: SymbolKind::Import,
        import_library: "must-not-select-this-library".into(),
    });
    let locator = |symbol: &[u8]| {
        target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                symbol: symbol.to_vec(),
            },
            target::TargetProfile::MacosArm64,
        )
        .expect("normalized Mach-O locator")
    };
    object.layout.normalized_imports.push(NormalizedImportPlan {
        symbol: imported,
        locator: locator(b"_getpid"),
    });
    let mut relocations = RelocationPlan::with_target(target);
    let storage_relocation = relocations.push_record(RelocationRecord {
        section: SectionKind::Data,
        offset: 0,
        byte_width: 8,
        symbol_handle: storage,
        addend: -3,
        kind: RelocationKind::Absolute64,
        ..Default::default()
    });
    relocations.push_record(RelocationRecord {
        section: SectionKind::Data,
        offset: 8,
        byte_width: 8,
        symbol_handle: imported,
        kind: RelocationKind::Absolute64,
        ..Default::default()
    });
    relocations.push_record(RelocationRecord {
        section: SectionKind::Text,
        offset: 0,
        byte_width: 4,
        symbol_handle: imported,
        kind: RelocationKind::Aarch64Branch26,
        ..Default::default()
    });

    for addend in [-3, 3] {
        relocations
            .record_set
            .records
            .get_mut(storage_relocation)
            .addend = addend;
        let image = image::build_final_image(image::FinalImageInput {
            target,
            object: &object,
            relocations: &relocations,
            text_bytes: &text,
            data_bytes: &data,
        });
        let output = image::emitted_direct_executable_output(
            image_macho::emit_macho_aarch64_executable(image).expect("emit object fixup fixture"),
        );
        let validate_object = |candidate: &ObjectPlan, candidate_relocations: &RelocationPlan| {
            image_macho::validate_macho_aarch64_object_fixups(
                candidate,
                candidate_relocations,
                text.len(),
                data.len(),
                &output,
            )
        };
        validate_object(&object, &relocations).expect("exact source-free object projection");
        let preferred = u64::from_le_bytes(output.final_data_bytes[..8].try_into().unwrap());
        assert_eq!(
            preferred,
            (output.final_image_layout.bss_address + 8)
                .checked_add_signed(addend)
                .unwrap()
        );
        let import_pointer = u64::from_le_bytes(output.final_data_bytes[8..16].try_into().unwrap());
        assert_eq!(
            import_pointer,
            output.final_image_layout.text_address + text.len() as u64,
            "imported address names the first referenced thunk, not the unused import"
        );

        let mut changed_relocations = relocations.clone();
        changed_relocations
            .record_set
            .records
            .get_mut(storage_relocation)
            .addend += 1;
        assert!(validate_object(&object, &changed_relocations).is_err());
        let mut changed_object = object.clone();
        changed_object.layout.normalized_imports[0].locator = locator(b"_close");
        assert!(validate_object(&changed_object, &relocations).is_err());
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        super::hosted_exit_runtime::assert_exit(&output.bytes, 37);
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        eprintln!("SKIP: eager-bound Mach-O execution requires macOS AArch64");
    }
}

/// Mirror the complete-placement gate installed-artifact projection applies:
/// both placed-region inventories must replay over the exact final bytes, no
/// byte may remain unclassified, and every import thunk must pair with its
/// placed binding slot.
fn validate_complete_placement(
    output: &ExecutableImageOutput,
) -> Result<(), diagnostics::Diagnostic> {
    image::validate_placed_executable_region_inventory(
        &output.executable_regions,
        &output.final_text_bytes,
    )?;
    image::validate_placed_data_region_inventory(&output.data_regions, &output.final_data_bytes)?;
    if !output.executable_regions.unclassified_gaps.is_empty()
        || !output.data_regions.unclassified_gaps.is_empty()
    {
        return Err(diagnostics::Diagnostic::error(
            "placed inventories left final bytes unclassified",
        ));
    }
    image_macho::validate_macho_aarch64_import_binding_pairing(
        &output.final_text_bytes,
        &output.executable_regions,
        &output.data_regions,
    )
}

/// Every import-custody row is required independently: omitting or
/// substituting either the placed thunk or the placed binding slot must
/// reject complete-placement validation. The emitted thunk decodes its bound
/// pointer from the exact final text, so a forged inventory row cannot stand
/// in for writer-retained placement.
#[test]
fn macho_import_custody_rejects_missing_and_substituted_placement() {
    let output = fixup_image(true);
    validate_complete_placement(&output).expect("emitted import custody is complete");
    let thunk_offset = output
        .executable_regions
        .regions
        .iter()
        .find(|region| region.origin == FinalExecutableRegionOrigin::ImportThunk)
        .expect("one placed import thunk")
        .section_offset;
    assert_eq!(thunk_offset, 8, "the thunk follows the compiler text");
    let slot_offset = output
        .data_regions
        .regions
        .iter()
        .find(|region| region.origin == FinalDataRegionOrigin::ImportBindingSlot)
        .expect("one placed binding slot")
        .section_offset;
    assert_eq!(slot_offset, 16, "the binding slot follows compiler data");

    let mutations: Vec<(&str, Box<dyn Fn(&mut ExecutableImageOutput)>)> = vec![
        (
            "missing thunk placement",
            Box::new(move |output| {
                output
                    .executable_regions
                    .regions
                    .retain(|region| !(region.section_offset == thunk_offset));
            }),
        ),
        (
            "missing binding-slot placement",
            Box::new(move |output| {
                output
                    .data_regions
                    .regions
                    .retain(|region| !(region.section_offset == slot_offset));
            }),
        ),
        (
            "substituted thunk placement",
            Box::new(move |output| {
                let region = output
                    .executable_regions
                    .regions
                    .iter_mut()
                    .find(|region| region.section_offset == thunk_offset)
                    .expect("placed thunk region");
                region.address += 0x2000;
            }),
        ),
        (
            "substituted binding-slot placement",
            Box::new(move |output| {
                let region = output
                    .data_regions
                    .regions
                    .iter_mut()
                    .find(|region| region.section_offset == slot_offset)
                    .expect("placed binding-slot region");
                region.address += 8;
            }),
        ),
    ];
    for (label, mutate) in mutations {
        let mut changed = output.clone();
        mutate(&mut changed);
        // Mapping and loader fixups alone cannot see the drifted custody; the
        // placed-region replay and thunk↔slot pairing must reject it.
        validate_mapping(&changed).expect("segment mapping does not arbitrate custody");
        validate_fixups(&changed, true).expect("loader fixups do not arbitrate custody");
        assert!(
            validate_complete_placement(&changed).is_err(),
            "{label}: complete placement must reject it"
        );
    }
}
