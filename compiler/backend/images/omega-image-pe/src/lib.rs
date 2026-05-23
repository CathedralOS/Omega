use omega_core::diagnostics::Diagnostic;
use omega_image::{
    ExecutableImageOutput, FinalImage, FinalImageLayout, FinalImageSection,
    apply_x86_64_relocations, final_image_symbol_name,
};

const DOS_HEADER_SIZE: usize = 0x80;
const COFF_HEADER_SIZE: usize = 20;
const OPTIONAL_HEADER_SIZE: usize = 240;
const SECTION_HEADER_SIZE: usize = 40;
const IMAGE_BASE: u64 = 0x1_4000_0000;
const SECTION_ALIGNMENT: usize = 0x1000;
const FILE_ALIGNMENT: usize = 0x200;
const TEXT_RVA: u32 = 0x1000;

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PeImportThunk {
    symbol: String,
    text_offset: usize,
}

fn install_import_thunks(image: &mut FinalImage) -> Vec<PeImportThunk> {
    let imports = image
        .imports
        .iter()
        .filter_map(|(_, import)| {
            image
                .symbols
                .is_valid(import.symbol_handle)
                .then_some(import.symbol_handle)
        })
        .collect::<Vec<_>>();
    let mut thunks = Vec::new();

    for symbol_handle in imports {
        let symbol = image.symbols.get(symbol_handle).name.clone();
        let text_offset = image.text.len();
        image.text.extend([0xff, 0x25, 0, 0, 0, 0]);

        let image_symbol = image.symbols.get_mut(symbol_handle);
        image_symbol.section = FinalImageSection::Text;
        image_symbol.offset = text_offset;
        image_symbol.size = 6;

        thunks.push(PeImportThunk {
            symbol,
            text_offset,
        });
    }

    thunks
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PeImportTable {
    bytes: Vec<u8>,
    iat_rvas: Vec<u32>,
    import_directory_rva: u32,
    import_directory_size: usize,
    iat_rva: u32,
    iat_size: usize,
}

fn build_import_table(imports: &[PeImportThunk], rdata_rva: u32) -> PeImportTable {
    let descriptor_offset = 0usize;
    let descriptor_count = usize::from(!imports.is_empty());
    let descriptor_table_size = (descriptor_count + 1) * 20;
    let ilt_offset = descriptor_table_size;
    let thunk_table_size = (imports.len() + 1) * 8;
    let iat_offset = ilt_offset + thunk_table_size;
    let dll_name_offset = iat_offset + thunk_table_size;
    let mut name_cursor = align_to(dll_name_offset + b"KERNEL32.dll\0".len(), 2);
    let mut hint_name_offsets = Vec::with_capacity(imports.len());

    for import in imports {
        hint_name_offsets.push(name_cursor);
        name_cursor = align_to(name_cursor + 2 + import.symbol.len() + 1, 2);
    }

    let mut bytes = vec![0; name_cursor];
    let ilt_rva = rdata_rva + ilt_offset as u32;
    let iat_rva = rdata_rva + iat_offset as u32;
    let dll_name_rva = rdata_rva + dll_name_offset as u32;

    if !imports.is_empty() {
        write_u32_at(&mut bytes, descriptor_offset, ilt_rva);
        write_u32_at(&mut bytes, descriptor_offset + 12, dll_name_rva);
        write_u32_at(&mut bytes, descriptor_offset + 16, iat_rva);
    }

    bytes[dll_name_offset..dll_name_offset + b"KERNEL32.dll\0".len()]
        .copy_from_slice(b"KERNEL32.dll\0");

    let mut iat_rvas = Vec::with_capacity(imports.len());
    for (index, import) in imports.iter().enumerate() {
        let hint_name_rva = rdata_rva + hint_name_offsets[index] as u32;
        write_u64_at(&mut bytes, ilt_offset + index * 8, u64::from(hint_name_rva));
        write_u64_at(&mut bytes, iat_offset + index * 8, u64::from(hint_name_rva));
        iat_rvas.push(iat_rva + (index * 8) as u32);

        let name_offset = hint_name_offsets[index];
        write_u16_at(&mut bytes, name_offset, 0);
        let symbol_start = name_offset + 2;
        bytes[symbol_start..symbol_start + import.symbol.len()]
            .copy_from_slice(import.symbol.as_bytes());
    }

    PeImportTable {
        bytes,
        iat_rvas,
        import_directory_rva: rdata_rva,
        import_directory_size: descriptor_table_size,
        iat_rva,
        iat_size: thunk_table_size,
    }
}

fn patch_import_thunks(
    image: &mut FinalImage,
    layout: &FinalImageLayout,
    thunks: &[PeImportThunk],
    iat_rvas: &[u32],
) -> Result<(), Diagnostic> {
    for (thunk, iat_rva) in thunks.iter().zip(iat_rvas.iter().copied()) {
        let instruction_address = layout.text_address + thunk.text_offset as u64;
        let next_instruction = instruction_address + 6;
        let iat_address = IMAGE_BASE + u64::from(iat_rva);
        let delta = iat_address as i64 - next_instruction as i64;
        let displacement = i32::try_from(delta).map_err(|_| {
            Diagnostic::error(format!(
                "PE x86_64 import thunk for `{}` is out of range",
                thunk.symbol
            ))
        })?;
        write_i32_at(&mut image.text, thunk.text_offset + 2, displacement)?;
    }

    Ok(())
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

struct PeHeaderInput {
    section_count: usize,
    entry_rva: u32,
    size_of_code: usize,
    size_of_initialized_data: usize,
    size_of_image: u32,
    size_of_headers: usize,
    import_directory_rva: u32,
    import_directory_size: usize,
    iat_rva: u32,
    iat_size: usize,
}

fn write_dos_header(bytes: &mut Vec<u8>) {
    bytes.extend([b'M', b'Z']);
    bytes.resize(0x3c, 0);
    write_u32(bytes, DOS_HEADER_SIZE as u32);
    bytes.resize(DOS_HEADER_SIZE, 0);
}

fn write_pe_headers(bytes: &mut Vec<u8>, input: PeHeaderInput) {
    bytes.extend(b"PE\0\0");
    write_u16(bytes, 0x8664);
    write_u16(bytes, input.section_count as u16);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u16(bytes, OPTIONAL_HEADER_SIZE as u16);
    write_u16(bytes, 0x0022);

    write_u16(bytes, 0x20b);
    bytes.push(0);
    bytes.push(0);
    write_u32(bytes, input.size_of_code as u32);
    write_u32(bytes, input.size_of_initialized_data as u32);
    write_u32(bytes, 0);
    write_u32(bytes, input.entry_rva);
    write_u32(bytes, TEXT_RVA);
    write_u64(bytes, IMAGE_BASE);
    write_u32(bytes, SECTION_ALIGNMENT as u32);
    write_u32(bytes, FILE_ALIGNMENT as u32);
    write_u16(bytes, 6);
    write_u16(bytes, 0);
    write_u16(bytes, 0);
    write_u16(bytes, 0);
    write_u16(bytes, 6);
    write_u16(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, input.size_of_image);
    write_u32(bytes, input.size_of_headers as u32);
    write_u32(bytes, 0);
    write_u16(bytes, 3);
    write_u16(bytes, 0x0100);
    write_u64(bytes, 0x100000);
    write_u64(bytes, 0x1000);
    write_u64(bytes, 0x100000);
    write_u64(bytes, 0x1000);
    write_u32(bytes, 0);
    write_u32(bytes, 16);

    for directory_index in 0..16 {
        match directory_index {
            1 => {
                write_u32(bytes, input.import_directory_rva);
                write_u32(bytes, input.import_directory_size as u32);
            }
            12 => {
                write_u32(bytes, input.iat_rva);
                write_u32(bytes, input.iat_size as u32);
            }
            _ => {
                write_u32(bytes, 0);
                write_u32(bytes, 0);
            }
        }
    }
}

fn write_section_header(
    bytes: &mut Vec<u8>,
    name: &str,
    virtual_size: usize,
    virtual_address: u32,
    raw_size: usize,
    raw_pointer: usize,
    characteristics: u32,
) {
    let mut name_bytes = [0u8; 8];
    let source_name = name.as_bytes();
    let copy_len = source_name.len().min(name_bytes.len());
    name_bytes[..copy_len].copy_from_slice(&source_name[..copy_len]);
    bytes.extend(name_bytes);
    write_u32(bytes, virtual_size as u32);
    write_u32(bytes, virtual_address);
    write_u32(bytes, raw_size as u32);
    write_u32(bytes, raw_pointer as u32);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u16(bytes, 0);
    write_u16(bytes, 0);
    write_u32(bytes, characteristics);
}

fn write_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend(value.to_le_bytes());
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}

fn write_u16_at(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32_at(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64_at(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_i32_at(bytes: &mut [u8], offset: usize, value: i32) -> Result<(), Diagnostic> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| Diagnostic::error("PE x86_64 patch offset overflow"))?;
    let Some(slice) = bytes.get_mut(offset..end) else {
        return Err(Diagnostic::error(format!(
            "PE x86_64 patch offset {offset} is outside text section"
        )));
    };

    slice.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

fn align_to_u32(value: u32, alignment: usize) -> u32 {
    let alignment = u32::try_from(alignment.max(1)).expect("alignment overflow");
    value.div_ceil(alignment) * alignment
}
