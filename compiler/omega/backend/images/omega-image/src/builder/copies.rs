use crate::model::{
    FinalExecutableRegion, FinalExecutableRegionOrigin, FinalImage, FinalImageImport,
    FinalImageRelocation, FinalImageSymbol,
};
use crate::symbols::{final_image_section, final_image_symbol_handle};
use omega_object_file::{ObjectPlan, RelocationPlan, SectionKind, SymbolKind, SymbolSection};
use psi_arena::Handle;

pub(super) fn copy_object_symbols(image: &mut FinalImage, object: &ObjectPlan) {
    image
        .symbol_table
        .symbols
        .insert_many(
            object
                .layout
                .symbols
                .iter()
                .map(|(_, symbol)| FinalImageSymbol {
                    name: symbol.name.clone(),
                    section: final_image_section(symbol.section),
                    offset: symbol.offset,
                    size: symbol.size,
                    kind: symbol.kind,
                }),
        );
}

pub(super) fn copy_object_executable_regions(image: &mut FinalImage, object: &ObjectPlan) {
    image.executable_regions.extend(
        object
            .layout
            .symbols
            .iter()
            .filter(|(_, symbol)| {
                symbol.kind == SymbolKind::Function
                    && symbol.section == SymbolSection::Section(SectionKind::Text)
            })
            .map(|(_, symbol)| FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: symbol.offset,
                byte_count: symbol.size,
                symbol: symbol.name.clone(),
                footprint: None,
            }),
    );
}

pub(super) fn copy_object_imports(image: &mut FinalImage, object: &ObjectPlan) {
    image.symbol_table.imports.insert_many(
        object
            .layout
            .symbols
            .iter()
            .filter(|(_, symbol)| symbol.kind == SymbolKind::Import)
            .map(|(symbol_handle, symbol)| FinalImageImport {
                symbol_handle: final_image_symbol_handle(symbol_handle),
                library: symbol.import_library.clone(),
            }),
    );
}

pub(super) fn copy_object_relocations(image: &mut FinalImage, relocations: &RelocationPlan) {
    let symbols = &image.symbol_table.symbols;
    image
        .relocation_table
        .relocations
        .insert_many(relocations.records().map(|(_, relocation)| {
            let symbol_handle = final_image_symbol_handle(relocation.symbol_handle);
            FinalImageRelocation {
                section: final_image_section(omega_object_file::SymbolSection::Section(
                    relocation.section,
                )),
                offset: relocation.offset,
                byte_width: relocation.byte_width,
                symbol_handle: symbol_handle
                    .is_valid()
                    .then_some(symbol_handle)
                    .filter(|handle| symbols.is_valid(*handle))
                    .unwrap_or_else(Handle::invalid),
                addend: relocation.addend,
                kind: relocation.kind,
            }
        }));
}
