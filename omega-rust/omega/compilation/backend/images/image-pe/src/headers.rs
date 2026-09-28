//! The DOS stub, the COFF and optional headers, and one section header per emitted
//! section.

use crate::bytes::{write_u16, write_u32, write_u64};
use crate::constants::{
    DOS_HEADER_SIZE, FILE_ALIGNMENT, IMAGE_BASE, OPTIONAL_HEADER_SIZE, SECTION_ALIGNMENT, TEXT_RVA,
};

pub(crate) struct PeHeaderInput {
    pub(crate) section_count: usize,
    pub(crate) entry_rva: u32,
    pub(crate) size_of_code: usize,
    pub(crate) size_of_initialized_data: usize,
    pub(crate) size_of_image: u32,
    pub(crate) size_of_headers: usize,
    pub(crate) import_directory_rva: u32,
    pub(crate) import_directory_size: usize,
    pub(crate) iat_rva: u32,
    pub(crate) iat_size: usize,
    /// Base-relocation (`.reloc`) directory RVA + size; zero when absent.
    pub(crate) reloc_directory_rva: u32,
    pub(crate) reloc_directory_size: usize,
    /// Whether a `.reloc` section is present (drives the DYNAMICBASE flag).
    pub(crate) has_reloc: bool,
    /// PE optional-header Subsystem: console 3, gui 2, efi_application 10
    /// (`subsystem <word>` in the target block; console is the default).
    pub(crate) subsystem: u16,
}

pub(crate) fn write_dos_header(bytes: &mut Vec<u8>) {
    bytes.extend(*b"MZ");
    bytes.resize(0x3c, 0);
    write_u32(bytes, DOS_HEADER_SIZE as u32);
    bytes.resize(DOS_HEADER_SIZE, 0);
}

pub(crate) fn write_pe_headers(bytes: &mut Vec<u8>, input: PeHeaderInput) {
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
    write_u16(bytes, input.subsystem);
    // DllCharacteristics: NX_COMPAT (0x0100) always; DYNAMICBASE (0x0040)
    // when a `.reloc` section is present, which lets the loader place the
    // image at any base (Windows ASLR; UEFI arbitrary base) -- and makes
    // every run of a relocatable image a live check of the `.reloc` data.
    let dll_characteristics: u16 = 0x0100 | if input.has_reloc { 0x0040 } else { 0 };
    write_u16(bytes, dll_characteristics);
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
            5 => {
                write_u32(bytes, input.reloc_directory_rva);
                write_u32(bytes, input.reloc_directory_size as u32);
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

pub(crate) fn write_section_header(
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
