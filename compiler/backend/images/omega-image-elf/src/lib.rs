use omega_core::diagnostics::Diagnostic;
use omega_image::{ExecutableImageOutput, FinalImage, FinalImageLayout, apply_aarch64_relocations};

const ELF_HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: usize = 56;
const PROGRAM_HEADER_COUNT: usize = 2;
const IMAGE_BASE: u64 = 0x400000;
const PAGE_SIZE: usize = 0x1000;

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

    apply_aarch64_relocations(&mut image, &layout, "ELF direct image")?;

    let data_memory_size = (bss_address - data_address)
        .checked_add(image.bss_size as u64)
        .expect("ELF data memory size overflow");
    let mut bytes = Vec::with_capacity(data_offset + image.data.len());
    write_elf_header(&mut bytes, text_address, text_offset, data_offset);
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

fn write_elf_header(
    bytes: &mut Vec<u8>,
    entry_address: u64,
    text_offset: usize,
    data_offset: usize,
) {
    bytes.extend([0x7f, b'E', b'L', b'F']);
    bytes.push(2);
    bytes.push(1);
    bytes.push(1);
    bytes.push(0);
    bytes.extend([0; 8]);
    write_u16(bytes, 2);
    write_u16(bytes, 183);
    write_u32(bytes, 1);
    write_u64(bytes, entry_address);
    write_u64(bytes, ELF_HEADER_SIZE as u64);
    write_u64(bytes, 0);
    write_u32(bytes, 0);
    write_u16(bytes, ELF_HEADER_SIZE as u16);
    write_u16(bytes, PROGRAM_HEADER_SIZE as u16);
    write_u16(bytes, PROGRAM_HEADER_COUNT as u16);
    write_u16(bytes, 0);
    write_u16(bytes, 0);
    write_u16(bytes, 0);

    debug_assert_eq!(bytes.len(), ELF_HEADER_SIZE);
    debug_assert!(text_offset >= bytes.len());
    debug_assert!(data_offset > text_offset);
}

fn write_text_program_header(bytes: &mut Vec<u8>, text_offset: usize, text_size: usize) {
    write_program_header(
        bytes,
        5,
        0,
        IMAGE_BASE,
        u64::try_from(text_offset + text_size).expect("ELF text segment size overflow"),
        u64::try_from(text_offset + text_size).expect("ELF text memory size overflow"),
    );
}

fn write_data_program_header(
    bytes: &mut Vec<u8>,
    data_offset: usize,
    data_size: usize,
    data_memory_size: u64,
) {
    write_program_header(
        bytes,
        6,
        u64::try_from(data_offset).expect("ELF data offset overflow"),
        IMAGE_BASE + data_offset as u64,
        u64::try_from(data_size).expect("ELF data segment size overflow"),
        data_memory_size,
    );
}

fn write_program_header(
    bytes: &mut Vec<u8>,
    flags: u32,
    offset: u64,
    virtual_address: u64,
    file_size: u64,
    memory_size: u64,
) {
    write_u32(bytes, 1);
    write_u32(bytes, flags);
    write_u64(bytes, offset);
    write_u64(bytes, virtual_address);
    write_u64(bytes, virtual_address);
    write_u64(bytes, file_size);
    write_u64(bytes, memory_size);
    write_u64(bytes, PAGE_SIZE as u64);
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

fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

fn align_to_u64(value: u64, alignment: u64) -> u64 {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}
