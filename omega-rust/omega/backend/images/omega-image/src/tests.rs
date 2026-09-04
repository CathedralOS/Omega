use crate::{
    FinalExecutableRegionOrigin, FinalImageImportPlan, FinalImageInput, FinalImageLayout,
    FinalImageSection, build_final_image, final_image_symbol_address,
};
use omega_object_file::{
    NormalizedImportPlan, ObjectPlan, RelocationKind, RelocationPlan, RelocationRecord,
    SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection,
};
use omega_target::{
    ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator,
};

#[test]
fn builds_final_image_from_object_symbols_imports_and_relocations() {
    let target = NativeTarget::host();
    let mut object = ObjectPlan::with_capacity(target, 0, 0);
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: 8,
        alignment: 16,
    });
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Data,
        size: 3,
        alignment: 4,
    });
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Bss,
        size: 24,
        alignment: 8,
    });

    let entry_symbol = object.layout.symbols.insert(SymbolPlan {
        name: "_start".into(),
        section: SymbolSection::Section(SectionKind::Text),
        offset: 4,
        size: 4,
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    let import_symbol = object.layout.symbols.insert(SymbolPlan {
        name: "host_write".into(),
        section: SymbolSection::None,
        offset: 0,
        size: 0,
        kind: SymbolKind::Import,
        import_library: String::new(),
    });
    object.layout.symbols.insert(SymbolPlan {
        name: "payload".into(),
        section: SymbolSection::Section(SectionKind::Data),
        offset: 2,
        size: 1,
        kind: SymbolKind::Object,
        import_library: String::new(),
    });
    object.layout.entry_symbol = entry_symbol;

    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(RelocationRecord {
        origin: omega_object_file::RelocationOrigin::Instruction {
            function_symbol_handle: entry_symbol,
            selected_instruction_index: 0,
        },
        section: SectionKind::Text,
        offset: 4,
        byte_width: 4,
        symbol_handle: import_symbol,
        addend: 0,
        kind: RelocationKind::X86_64Relative32,
    });

    let image = build_final_image(FinalImageInput {
        target,
        object: &object,
        relocations: &relocations,
        text_bytes: &[0xaa; 8],
        data_bytes: &[1, 2, 3],
    });

    assert_eq!(image.target, target);
    assert_eq!(image.memory.text, vec![0xaa; 8]);
    assert_eq!(image.memory.data, vec![1, 2, 3]);
    assert_eq!((image.memory.bss_size, image.memory.bss_alignment), (24, 8));
    assert_eq!(image.symbol_table.symbols.len(), 3);
    assert_eq!(image.symbol_table.imports.len(), 1);
    assert_eq!(image.relocation_table.relocations.len(), 1);
    assert_eq!(image.executable_regions.len(), 1);
    assert_eq!(
        image.executable_regions[0].origin,
        FinalExecutableRegionOrigin::CompilerFunction
    );
    assert_eq!(image.executable_regions[0].section_offset, 4);
    assert_eq!(image.executable_regions[0].byte_count, 4);
    assert_eq!(image.executable_regions[0].symbol, "_start");

    let entry = image
        .symbol_table
        .symbols
        .get(image.symbol_table.entry_symbol);
    assert_eq!(entry.name, "_start");
    assert_eq!(entry.section, FinalImageSection::Text);
    assert_eq!(entry.offset, 4);

    let import = image
        .symbol_table
        .imports
        .iter()
        .next()
        .expect("expected import")
        .1;
    assert_eq!(
        image.symbol_table.symbols.get(import.symbol_handle).name,
        "host_write"
    );
    assert_eq!(
        image
            .relocation_table
            .relocations
            .iter()
            .next()
            .unwrap()
            .1
            .symbol_handle,
        import.symbol_handle
    );
    assert_eq!(
        final_image_symbol_address(
            &image,
            image.symbol_table.entry_symbol,
            &FinalImageLayout {
                text_address: 0x1000,
                data_address: 0x2000,
                bss_address: 0x3000,
            }
        ),
        Some(0x1004)
    );
    assert!(matches!(
        &import.import,
        FinalImageImportPlan::StringBackedBootstrap { library } if library.is_empty()
    ));
}

#[test]
fn final_image_keeps_normalized_import_atomic_and_ignores_symbol_spelling() {
    let target = NativeTarget::windows_x64();
    let locator = normalize_foreign_locator(
        ForeignLocatorCandidate::PeByOrdinal {
            library: b"raw\xff.dll".to_vec(),
            ordinal: 23,
        },
        TargetProfile::WindowsX64,
    )
    .expect("valid PE locator");
    let mut object = ObjectPlan::with_capacity(target, 0, 1);
    let symbol = object.layout.symbols.insert(SymbolPlan {
        name: "diagnostic-only".into(),
        section: SymbolSection::None,
        offset: 0,
        size: 0,
        kind: SymbolKind::Import,
        import_library: "must-not-win.dll".into(),
    });
    object.layout.normalized_imports.push(NormalizedImportPlan {
        symbol,
        locator: locator.clone(),
    });
    let relocations = RelocationPlan::with_target(target);

    let image = build_final_image(FinalImageInput {
        target,
        object: &object,
        relocations: &relocations,
        text_bytes: &[],
        data_bytes: &[],
    });
    let import = image.symbol_table.imports.iter().next().unwrap().1;
    assert_eq!(
        &import.import,
        &FinalImageImportPlan::Normalized(locator),
        "normalized coordinates must win atomically over legacy symbol fields"
    );
}
