mod aarch64_relocations;
mod model;

pub use aarch64_relocations::apply_aarch64_relocations;
pub use model::{
    FinalImage, FinalImageImport, FinalImageLayout, FinalImageRelocation, FinalImageSection,
    FinalImageSymbol, FinalImageSymbolHandle,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableImageOutput {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub imports: usize,
    pub relocations: usize,
}

pub fn final_image_symbol_address(
    image: &FinalImage,
    symbol: FinalImageSymbolHandle,
    layout: &FinalImageLayout,
) -> Option<u64> {
    if !image.symbols.is_valid(symbol) {
        return None;
    }

    let symbol = image.symbols.get(symbol);
    let section_address = match symbol.section {
        FinalImageSection::Text => layout.text_address,
        FinalImageSection::Data => layout.data_address,
        FinalImageSection::Bss => layout.bss_address,
        FinalImageSection::None => return None,
    };

    Some(section_address + symbol.offset as u64)
}

pub fn final_image_imports_symbol(image: &FinalImage, symbol_name: &str) -> bool {
    image
        .imports
        .iter()
        .any(|(_, import)| import.symbol == symbol_name)
}
