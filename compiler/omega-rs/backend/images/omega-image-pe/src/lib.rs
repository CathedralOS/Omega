use omega_core::diagnostics::Diagnostic;
use omega_image::{ExecutableImageOutput, FinalImage, apply_x86_64_relocations};

mod bytes;
mod constants;
mod entry;
mod headers;
mod imports;
mod layout;
mod sections;

use constants::TEXT_RVA;
use entry::pe_entry_rva;
use headers::{PeHeaderInput, write_dos_header, write_pe_headers, write_section_header};
use imports::{build_import_table, install_import_thunks, patch_import_thunks};
use sections::plan_pe_sections;

pub fn emit_pe_x86_64_executable(
    mut image: FinalImage,
    subsystem: u16,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let import_thunks = install_import_thunks(&mut image);
    let initial_sections = plan_pe_sections(&image, 0);
    let import_table = build_import_table(&import_thunks, initial_sections.rdata_rva);
    let sections = plan_pe_sections(&image, import_table.bytes.len());
    let layout = sections.final_image_layout();

    patch_import_thunks(&mut image, &layout, &import_thunks, &import_table.iat_rvas)?;
    apply_x86_64_relocations(&mut image, &layout, "PE direct executable")?;

    let entry_rva = pe_entry_rva(&image)?;

    let mut bytes = Vec::new();
    write_dos_header(&mut bytes);
    write_pe_headers(
        &mut bytes,
        PeHeaderInput {
            section_count: sections.section_count,
            entry_rva,
            size_of_code: sections.text_raw_size,
            size_of_initialized_data: sections.rdata_raw_size + sections.data_raw_size,
            size_of_image: sections.size_of_image,
            size_of_headers: sections.headers_size,
            import_directory_rva: import_table.import_directory_rva,
            import_directory_size: import_table.import_directory_size,
            iat_rva: import_table.iat_rva,
            iat_size: import_table.iat_size,
            subsystem,
        },
    );
    write_section_header(
        &mut bytes,
        ".text",
        sections.text_virtual_size,
        TEXT_RVA,
        sections.text_raw_size,
        sections.text_raw,
        0x6000_0020,
    );
    write_section_header(
        &mut bytes,
        ".rdata",
        sections.rdata_virtual_size,
        sections.rdata_rva,
        sections.rdata_raw_size,
        sections.rdata_raw,
        0x4000_0040,
    );
    if sections.has_data {
        write_section_header(
            &mut bytes,
            ".data",
            image.memory.data.len(),
            sections.data_rva,
            sections.data_raw_size,
            sections.data_raw,
            0xc000_0040,
        );
    }
    if sections.has_bss {
        write_section_header(
            &mut bytes,
            ".bss",
            image.memory.bss_size,
            sections.bss_rva,
            0,
            0,
            0xc000_0080,
        );
    }

    bytes.resize(sections.text_raw, 0);
    bytes.extend(&image.memory.text);
    bytes.resize(sections.text_raw + sections.text_raw_size, 0);
    bytes.resize(sections.rdata_raw, 0);
    bytes.extend(&import_table.bytes);
    bytes.resize(sections.rdata_raw + sections.rdata_raw_size, 0);
    if sections.has_data {
        bytes.resize(sections.data_raw, 0);
        bytes.extend(&image.memory.data);
        bytes.resize(sections.data_raw + sections.data_raw_size, 0);
    }

    Ok(ExecutableImageOutput {
        bytes,
        file_name: "omega-program.exe".to_owned(),
        format: "pe64-x86_64-executable".to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
    })
}
