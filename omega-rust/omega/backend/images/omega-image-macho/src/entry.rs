//! The entry point's offset within text, resolved from the final image's entry
//! symbol.

use omega_image::{FinalImage, FinalImageSection, final_image_symbol_name};
use psi_diagnostics::Diagnostic;

pub(crate) fn macho_entry_text_offset(image: &FinalImage) -> Result<usize, Diagnostic> {
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
                "Mach-O entry symbol `{}` is missing from the final image",
                final_image_symbol_name(image, image.symbol_table.entry_symbol)
            ))
        })?;

    if entry_symbol.section != FinalImageSection::Text {
        return Err(Diagnostic::error(format!(
            "Mach-O entry symbol `{}` is not in the text section",
            final_image_symbol_name(image, image.symbol_table.entry_symbol)
        )));
    }

    Ok(entry_symbol.offset)
}

#[cfg(test)]
mod tests {
    use super::macho_entry_text_offset;
    use omega_image::{FinalImage, FinalImageSection, FinalImageSymbol};
    use psi_arena::Handle;

    #[test]
    fn resolves_macho_entry_offset_from_final_image_entry_symbol() {
        let mut image = FinalImage::with_capacity(
            FinalImage::default().target,
            omega_image::FinalImageMemory {
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
