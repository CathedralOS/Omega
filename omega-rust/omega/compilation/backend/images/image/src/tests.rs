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
        locator: locator.clone().into(),
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
        &FinalImageImportPlan::Normalized((locator).into()),
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

#[test]
fn executable_extent_uses_resolved_bytes_and_preserves_placement_refusals() {
    use crate::{
        FinalExecutableRegion, FinalExecutableRegionOrigin, place_executable_extent,
        validate_placed_executable_region_inventory,
    };
    let text = [11, 23, 47, 89, 131, 173];
    let region = FinalExecutableRegion {
        origin: FinalExecutableRegionOrigin::CompilerFunction,
        section_offset: 2,
        byte_count: 2,
        symbol: "resolved".to_owned(),
        footprint: None,
    };
    let inventory = place_executable_extent(&text, vec![region.clone()], 0x4000).unwrap();
    assert_eq!(inventory.text_byte_count, 6);
    assert_eq!(inventory.regions[0].address, 0x4002);
    assert_eq!(inventory.unclassified_gaps.len(), 2);
    assert_eq!(inventory.unclassified_gaps[0].byte_count, 2);
    assert_eq!(inventory.unclassified_gaps[1].section_offset, 4);
    validate_placed_executable_region_inventory(&inventory, &text).unwrap();
    for index in [0, 2, 5] {
        let mut changed = text;
        changed[index] ^= 1;
        assert!(validate_placed_executable_region_inventory(&inventory, &changed).is_err());
    }
    let overlapping = FinalExecutableRegion {
        section_offset: 3,
        ..region.clone()
    };
    assert!(
        place_executable_extent(&text, vec![region.clone(), overlapping], 0x4000)
            .unwrap_err()
            .to_string()
            .contains("overlaps")
    );
    let outside = FinalExecutableRegion {
        section_offset: 5,
        ..region.clone()
    };
    assert!(
        place_executable_extent(&text, vec![outside], 0x4000)
            .unwrap_err()
            .to_string()
            .contains("exceeds .text size")
    );
    assert!(
        place_executable_extent(&text, vec![region], u64::MAX)
            .unwrap_err()
            .to_string()
            .contains("address overflows")
    );
    let empty = place_executable_extent(&[], Vec::new(), 0).unwrap();
    validate_placed_executable_region_inventory(&empty, &[]).unwrap();
}
