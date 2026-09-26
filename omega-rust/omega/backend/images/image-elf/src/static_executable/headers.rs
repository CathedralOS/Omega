//! The static lane's ELF header and its two program headers, written straight into
//! the output buffer with no intermediate structure.

use crate::bytes::{write_u16, write_u32, write_u64};
use crate::constants::{
    ELF_HEADER_SIZE, IMAGE_BASE, PAGE_SIZE, PROGRAM_HEADER_COUNT, PROGRAM_HEADER_SIZE,
};

pub(crate) fn write_elf_header(
    bytes: &mut Vec<u8>,
    machine: u16,
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
    // e_machine: 183 = EM_AARCH64, 62 = EM_X86_64.
    write_u16(bytes, machine);
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

pub(crate) fn write_text_program_header(bytes: &mut Vec<u8>, text_offset: usize, text_size: usize) {
    write_program_header(
        bytes,
        5,
        0,
        IMAGE_BASE,
        u64::try_from(text_offset + text_size).expect("ELF text segment size overflow"),
        u64::try_from(text_offset + text_size).expect("ELF text memory size overflow"),
    );
}

pub(crate) fn write_data_program_header(
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
