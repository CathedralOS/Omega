use omega_core::diagnostics::Diagnostic;
use omega_image::{
    ExecutableImageOutput, FinalImage, FinalImageLayout, FinalImageSection,
    apply_x86_64_relocations, final_image_symbol_name,
};

mod bytes;
mod constants;
mod headers;
mod imports;
mod layout;

use constants::{
    COFF_HEADER_SIZE, DOS_HEADER_SIZE, FILE_ALIGNMENT, IMAGE_BASE, OPTIONAL_HEADER_SIZE,
    SECTION_ALIGNMENT, SECTION_HEADER_SIZE, TEXT_RVA,
};
use headers::{PeHeaderInput, write_dos_header, write_pe_headers, write_section_header};
use imports::{build_import_table, install_import_thunks, patch_import_thunks};
use layout::{align_to, align_to_u32};

pub fn emit_pe_x86_64_executable(
    mut image: FinalImage,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let import_thunks = install_import_thunks(&mut image);
    let text_virtual_size = image.text.len();
    let rdata_rva = align_to_u32(TEXT_RVA + text_virtual_size as u32, SECTION_ALIGNMENT);
    let import_table = build_import_table(&import_thunks, rdata_rva);
    let rdata_virtual_size = import_table.bytes.len();
    let data_rva = align_to_u32(rdata_rva + rdata_virtual_size as u32, SECTION_ALIGNMENT);
    let bss_rva = align_to_u32(data_rva + image.data.len() as u32, SECTION_ALIGNMENT);
    let layout = FinalImageLayout {
        text_address: IMAGE_BASE + u64::from(TEXT_RVA),
        data_address: IMAGE_BASE + u64::from(data_rva),
        bss_address: IMAGE_BASE + u64::from(bss_rva),
    };

    patch_import_thunks(&mut image, &layout, &import_thunks, &import_table.iat_rvas)?;
    apply_x86_64_relocations(&mut image, &layout, "PE direct executable")?;

    let has_data = !image.data.is_empty();
    let has_bss = image.bss_size > 0;
    let section_count = 2 + usize::from(has_data) + usize::from(has_bss);
    let headers_size = align_to(
        DOS_HEADER_SIZE
            + 4
            + COFF_HEADER_SIZE
            + OPTIONAL_HEADER_SIZE
            + section_count * SECTION_HEADER_SIZE,
        FILE_ALIGNMENT,
    );
    let text_raw_size = align_to(image.text.len(), FILE_ALIGNMENT);
    let rdata_raw_size = align_to(import_table.bytes.len(), FILE_ALIGNMENT);
    let data_raw_size = align_to(image.data.len(), FILE_ALIGNMENT);
    let text_raw = headers_size;
    let rdata_raw = text_raw + text_raw_size;
    let data_raw = rdata_raw + rdata_raw_size;
    let size_of_image = align_to_u32(bss_rva + image.bss_size as u32, SECTION_ALIGNMENT);
    let entry_rva = pe_entry_rva(&image)?;

    let mut bytes = Vec::new();
    write_dos_header(&mut bytes);
    write_pe_headers(
        &mut bytes,
        PeHeaderInput {
            section_count,
            entry_rva,
            size_of_code: text_raw_size,
            size_of_initialized_data: rdata_raw_size + data_raw_size,
            size_of_image,
            size_of_headers: headers_size,
            import_directory_rva: import_table.import_directory_rva,
            import_directory_size: import_table.import_directory_size,
            iat_rva: import_table.iat_rva,
            iat_size: import_table.iat_size,
        },
    );
    write_section_header(
        &mut bytes,
        ".text",
        text_virtual_size,
        TEXT_RVA,
        text_raw_size,
        text_raw,
        0x6000_0020,
    );
    write_section_header(
        &mut bytes,
        ".rdata",
        rdata_virtual_size,
        rdata_rva,
        rdata_raw_size,
        rdata_raw,
        0x4000_0040,
    );
    if has_data {
        write_section_header(
            &mut bytes,
            ".data",
            image.data.len(),
            data_rva,
            data_raw_size,
            data_raw,
            0xc000_0040,
        );
    }
    if has_bss {
        write_section_header(
            &mut bytes,
            ".bss",
            image.bss_size,
            bss_rva,
            0,
            0,
            0xc000_0080,
        );
    }

    bytes.resize(text_raw, 0);
    bytes.extend(&image.text);
    bytes.resize(text_raw + text_raw_size, 0);
    bytes.resize(rdata_raw, 0);
    bytes.extend(&import_table.bytes);
    bytes.resize(rdata_raw + rdata_raw_size, 0);
    if has_data {
        bytes.resize(data_raw, 0);
        bytes.extend(&image.data);
        bytes.resize(data_raw + data_raw_size, 0);
    }

    Ok(ExecutableImageOutput {
        bytes,
        file_name: "omega-program.exe".to_owned(),
        format: "pe64-x86_64-executable".to_owned(),
        text_bytes: image.text.len(),
        data_bytes: image.data.len(),
        bss_bytes: image.bss_size,
        symbols: image.symbols.len(),
        imports: image.imports.len(),
        relocations: image.relocations.len(),
    })
}

fn pe_entry_rva(image: &FinalImage) -> Result<u32, Diagnostic> {
    let entry_symbol = image
        .symbols
        .is_valid(image.entry_symbol)
        .then(|| image.symbols.get(image.entry_symbol))
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "PE entry symbol `{}` is missing from the final image",
                final_image_symbol_name(image, image.entry_symbol)
            ))
        })?;

    if entry_symbol.section != FinalImageSection::Text {
        return Err(Diagnostic::error(format!(
            "PE entry symbol `{}` is not in the text section",
            final_image_symbol_name(image, image.entry_symbol)
        )));
    }

    Ok(TEXT_RVA + entry_symbol.offset as u32)
}
