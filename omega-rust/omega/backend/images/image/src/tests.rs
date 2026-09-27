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
