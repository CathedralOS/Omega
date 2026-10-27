//! The static lane: an ELF64 executable with a 64-byte header, two program
//! headers and no section headers, refused outright if the image references
//! any import. `headers.rs` writes the header bytes, `sections.rs` and
//! `layout.rs` place `.text`, `.data` and `.bss`, and the shared
//! `entry_symbol.rs` resolves the entry address.

use diagnostics::Diagnostic;
use image::{
    ExecutableImageOutput, FinalImage, PlacedDataRegionInventory, apply_relocations,
    place_data_regions, place_executable_regions,
};

mod headers;
mod layout;
mod sections;
#[cfg(test)]
mod tests;

use crate::entry_symbol::elf_entry_address;
use crate::imports::{ElfImportLocator, canonical_referenced_imports};
use headers::{write_data_program_header, write_elf_header, write_text_program_header};
use sections::plan_elf_sections;

// ELF e_machine values.
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;

/// Emit the static ELF64 executable for the image's own target: `e_machine` and
/// the published format name are read off it, and a target off the two hosted
/// ELF diagonals is refused before any byte is written.
pub fn emit_elf_executable(mut image: FinalImage) -> Result<ExecutableImageOutput, Diagnostic> {
    let (machine, format) = match (image.target.object_format, image.target.architecture) {
        (target::ObjectFormat::Elf, target::Architecture::Aarch64) => {
            (EM_AARCH64, "elf64-aarch64-executable")
        }
        (target::ObjectFormat::Elf, target::Architecture::X86_64) => {
            (EM_X86_64, "elf64-x86-64-executable")
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "ELF64 executable emission cannot supply an image whose target is {:?}",
                image.target,
            )));
        }
    };
    let imports = canonical_referenced_imports(&image)?;
    if let Some(import) = imports.first() {
        let message = match &import.locator {
            ElfImportLocator::StringBackedBootstrap { library, symbol } => format!(
                "ELF direct image relocation references unknown symbol `{symbol}`; canonical dynamic import request names library `{library}` at {} relocation site(s), but ELF loader binding is not implemented",
                import.relocations.len(),
            ),
            ElfImportLocator::Component { locator, .. } => format!(
                "component ELF image {} requires the selected loader and separate-image publication; static emission cannot resolve this slot",
                locator.image_filename(),
            ),
            ElfImportLocator::Versioned {
                target_profile,
                compatibility_report_identity,
                object,
                symbol,
                version,
            } => format!(
                "versioned ELF foreign locator report 0x{compatibility_report_identity:016x} for target `{}` reached final emission with object {}, symbol {}, version {}, and {} exact relocation site(s); runnable dynamic ELF emission remains fail-closed before image mutation because no target-owned ELF loader plan carries the exact PT_INTERP bytes",
                target_profile.target_name(),
                hex_bytes(object),
                hex_bytes(symbol),
                hex_bytes(version),
                import.relocations.len(),
            ),
        };
        return Err(Diagnostic::error(message));
    }
    let sections = plan_elf_sections(&image);
    let layout = sections.final_image_layout();
    let entry_address = elf_entry_address(&image, sections.text_address)?;

    apply_relocations(&mut image, &layout, "ELF direct image")?;
    let executable_regions = place_executable_regions(&image, layout)?;
    let data_regions = place_data_regions(&image, layout)?;

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
        final_image_layout: layout,
        final_text_bytes: image.memory.text.clone(),
        final_data_bytes: image.memory.data.clone(),
        final_import_data_bytes: Vec::new(),
        bytes,
        file_name: "omega-program".to_owned(),
        format: format.to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
        data_regions,
        import_data_regions: PlacedDataRegionInventory::empty(),
    })
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len().saturating_mul(2).saturating_add(2));
    rendered.push_str("0x");
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}").expect("writing to String cannot fail");
    }
    rendered
}
