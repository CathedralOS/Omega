//! Dyld import tests.

use super::{
    install_import_thunks, macho_bind_info, patch_import_thunks, validate_import_thunk_footprints,
};
use crate::dyld_linking::load_commands::write_macho_load_dylib_command;
use crate::isa::MachoIsa;
use arena::Handle;
use calling_conventions::{MachineRegister, MachineState, MachineStateSet};
use image::{
    FinalExecutableRegionOrigin, FinalImage, FinalImageImport, FinalImageImportPlan,
    FinalImageLayout, FinalImageRelocation, FinalImageSymbol,
};
use object_file::SymbolKind;
use target::{ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator};

fn image_with_referenced_import() -> FinalImage {
    let mut image = FinalImage::with_capacity(
        NativeTarget::macos_arm64(),
        Default::default(),
        Handle::invalid(),
        1,
        1,
        1,
    );
    let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "_write".into(),
        kind: SymbolKind::Import,
        ..FinalImageSymbol::default()
    });
    image.symbol_table.imports.insert(FinalImageImport {
        symbol_handle: symbol,
        import: FinalImageImportPlan::StringBackedBootstrap {
            library: "/usr/lib/libSystem.B.dylib".into(),
        },
    });
    image
        .relocation_table
        .relocations
        .insert(FinalImageRelocation {
            symbol_handle: symbol,
            ..FinalImageRelocation::default()
        });
    image
}

fn image_with_normalized_import(
    install_name: Vec<u8>,
    bind_symbol: Vec<u8>,
    diagnostic_symbol: &str,
) -> FinalImage {
    let mut image = image_with_referenced_import();
    let symbol_handle = image
        .symbol_table
        .imports
        .iter()
        .next()
        .expect("one import symbol")
        .1
        .symbol_handle;
    image.symbol_table.symbols.get_mut(symbol_handle).name = diagnostic_symbol.into();
    let locator = normalize_foreign_locator(
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name,
            symbol: bind_symbol,
        },
        TargetProfile::MacosArm64,
    )
    .expect("valid structural Mach-O locator");
    let import_handle = image
        .symbol_table
        .imports
        .iter()
        .next()
        .expect("one referenced import")
        .0;
    image.symbol_table.imports.get_mut(import_handle).import =
        FinalImageImportPlan::Normalized(locator);
    image
}

fn patch_test_thunks(image: &mut FinalImage, thunks: &[super::MachoImportThunk]) {
    patch_import_thunks(
        image,
        &FinalImageLayout {
            text_address: 0x1000,
            data_address: 0x2000,
            bss_address: 0x3000,
        },
        thunks,
        MachoIsa::Aarch64,
    )
    .expect("test Mach-O thunk should patch");
}

#[test]
fn installed_import_thunks_enter_the_executable_region_inventory() {
    let mut image = image_with_referenced_import();

    let imports =
        install_import_thunks(&mut image, MachoIsa::Aarch64).expect("valid bootstrap import");
    patch_test_thunks(&mut image, &imports.thunks);
    validate_import_thunk_footprints(&mut image, &imports.thunks, MachoIsa::Aarch64)
        .expect("patched Mach-O thunk bytes should validate");

    assert_eq!(imports.thunks.len(), 1);
    assert_eq!(image.executable_regions.len(), 1);
    assert_eq!(
        image.executable_regions[0].origin,
        FinalExecutableRegionOrigin::ImportThunk
    );
    assert_eq!(image.executable_regions[0].byte_count, 12);
    assert_eq!(image.executable_regions[0].symbol, "_write");
    let footprint = image.executable_regions[0]
        .footprint
        .as_ref()
        .expect("validated thunk should carry footprint evidence");
    assert!(
        footprint
            .registers()
            .contains(MachineRegister::Aarch64X(16))
    );
}

#[test]
fn mutated_import_thunk_opcode_rejects_final_validation() {
    let mut image = image_with_referenced_import();
    let imports =
        install_import_thunks(&mut image, MachoIsa::Aarch64).expect("valid bootstrap import");
    patch_test_thunks(&mut image, &imports.thunks);
    image.memory.text[9] = 0;

    let diagnostic =
        validate_import_thunk_footprints(&mut image, &imports.thunks, MachoIsa::Aarch64)
            .expect_err("mutated Mach-O thunk opcode must reject");
    assert!(diagnostic.message.contains("canonical binding-slot branch"));
}

#[test]
fn x86_64_import_thunks_emit_and_validate_the_closed_jmp_rip_form() {
    let mut image = FinalImage::with_capacity(
        NativeTarget::macos_x64(),
        Default::default(),
        Handle::invalid(),
        1,
        1,
        1,
    );
    let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "_write".into(),
        kind: SymbolKind::Import,
        ..FinalImageSymbol::default()
    });
    image.symbol_table.imports.insert(FinalImageImport {
        symbol_handle: symbol,
        import: FinalImageImportPlan::StringBackedBootstrap {
            library: "/usr/lib/libSystem.B.dylib".into(),
        },
    });
    image
        .relocation_table
        .relocations
        .insert(FinalImageRelocation {
            symbol_handle: symbol,
            ..FinalImageRelocation::default()
        });

    let imports =
        install_import_thunks(&mut image, MachoIsa::X86_64).expect("valid x86-64 bootstrap import");
    assert_eq!(imports.thunks.len(), 1);
    assert_eq!(image.executable_regions[0].byte_count, 6);
    assert_eq!(image.memory.text.len(), 6);
    assert_eq!(image.memory.data.len(), 8);

    patch_import_thunks(
        &mut image,
        &FinalImageLayout {
            text_address: 0x1000,
            data_address: 0x2000,
            bss_address: 0x3000,
        },
        &imports.thunks,
        MachoIsa::X86_64,
    )
    .expect("x86-64 Mach-O thunk should patch");
    // `jmp qword ptr [rip + 0xffa]` = 0x1006 + 0xffa = 0x2000, the binding slot.
    assert_eq!(&image.memory.text[..], &[0xff, 0x25, 0xfa, 0x0f, 0, 0]);

    validate_import_thunk_footprints(&mut image, &imports.thunks, MachoIsa::X86_64)
        .expect("patched x86-64 thunk bytes should validate");
    let footprint = image.executable_regions[0]
        .footprint
        .as_ref()
        .expect("validated thunk should carry footprint evidence");
    // The closed x86-64 form writes no register; only the instruction pointer.
    assert!(!footprint.registers().contains(MachineRegister::X86Rax));
    assert!(
        footprint
            .machine_state()
            .contains_all(MachineStateSet::new([MachineState::InstructionPointer]))
    );
}

#[test]
fn x86_64_mutated_import_thunk_rejects_final_validation() {
    let mut image = FinalImage::with_capacity(
        NativeTarget::macos_x64(),
        Default::default(),
        Handle::invalid(),
        1,
        1,
        1,
    );
    let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "_write".into(),
        kind: SymbolKind::Import,
        ..FinalImageSymbol::default()
    });
    image.symbol_table.imports.insert(FinalImageImport {
        symbol_handle: symbol,
        import: FinalImageImportPlan::StringBackedBootstrap {
            library: "/usr/lib/libSystem.B.dylib".into(),
        },
    });
    image
        .relocation_table
        .relocations
        .insert(FinalImageRelocation {
            symbol_handle: symbol,
            ..FinalImageRelocation::default()
        });
    let imports =
        install_import_thunks(&mut image, MachoIsa::X86_64).expect("valid x86-64 bootstrap import");
    patch_import_thunks(
        &mut image,
        &FinalImageLayout {
            text_address: 0x1000,
            data_address: 0x2000,
            bss_address: 0x3000,
        },
        &imports.thunks,
        MachoIsa::X86_64,
    )
    .expect("patch");
    image.memory.text[0] = 0xff;
    image.memory.text[1] = 0x24;

    assert!(
        validate_import_thunk_footprints(&mut image, &imports.thunks, MachoIsa::X86_64).is_err()
    );
}

#[test]
fn normalized_macho_locator_emits_exact_raw_load_and_bind_bytes() {
    let install_name = b"/tmp/lib\xffomega.dylib".to_vec();
    let bind_symbol = b"_raw_\xfe_entry".to_vec();
    let mut image = image_with_normalized_import(
        install_name.clone(),
        bind_symbol.clone(),
        "unrelated diagnostic label",
    );

    let imports =
        install_import_thunks(&mut image, MachoIsa::Aarch64).expect("normalized Mach-O import");

    assert_eq!(imports.thunks.len(), 1);
    assert_eq!(imports.thunks[0].symbol, "unrelated diagnostic label");
    assert_eq!(imports.thunks[0].bind_symbol, bind_symbol);
    assert_eq!(imports.thunks[0].library, install_name);
    assert_eq!(imports.thunks[0].dylib_ordinal, 2);
    assert_eq!(imports.dylibs.len(), 2);
    let mut load_command = Vec::new();
    write_macho_load_dylib_command(&mut load_command, &imports.dylibs[1]);
    assert_eq!(
        &load_command[24..24 + install_name.len()],
        install_name.as_slice()
    );
    let bind_info = macho_bind_info(&imports.thunks);
    let mut expected_prefix = vec![0x12, 0x40];
    expected_prefix.extend(&bind_symbol);
    expected_prefix.push(0);
    assert!(bind_info.starts_with(&expected_prefix));
}

#[test]
fn normalized_macho_imports_deduplicate_raw_install_names_and_share_ordinal() {
    let install_name = b"/tmp/libomega-custom.dylib".to_vec();
    let mut image =
        image_with_normalized_import(install_name.clone(), b"_first".to_vec(), "first diagnostic");
    let second_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "second diagnostic".into(),
        kind: SymbolKind::Import,
        ..FinalImageSymbol::default()
    });
    let second_locator = normalize_foreign_locator(
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name: install_name.clone(),
            symbol: b"_second".to_vec(),
        },
        TargetProfile::MacosArm64,
    )
    .expect("second normalized locator");
    image.symbol_table.imports.insert(FinalImageImport {
        symbol_handle: second_symbol,
        import: FinalImageImportPlan::Normalized(second_locator),
    });
    image
        .relocation_table
        .relocations
        .insert(FinalImageRelocation {
            symbol_handle: second_symbol,
            ..FinalImageRelocation::default()
        });

    let imports =
        install_import_thunks(&mut image, MachoIsa::Aarch64).expect("two normalized imports");

    assert_eq!(imports.dylibs.len(), 2);
    assert_eq!(imports.dylibs[1].path.as_ref(), install_name);
    assert_eq!(
        imports
            .thunks
            .iter()
            .map(|thunk| thunk.dylib_ordinal)
            .collect::<Vec<_>>(),
        vec![2, 2]
    );
}

#[test]
fn wrong_normalized_locator_case_rejects_without_image_mutation() {
    let mut image = image_with_referenced_import();
    let locator = normalize_foreign_locator(
        ForeignLocatorCandidate::PeByName {
            library: b"KERNEL32.dll".to_vec(),
            export: b"ExitProcess".to_vec(),
        },
        TargetProfile::WindowsX64,
    )
    .expect("valid PE locator");
    let import_handle = image.symbol_table.imports.iter().next().unwrap().0;
    image.symbol_table.imports.get_mut(import_handle).import =
        FinalImageImportPlan::Normalized(locator);
    let before = image.clone();

    let diagnostic = install_import_thunks(&mut image, MachoIsa::Aarch64)
        .expect_err("non-Mach-O normalized locator must reject");

    assert!(diagnostic.message.contains("non-Mach-O"));
    assert_eq!(image, before);
}

#[test]
fn duplicate_import_row_rejects_without_image_mutation() {
    let mut image = image_with_referenced_import();
    let duplicate = image.symbol_table.imports.iter().next().unwrap().1.clone();
    image.symbol_table.imports.insert(duplicate);
    let before = image.clone();

    let diagnostic = install_import_thunks(&mut image, MachoIsa::Aarch64)
        .expect_err("duplicate import row must reject");

    assert!(diagnostic.message.contains("duplicate import rows"));
    assert_eq!(image, before);
}

#[test]
fn repeated_normalized_identity_rejects_without_image_mutation() {
    let mut image =
        image_with_normalized_import(b"/tmp/libomega.dylib".to_vec(), b"_same".to_vec(), "first");
    let locator = image
        .symbol_table
        .imports
        .iter()
        .next()
        .and_then(|(_, import)| match &import.import {
            FinalImageImportPlan::Normalized(locator) => Some(locator.clone()),
            _ => None,
        })
        .unwrap();
    let second_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "second".into(),
        kind: SymbolKind::Import,
        ..FinalImageSymbol::default()
    });
    image.symbol_table.imports.insert(FinalImageImport {
        symbol_handle: second_symbol,
        import: FinalImageImportPlan::Normalized(locator),
    });
    let before = image.clone();

    let diagnostic = install_import_thunks(&mut image, MachoIsa::Aarch64)
        .expect_err("one normalized identity cannot name two import symbols");

    assert!(diagnostic.message.contains("same exact locator"));
    assert_eq!(image, before);
}

#[test]
fn excessive_dylib_ordinals_reject_before_image_mutation() {
    let mut image = FinalImage::with_capacity(
        NativeTarget::macos_arm64(),
        Default::default(),
        Handle::invalid(),
        15,
        15,
        15,
    );
    for index in 0..15 {
        let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: format!("diagnostic-{index}"),
            kind: SymbolKind::Import,
            ..FinalImageSymbol::default()
        });
        let locator = normalize_foreign_locator(
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name: format!("/tmp/libomega-{index}.dylib").into_bytes(),
                symbol: format!("_entry_{index}").into_bytes(),
            },
            TargetProfile::MacosArm64,
        )
        .expect("valid distinct locator");
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: symbol,
            import: FinalImageImportPlan::Normalized(locator),
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                symbol_handle: symbol,
                ..FinalImageRelocation::default()
            });
    }
    let before = image.clone();

    let diagnostic = install_import_thunks(&mut image, MachoIsa::Aarch64)
        .expect_err("sixteen image-local dylib ordinals exceed IMM encoding");

    assert!(diagnostic.message.contains("supports at most 15"));
    assert_eq!(image, before);
}
