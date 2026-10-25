//! The entry point's offset within text, resolved from the final image's entry
//! symbol. The symbol's presence and text anchoring are the format-neutral
//! owner's rule; this module only supplies the Mach-O format name.

use diagnostics::Diagnostic;
use image::FinalImage;

pub(crate) fn macho_entry_text_offset(image: &FinalImage) -> Result<usize, Diagnostic> {
    Ok(image::final_image_entry_symbol(image, "Mach-O")?.offset)
}

#[cfg(test)]
mod tests {
    use super::macho_entry_text_offset;
    use arena::Handle;
    use image::{FinalImage, FinalImageSection, FinalImageSymbol};

    #[test]
    fn resolves_macho_entry_offset_from_final_image_entry_symbol() {
        let mut image = FinalImage::with_capacity(
            FinalImage::default().target,
            image::FinalImageMemory {
                text: vec![0; 16],
                ..Default::default()
            },
            Handle::invalid(),
            0,
            0,
            0,
        );
        let entry_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "_start".into(),
            section: FinalImageSection::Text,
            offset: 8,
            size: 4,
            ..FinalImageSymbol::default()
        });
        image.symbol_table.entry_symbol = entry_symbol;

        assert_eq!(macho_entry_text_offset(&image).unwrap(), 8);
    }
}
