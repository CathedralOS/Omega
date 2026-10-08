//! The entry RVA, resolved from the final image's entry symbol.

use crate::constants::TEXT_RVA;
use diagnostics::Diagnostic;
use image::{FinalImage, FinalImageSection, final_image_symbol_name};

pub(crate) fn pe_entry_rva(image: &FinalImage) -> Result<u32, Diagnostic> {
    Ok(TEXT_RVA + image::final_image_entry_symbol(image, "PE")?.offset as u32)
}

#[cfg(test)]
mod tests {
    use super::pe_entry_rva;
    use crate::constants::TEXT_RVA;
    use arena::Handle;
    use image::{FinalImage, FinalImageSection, FinalImageSymbol};

    #[test]
    fn resolves_pe_entry_rva_from_final_image_entry_symbol() {
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
            offset: 6,
            size: 4,
            ..FinalImageSymbol::default()
        });
        image.symbol_table.entry_symbol = entry_symbol;

        assert_eq!(pe_entry_rva(&image).unwrap(), TEXT_RVA + 6);
    }
}
