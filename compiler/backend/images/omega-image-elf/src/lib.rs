use omega_core::diagnostics::Diagnostic;
use omega_image::{
    ExecutableImageOutput, FinalImage, FinalImageLayout, FinalImageSection,
    apply_aarch64_relocations, final_image_symbol_name,
};

mod bytes;
mod constants;
mod headers;
mod layout;

use constants::{
    ELF_HEADER_SIZE, IMAGE_BASE, PAGE_SIZE, PROGRAM_HEADER_COUNT, PROGRAM_HEADER_SIZE,
};
use headers::{write_data_program_header, write_elf_header, write_text_program_header};
use layout::{align_to, align_to_u64};

pub fn emit_elf_aarch64_executable(
    mut image: FinalImage,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let text_offset = align_to(
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE * PROGRAM_HEADER_COUNT,
        PAGE_SIZE,
    );
    let data_offset = align_to(text_offset + image.text.len(), PAGE_SIZE);
    let text_address = IMAGE_BASE + text_offset as u64;
    let data_address = IMAGE_BASE + data_offset as u64;
    let bss_address = align_to_u64(
        data_address + image.data.len() as u64,
        image.bss_alignment as u64,
    );
    let layout = FinalImageLayout {
        text_address,
        data_address,
        bss_address,
    };
    let entry_address = elf_entry_address(&image, text_address)?;

    apply_aarch64_relocations(&mut image, &layout, "ELF direct image")?;

    let data_memory_size = (bss_address - data_address)
        .checked_add(image.bss_size as u64)
        .expect("ELF data memory size overflow");
    let mut bytes = Vec::with_capacity(data_offset + image.data.len());
    write_elf_header(&mut bytes, entry_address, text_offset, data_offset);
    write_text_program_header(&mut bytes, text_offset, image.text.len());
    write_data_program_header(&mut bytes, data_offset, image.data.len(), data_memory_size);
    bytes.resize(text_offset, 0);
    bytes.extend(&image.text);
    bytes.resize(data_offset, 0);
    bytes.extend(&image.data);

    Ok(ExecutableImageOutput {
        bytes,
        file_name: "omega-program".to_owned(),
        format: "elf64-aarch64-executable".to_owned(),
        text_bytes: image.text.len(),
        data_bytes: image.data.len(),
        bss_bytes: image.bss_size,
        symbols: image.symbols.len(),
        imports: image.imports.len(),
        relocations: image.relocations.len(),
    })
}

fn elf_entry_address(image: &FinalImage, text_address: u64) -> Result<u64, Diagnostic> {
    let entry_symbol = image
        .symbols
        .is_valid(image.entry_symbol)
        .then(|| image.symbols.get(image.entry_symbol))
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "ELF entry symbol `{}` is missing from the final image",
                final_image_symbol_name(image, image.entry_symbol)
            ))
        })?;

    if entry_symbol.section != FinalImageSection::Text {
        return Err(Diagnostic::error(format!(
            "ELF entry symbol `{}` is not in the text section",
            final_image_symbol_name(image, image.entry_symbol)
        )));
    }

    Ok(text_address + entry_symbol.offset as u64)
}

#[cfg(test)]
mod tests {
    use super::emit_elf_aarch64_executable;
    use omega_image::{FinalImage, FinalImageSection, FinalImageSymbol};

    #[test]
    fn emits_entry_address_from_final_image_entry_symbol() {
        let mut image = FinalImage {
            text: vec![0; 16],
            ..FinalImage::default()
        };
        let entry_symbol = image.symbols.insert(FinalImageSymbol {
            name: "_start".into(),
            section: FinalImageSection::Text,
            offset: 4,
            size: 4,
            ..FinalImageSymbol::default()
        });
        image.entry_symbol = entry_symbol;

        let output = emit_elf_aarch64_executable(image).expect("ELF image should emit");
        let entry_bytes: [u8; 8] = output.bytes[24..32].try_into().unwrap();

        assert_eq!(u64::from_le_bytes(entry_bytes), 0x401004);
    }
}
