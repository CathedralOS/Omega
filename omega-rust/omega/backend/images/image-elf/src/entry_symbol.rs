//! Resolution of the entry address from the final image's entry symbol, which must
//! be present and must live in text.

use diagnostics::Diagnostic;
use image::{FinalImage, FinalImageSection, final_image_symbol_name};

pub(crate) fn elf_entry_address(image: &FinalImage, text_address: u64) -> Result<u64, Diagnostic> {
    let entry_symbol = image
        .symbol_table
        .symbols
        .is_valid(image.symbol_table.entry_symbol)
        .then(|| {
            image
                .symbol_table
                .symbols
                .get(image.symbol_table.entry_symbol)
        })
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "ELF entry symbol `{}` is missing from the final image",
                final_image_symbol_name(image, image.symbol_table.entry_symbol)
            ))
        })?;

    if entry_symbol.section != FinalImageSection::Text {
        return Err(Diagnostic::error(format!(
            "ELF entry symbol `{}` is not in the text section",
            final_image_symbol_name(image, image.symbol_table.entry_symbol)
        )));
    }

    Ok(text_address + entry_symbol.offset as u64)
}

#[cfg(test)]
mod tests {}
