//! Assembly of one final image from an object plan and its relocation plan.

mod copies;
mod sections;

use crate::model::{FinalImage, FinalImageMemory};
use crate::symbols::final_image_symbol_handle;
use omega_object_file::{ObjectPlan, RelocationPlan, SectionKind, SymbolKind};
use omega_target::NativeTarget;

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
    copies::copy_object_imports(&mut image, input.object);
    copies::copy_object_relocations(&mut image, input.relocations);

    image
}
