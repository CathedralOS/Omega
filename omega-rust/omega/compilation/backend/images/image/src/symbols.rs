//! Handle and section translation between the object plan and the final image,
//! including the coordinate reuse the crate header explains.

use crate::final_image::{
    FinalImage, FinalImageLayout, FinalImageSection, FinalImageSymbol, FinalImageSymbolHandle,
};
use arena::Handle;
use object_file::{ObjectSymbolHandle, SectionKind, SymbolSection};

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
    let symbol = image.symbol_table.symbol(symbol)?;
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
    image
        .symbol_table
        .symbol(symbol)
        .map_or("", |symbol| symbol.name.as_str())
}

/// The validated entry symbol of a final image under the named image format:
/// present in the symbol table and anchored in the text section, otherwise the
/// format's entry diagnostic.
pub fn final_image_entry_symbol<'a>(
    image: &'a FinalImage,
    format: &'static str,
) -> Result<&'a FinalImageSymbol, diagnostics::Diagnostic> {
    let entry_symbol = image
        .symbol_table
        .symbol(image.symbol_table.entry_symbol)
        .ok_or_else(|| {
            diagnostics::Diagnostic::error(format!(
                "{format} entry symbol `{}` is missing from the final image",
                final_image_symbol_name(image, image.symbol_table.entry_symbol)
            ))
        })?;

    if entry_symbol.section != FinalImageSection::Text {
        return Err(diagnostics::Diagnostic::error(format!(
            "{format} entry symbol `{}` is not in the text section",
            final_image_symbol_name(image, image.symbol_table.entry_symbol)
        )));
    }

    Ok(entry_symbol)
}

pub(crate) fn final_image_section(section: SymbolSection) -> FinalImageSection {
    match section {
        SymbolSection::None => FinalImageSection::None,
        SymbolSection::Section(SectionKind::Text) => FinalImageSection::Text,
        SymbolSection::Section(SectionKind::Data) => FinalImageSection::Data,
        SymbolSection::Section(SectionKind::Bss) => FinalImageSection::Bss,
    }
}
