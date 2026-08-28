use crate::model::{FinalImage, FinalImageLayout, FinalImageSection, FinalImageSymbolHandle};
use omega_object_file::{ObjectSymbolHandle, SectionKind, SymbolSection};
use psi_arena::Handle;

pub(crate) fn final_image_symbol_handle(symbol: ObjectSymbolHandle) -> FinalImageSymbolHandle {
    if symbol.is_valid() {
        Handle::from_parts(symbol.arena_index(), symbol.generation())
    } else {
        FinalImageSymbolHandle::invalid()
    }
}

pub fn final_image_symbol_address(
    image: &FinalImage,
    symbol: FinalImageSymbolHandle,
    layout: &FinalImageLayout,
) -> Option<u64> {
    if !image.symbol_table.symbols.is_valid(symbol) {
        return None;
    }

    let symbol = image.symbol_table.symbols.get(symbol);
    let section_address = match symbol.section {
        FinalImageSection::Text => layout.text_address,
        FinalImageSection::Data => layout.data_address,
        FinalImageSection::Bss => layout.bss_address,
        FinalImageSection::None => return None,
    };

    Some(section_address + symbol.offset as u64)
}

pub fn final_image_imports_symbol(image: &FinalImage, symbol: FinalImageSymbolHandle) -> bool {
    image
        .symbol_table
        .imports
        .iter()
        .any(|(_, import)| import.symbol_handle == symbol)
}

pub fn final_image_symbol_name(image: &FinalImage, symbol: FinalImageSymbolHandle) -> &str {
    if image.symbol_table.symbols.is_valid(symbol) {
        image.symbol_table.symbols.get(symbol).name.as_str()
    } else {
        ""
    }
}

pub(crate) fn final_image_section(section: SymbolSection) -> FinalImageSection {
    match section {
        SymbolSection::None => FinalImageSection::None,
        SymbolSection::Section(SectionKind::Text) => FinalImageSection::Text,
        SymbolSection::Section(SectionKind::Data) => FinalImageSection::Data,
        SymbolSection::Section(SectionKind::Bss) => FinalImageSection::Bss,
    }
}
