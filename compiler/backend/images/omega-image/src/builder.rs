mod copies;
mod sections;

use crate::model::FinalImage;
use crate::symbols::final_image_symbol_handle;
use omega_core::arena::Arena;
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
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.kind == SymbolKind::Import)
        .count();
    let mut image = FinalImage {
        target: input.target,
        entry_symbol: final_image_symbol_handle(input.object.entry_symbol),
        text: input.text_bytes.to_vec(),
        data: input.data_bytes.to_vec(),
        bss_size: sections::section_size(input.object, SectionKind::Bss),
        bss_alignment: sections::section_alignment(input.object, SectionKind::Bss),
        symbols: Arena::with_capacity(input.object.symbols.len()),
        imports: Arena::with_capacity(import_count),
        relocations: Arena::with_capacity(input.relocations.record_set.records.len()),
    };

    copies::copy_object_symbols(&mut image, input.object);
    copies::copy_object_imports(&mut image, input.object);
    copies::copy_object_relocations(&mut image, input.relocations);

    image
}
