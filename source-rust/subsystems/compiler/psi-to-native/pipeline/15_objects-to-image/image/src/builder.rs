/*
We assemble a `FinalImage` from a `FinalImageInput` by pre-sizing the symbol,
import, and relocation arenas and copying the object plan's symbols, executable
regions, initialized-data regions, normalized imports, and relocation records in a single deterministic pass.
*/

mod copies {
    use crate::final_image::{
        FinalDataRegion, FinalDataRegionOrigin, FinalExecutableRegion, FinalExecutableRegionOrigin,
        FinalFunctionUnwind, FinalImage, FinalImageImport, FinalImageImportPlan,
        FinalImageRelocation, FinalImageSymbol,
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

    pub(super) fn copy_object_function_unwinds(image: &mut FinalImage, object: &ObjectPlan) {
        image
            .function_unwinds
            .extend(
                object
                    .layout
                    .function_unwinds
                    .iter()
                    .map(|unwind| FinalFunctionUnwind {
                        symbol: final_image_symbol_handle(unwind.symbol),
                        frame: unwind.frame.clone(),
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
                .map(|(symbol_handle, _)| FinalImageImport {
                    symbol_handle: final_image_symbol_handle(symbol_handle),
                    import: object
                        .layout
                        .normalized_imports
                        .iter()
                        .find_map(|import| {
                            (import.symbol == symbol_handle)
                                .then(|| FinalImageImportPlan::Normalized(import.locator.clone()))
                        })
                        // Without a normalized row the import keeps no plan,
                        // and every image writer refuses it.
                        .unwrap_or(FinalImageImportPlan::None),
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

pub struct FinalImageInput<'a, Imports = &'a machine_code::import_code::EncodedImportProgram> {
    pub target: NativeTarget,
    pub import_program: Imports,
    pub object: &'a ObjectPlan,
    pub relocations: &'a RelocationPlan,
    pub text_bytes: &'a [u8],
    pub data_bytes: &'a [u8],
}

pub fn build_final_image(input: FinalImageInput<'_>) -> FinalImage {
    FinalImageInput {
        target: input.target,
        import_program: input.import_program.clone(),
        object: input.object,
        relocations: input.relocations,
        text_bytes: input.text_bytes,
        data_bytes: input.data_bytes,
    }
    .into_image()
}

impl FinalImageInput<'_, machine_code::import_code::EncodedImportProgram> {
    pub fn into_image(self) -> FinalImage {
        let import_count = self
            .object
            .layout
            .symbols
            .iter()
            .filter(|(_, symbol)| symbol.kind == SymbolKind::Import)
            .count();
        let mut image = FinalImage::with_capacity(
            self.target,
            FinalImageMemory {
                text: self.text_bytes.to_vec(),
                data: self.data_bytes.to_vec(),
                bss_size: sections::section_size(self.object, SectionKind::Bss),
                bss_alignment: sections::section_alignment(self.object, SectionKind::Bss),
            },
            final_image_symbol_handle(self.object.layout.entry_symbol),
            self.object.layout.symbols.len(),
            import_count,
            self.relocations.record_count(),
        );

        image.import_program = self.import_program;
        copies::copy_object_symbols(&mut image, self.object);
        copies::copy_object_executable_regions(&mut image, self.object);
        copies::copy_object_function_unwinds(&mut image, self.object);
        copies::copy_object_data_regions(&mut image);
        copies::copy_object_imports(&mut image, self.object);
        copies::copy_object_relocations(&mut image, self.relocations);

        image
    }
}
