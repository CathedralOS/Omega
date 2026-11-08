/*
We assemble a `FinalImage` from a `FinalImageInput` by pre-sizing the symbol,
import, and relocation arenas and copying the object plan's symbols, executable
regions, initialized-data regions, normalized or bootstrap imports, and
relocation records in a single deterministic pass.
*/

mod copies {
    use crate::final_image::{
        FinalDataRegion, FinalDataRegionOrigin, FinalExecutableRegion, FinalExecutableRegionOrigin,
        FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageRelocation, FinalImageSymbol,
    };
    use crate::symbols::{final_image_section, final_image_symbol_handle};
    use arena::Handle;
    use object_file::{ObjectPlan, RelocationPlan, SectionKind, SymbolKind, SymbolSection};

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
                .filter(|(_, symbol)| symbol.section == SymbolSection::Section(SectionKind::Text))
                .filter_map(|(_, symbol)| {
                    let origin = match symbol.kind {
                        SymbolKind::Function => FinalExecutableRegionOrigin::CompilerFunction,
                        SymbolKind::Object => FinalExecutableRegionOrigin::CompilerReadOnlyData,
                        SymbolKind::Import => return None,
                    };
                    Some(FinalExecutableRegion {
                        origin,
                        section_offset: symbol.offset,
                        byte_count: symbol.size,
                        symbol: symbol.name.clone(),
                        footprint: None,
                        entry_effects: None,
                    })
                }),
        );
    }

    pub(super) fn copy_object_data_regions(image: &mut FinalImage) {
        if image.memory.data.is_empty() {
            return;
        }
        image.data_regions.push(FinalDataRegion {
            origin: FinalDataRegionOrigin::CompilerData,
            section_offset: 0,
            byte_count: image.memory.data.len(),
            symbol: String::new(),
        });
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
                    import: object
                        .layout
                        .normalized_imports
                        .iter()
                        .find_map(|import| {
                            (import.symbol == symbol_handle)
                                .then(|| FinalImageImportPlan::Normalized(import.locator.clone()))
                        })
                        .unwrap_or_else(|| FinalImageImportPlan::StringBackedBootstrap {
                            library: symbol.import_library.clone(),
                        }),
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
                    section: final_image_section(object_file::SymbolSection::Section(
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
}

mod sections {
    use object_file::{ObjectPlan, SectionKind};

    pub(super) fn section_size(object: &ObjectPlan, kind: SectionKind) -> usize {
        object
            .layout
            .sections
            .iter()
            .find(|(_, section)| section.kind == kind)
            .map(|(_, section)| section.size)
            .unwrap_or(0)
    }

    pub(super) fn section_alignment(object: &ObjectPlan, kind: SectionKind) -> usize {
        object
            .layout
            .sections
            .iter()
            .find(|(_, section)| section.kind == kind)
            .map(|(_, section)| section.alignment)
            .unwrap_or(1)
    }
}

use crate::final_image::{FinalImage, FinalImageMemory};
use crate::symbols::final_image_symbol_handle;
use object_file::{ObjectPlan, RelocationPlan, SectionKind, SymbolKind};
use target::NativeTarget;

pub struct FinalImageInput<'a> {
    pub target: NativeTarget,
    pub object: &'a ObjectPlan,
    pub relocations: &'a RelocationPlan,
    pub text_bytes: &'a [u8],
    pub data_bytes: &'a [u8],
}

pub fn build_final_image(input: FinalImageInput<'_>) -> FinalImage {
    let import_count = input
        .object
        .layout
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.kind == SymbolKind::Import)
        .count();
    let mut image = FinalImage::with_capacity(
        input.target,
        FinalImageMemory {
            text: input.text_bytes.to_vec(),
            data: input.data_bytes.to_vec(),
            bss_size: sections::section_size(input.object, SectionKind::Bss),
            bss_alignment: sections::section_alignment(input.object, SectionKind::Bss),
        },
        final_image_symbol_handle(input.object.layout.entry_symbol),
        input.object.layout.symbols.len(),
        import_count,
        input.relocations.record_count(),
    );

    copies::copy_object_symbols(&mut image, input.object);
    copies::copy_object_executable_regions(&mut image, input.object);
    copies::copy_object_data_regions(&mut image);
    copies::copy_object_imports(&mut image, input.object);
    copies::copy_object_relocations(&mut image, input.relocations);

    image
}
