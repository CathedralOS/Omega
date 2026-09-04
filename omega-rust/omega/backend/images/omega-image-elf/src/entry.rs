//! Resolution of the entry address from the final image's entry symbol, which must
//! be present and must live in text.

use omega_image::{FinalImage, FinalImageSection, final_image_symbol_name};
use psi_diagnostics::Diagnostic;

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
mod tests {
    use super::elf_entry_address;
    use omega_image::{FinalImage, FinalImageSection, FinalImageSymbol};
    use psi_arena::Handle;

    #[test]
    fn resolves_elf_entry_address_from_final_image_entry_symbol() {
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
            offset: 4,
            size: 4,
            ..FinalImageSymbol::default()
        });
        image.symbol_table.entry_symbol = entry_symbol;

        assert_eq!(elf_entry_address(&image, 0x401000).unwrap(), 0x401004);
    }
}
