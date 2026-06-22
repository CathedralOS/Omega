use omega_core::diagnostics::Diagnostic;
use omega_image::{
    ExecutableImageOutput, FinalImage, FinalImageLayout, apply_aarch64_relocations,
    apply_x86_64_relocations,
};

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

// ELF e_machine values.
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;

pub fn emit_elf_aarch64_executable(
    image: FinalImage,
) -> Result<ExecutableImageOutput, Diagnostic> {
    emit_elf_executable(image, EM_AARCH64, "elf64-aarch64-executable", apply_aarch64_relocations)
}

pub fn emit_elf_x86_64_executable(image: FinalImage) -> Result<ExecutableImageOutput, Diagnostic> {
    emit_elf_executable(image, EM_X86_64, "elf64-x86-64-executable", apply_x86_64_relocations)
}

/// Shared ELF64 executable emitter. The ELF container is architecture-agnostic
/// apart from `e_machine` and the relocation application, both passed in.
fn emit_elf_executable(
    mut image: FinalImage,
    machine: u16,
    format: &str,
    apply_relocations: fn(&mut FinalImage, &FinalImageLayout, &str) -> Result<(), Diagnostic>,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let sections = plan_elf_sections(&image);
    let layout = sections.final_image_layout();
    let entry_address = elf_entry_address(&image, sections.text_address)?;

    apply_relocations(&mut image, &layout, "ELF direct image")?;

    let mut bytes = Vec::with_capacity(sections.data_offset + image.memory.data.len());
    write_elf_header(
        &mut bytes,
        machine,
        entry_address,
        sections.text_offset,
        sections.data_offset,
    );
    write_text_program_header(&mut bytes, sections.text_offset, image.memory.text.len());
    write_data_program_header(
        &mut bytes,
        sections.data_offset,
        image.memory.data.len(),
        sections.data_memory_size,
    );
    bytes.resize(sections.text_offset, 0);
    bytes.extend(&image.memory.text);
    bytes.resize(sections.data_offset, 0);
    bytes.extend(&image.memory.data);

    Ok(ExecutableImageOutput {
        bytes,
        file_name: "omega-program".to_owned(),
        format: format.to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
    })
}
