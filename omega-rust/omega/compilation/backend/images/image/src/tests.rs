use crate::{FinalImageImportPlan, FinalImageInput, build_final_image};
use object_file::{
    NormalizedImportPlan, ObjectPlan, RelocationPlan, SymbolKind, SymbolPlan, SymbolSection,
};
use target::{ForeignLocatorCandidate, NativeTarget, TargetProfile, normalize_foreign_locator};

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

#[test]
fn object_kind_text_symbols_place_as_read_only_data_beside_code() {
    use crate::{FinalExecutableRegionOrigin, FinalImageLayout, place_executable_regions};
    use object_file::SectionKind;
    let target = NativeTarget::macos_arm64();
    let mut object = ObjectPlan::with_capacity(target, 1, 2);
    for (name, offset, size, kind) in [
        ("code", 0, 8, SymbolKind::Function),
        ("pool", 8, 8, SymbolKind::Object),
    ] {
        object.layout.symbols.insert(SymbolPlan {
            name: name.into(),
            section: SymbolSection::Section(SectionKind::Text),
            offset,
            size,
            kind,
            import_library: String::new(),
        });
    }
    let relocations = RelocationPlan::with_target(target);
    let text = [
        0x1f, 0x20, 0x03, 0xd5, 0x1f, 0x20, 0x03, 0xd5, b'D', b'o', b'o', b'r', 0, 0, 0, 0,
    ];
    let image = build_final_image(FinalImageInput {
        target,
        object: &object,
        relocations: &relocations,
        text_bytes: &text,
        data_bytes: &[],
    });
    let origins = image
        .executable_regions
        .iter()
        .map(|region| (region.symbol.as_str(), region.origin))
        .collect::<Vec<_>>();
    assert_eq!(
        origins,
        [
            ("code", FinalExecutableRegionOrigin::CompilerFunction),
            ("pool", FinalExecutableRegionOrigin::CompilerReadOnlyData),
        ]
    );
    let inventory = place_executable_regions(
        &image,
        FinalImageLayout {
            text_address: 0x1_0000_4000,
            data_address: 0,
            bss_address: 0,
        },
    )
    .unwrap();
    assert!(inventory.unclassified_gaps.is_empty());
}
