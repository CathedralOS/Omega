use omega_core::diagnostics::Diagnostic;
use omega_image::{ExecutableImageOutput, FinalImage, FinalImageLayout, apply_aarch64_relocations};

mod bytes;
mod constants;
mod entry;
mod headers;
mod layout;
#[cfg(test)]
mod tests;

use constants::{
    ELF_HEADER_SIZE, IMAGE_BASE, PAGE_SIZE, PROGRAM_HEADER_COUNT, PROGRAM_HEADER_SIZE,
};
use entry::elf_entry_address;
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
