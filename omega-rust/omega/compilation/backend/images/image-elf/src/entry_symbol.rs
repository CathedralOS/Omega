//! Resolution of the entry address from the final image's entry symbol, which must
//! be present and must live in text.

use diagnostics::Diagnostic;
use image::FinalImage;

pub(crate) fn elf_entry_address(image: &FinalImage, text_address: u64) -> Result<u64, Diagnostic> {
    Ok(text_address + image::final_image_entry_symbol(image, "ELF")?.offset as u64)
}

#[cfg(test)]
mod tests {}
