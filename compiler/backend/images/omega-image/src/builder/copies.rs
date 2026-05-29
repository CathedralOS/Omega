use crate::model::{FinalImage, FinalImageImport, FinalImageRelocation, FinalImageSymbol};
use crate::symbols::{final_image_section, final_image_symbol_handle};
use omega_core::arena::Handle;
use omega_object_file::{ObjectPlan, RelocationPlan, SymbolKind};

pub(super) fn copy_object_symbols(image: &mut FinalImage, object: &ObjectPlan) {
    image
        .symbols
        .insert_many(object.symbols.iter().map(|(_, symbol)| FinalImageSymbol {
            name: symbol.name.clone(),
            section: final_image_section(symbol.section),
            offset: symbol.offset,
            size: symbol.size,
            kind: symbol.kind,
        }));
}

pub(super) fn copy_object_imports(image: &mut FinalImage, object: &ObjectPlan) {
    image.imports.insert_many(
        object
            .symbols
            .iter()
            .filter(|(_, symbol)| symbol.kind == SymbolKind::Import)
            .map(|(symbol_handle, _)| FinalImageImport {
                symbol_handle: final_image_symbol_handle(symbol_handle),
            }),
    );
}

pub(super) fn copy_object_relocations(image: &mut FinalImage, relocations: &RelocationPlan) {
    let symbols = &image.symbols;
    image.relocations.insert_many(
        relocations
            .record_set
            .records
            .iter()
            .map(|(_, relocation)| {
                let symbol_handle = final_image_symbol_handle(relocation.symbol_handle);
                FinalImageRelocation {
                    text_offset: relocation.text_offset,
                    byte_width: relocation.byte_width,
                    symbol_handle: symbol_handle
                        .is_valid()
                        .then_some(symbol_handle)
                        .filter(|handle| symbols.is_valid(*handle))
                        .unwrap_or_else(Handle::invalid),
                    kind: relocation.kind,
                }
            }),
    );
}
