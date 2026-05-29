use omega_core::diagnostics::Diagnostic;
use omega_image::{ExecutableImageOutput, FinalImage, apply_aarch64_relocations};

mod bytes;
mod constants;
mod entry;
mod headers;
mod layout;
mod sections;
#[cfg(test)]
mod tests;

use entry::elf_entry_address;
use headers::{write_data_program_header, write_elf_header, write_text_program_header};
use sections::plan_elf_sections;

pub fn emit_elf_aarch64_executable(
    mut image: FinalImage,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let sections = plan_elf_sections(&image);
    let layout = sections.final_image_layout();
    let entry_address = elf_entry_address(&image, sections.text_address)?;

    apply_aarch64_relocations(&mut image, &layout, "ELF direct image")?;

    let mut bytes = Vec::with_capacity(sections.data_offset + image.data.len());
    write_elf_header(
        &mut bytes,
        entry_address,
        sections.text_offset,
        sections.data_offset,
    );
    write_text_program_header(&mut bytes, sections.text_offset, image.text.len());
    write_data_program_header(
        &mut bytes,
        sections.data_offset,
        image.data.len(),
        sections.data_memory_size,
    );
    bytes.resize(sections.text_offset, 0);
    bytes.extend(&image.text);
    bytes.resize(sections.data_offset, 0);
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
