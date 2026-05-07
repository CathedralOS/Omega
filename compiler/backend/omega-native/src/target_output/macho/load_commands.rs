use super::bytes::{write_fixed_string_16, write_u32, write_u64};
use super::constants::{
    MACHO_ARM64_PAGE_SIZE, MACHO_CODE_SIGNATURE_COMMAND_SIZE, MACHO_DYSYMTAB_COMMAND_SIZE,
    MACHO_EXECUTABLE_BASE, MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE,
    MACHO_HEADER_FLAGS_DYLDLINK, MACHO_HEADER_FLAGS_NOUNDEFS, MACHO_HEADER_FLAGS_PIE,
    MACHO_HEADER_FLAGS_TWOLEVEL, MACHO_LOAD_DYLINKER_COMMAND_SIZE,
    MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE, MACHO_MAIN_COMMAND_SIZE, MACHO_SECTION_SIZE,
    MACHO_SEGMENT_COMMAND_SIZE, MACHO_SYMTAB_COMMAND_SIZE,
};
use super::layout::{align_to_u64, alignment_power};

pub(super) fn write_macho_executable_header(
    bytes: &mut Vec<u8>,
    command_count: usize,
    sizeofcmds: usize,
) {
    write_macho_header_for(
        bytes,
        2,
        command_count,
        sizeofcmds,
        MACHO_HEADER_FLAGS_NOUNDEFS
            | MACHO_HEADER_FLAGS_DYLDLINK
            | MACHO_HEADER_FLAGS_TWOLEVEL
            | MACHO_HEADER_FLAGS_PIE,
    );
}

fn write_macho_header_for(
    bytes: &mut Vec<u8>,
    file_type: u32,
    command_count: usize,
    sizeofcmds: usize,
    flags: u32,
) {
    write_u32(bytes, 0xfeedfacf);
    write_u32(bytes, 0x0100000c);
    write_u32(bytes, 0);
    write_u32(bytes, file_type);
    write_u32(
        bytes,
        u32::try_from(command_count).expect("Mach-O command count overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(sizeofcmds).expect("Mach-O load command size overflow"),
    );
    write_u32(bytes, flags);
    write_u32(bytes, 0);
}

pub(super) fn write_macho_pagezero_segment(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x19);
    write_u32(bytes, MACHO_SEGMENT_COMMAND_SIZE as u32);
    write_fixed_string_16(bytes, "__PAGEZERO");
    write_u64(bytes, 0);
    write_u64(bytes, MACHO_EXECUTABLE_BASE);
    write_u64(bytes, 0);
    write_u64(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

pub(super) fn write_macho_executable_text_segment(
    bytes: &mut Vec<u8>,
    text_offset: usize,
    text_size: usize,
    text_file_size: usize,
) {
    write_u32(bytes, 0x19);
    write_u32(
        bytes,
        u32::try_from(MACHO_SEGMENT_COMMAND_SIZE + MACHO_SECTION_SIZE)
            .expect("Mach-O text segment command size overflow"),
    );
    write_fixed_string_16(bytes, "__TEXT");
    write_u64(bytes, MACHO_EXECUTABLE_BASE);
    write_u64(
        bytes,
        align_to_u64(text_file_size as u64, MACHO_ARM64_PAGE_SIZE as u64),
    );
    write_u64(bytes, 0);
    write_u64(
        bytes,
        u64::try_from(text_file_size).expect("Mach-O text file size overflow"),
    );
    write_u32(bytes, 5);
    write_u32(bytes, 5);
    write_u32(bytes, 1);
    write_u32(bytes, 0);

    write_fixed_string_16(bytes, "__text");
    write_fixed_string_16(bytes, "__TEXT");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + text_offset as u64);
    write_u64(
        bytes,
        u64::try_from(text_size).expect("Mach-O text size overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(text_offset).expect("Mach-O text offset overflow"),
    );
    write_u32(bytes, 2);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0x80000400);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

pub(super) fn write_macho_executable_data_segment(
    bytes: &mut Vec<u8>,
    data_offset: usize,
    data_size: usize,
    bss_size: usize,
    data_vm_size: u64,
    bss_alignment: usize,
) {
    let section_count = usize::from(data_size > 0) + usize::from(bss_size > 0);
    write_u32(bytes, 0x19);
    write_u32(
        bytes,
        u32::try_from(MACHO_SEGMENT_COMMAND_SIZE + section_count * MACHO_SECTION_SIZE)
            .expect("Mach-O data segment command size overflow"),
    );
    write_fixed_string_16(bytes, "__DATA");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + data_offset as u64);
    write_u64(bytes, data_vm_size);
    write_u64(
        bytes,
        u64::try_from(data_offset).expect("Mach-O data file offset overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(data_size).expect("Mach-O data file size overflow"),
    );
    write_u32(bytes, 3);
    write_u32(bytes, 3);
    write_u32(
        bytes,
        u32::try_from(section_count).expect("Mach-O data section count overflow"),
    );
    write_u32(bytes, 0);

    if data_size > 0 {
        write_macho_executable_data_section(bytes, data_offset, data_size);
    }
    if bss_size > 0 {
        write_macho_executable_bss_section(bytes, data_offset + data_size, bss_size, bss_alignment);
    }
}

fn write_macho_executable_data_section(bytes: &mut Vec<u8>, data_offset: usize, data_size: usize) {
    write_fixed_string_16(bytes, "__data");
    write_fixed_string_16(bytes, "__DATA");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + data_offset as u64);
    write_u64(
        bytes,
        u64::try_from(data_size).expect("Mach-O data section size overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(data_offset).expect("Mach-O data section offset overflow"),
    );
    write_u32(bytes, 3);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_executable_bss_section(
    bytes: &mut Vec<u8>,
    bss_address_offset: usize,
    bss_size: usize,
    bss_alignment: usize,
) {
    write_fixed_string_16(bytes, "__bss");
    write_fixed_string_16(bytes, "__DATA");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + bss_address_offset as u64);
    write_u64(
        bytes,
        u64::try_from(bss_size).expect("Mach-O bss section size overflow"),
    );
    write_u32(bytes, 0);
    write_u32(bytes, alignment_power(bss_alignment));
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 1);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

pub(super) fn write_macho_load_dylinker_command(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    write_u32(bytes, 0xe);
    write_u32(bytes, MACHO_LOAD_DYLINKER_COMMAND_SIZE as u32);
    write_u32(bytes, 12);
    bytes.extend(b"/usr/lib/dyld\0");
    bytes.resize(start + MACHO_LOAD_DYLINKER_COMMAND_SIZE, 0);
}

pub(super) fn write_macho_main_command(bytes: &mut Vec<u8>, entry_offset: usize) {
    write_u32(bytes, 0x80000028);
    write_u32(bytes, MACHO_MAIN_COMMAND_SIZE as u32);
    write_u64(
        bytes,
        u64::try_from(entry_offset).expect("Mach-O entry offset overflow"),
    );
    write_u64(bytes, 0);
}

pub(super) fn write_macho_executable_build_version_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x32);
    write_u32(bytes, MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE as u32);
    write_u32(bytes, 1);
    write_u32(bytes, 13 << 16);
    write_u32(bytes, 13 << 16);
    write_u32(bytes, 1);
    write_u32(bytes, 3);
    write_u32(bytes, 0);
}

pub(super) fn write_macho_load_libsystem_command(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    write_u32(bytes, 0xc);
    write_u32(bytes, MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE as u32);
    write_u32(bytes, 24);
    write_u32(bytes, 2);
    write_u32(bytes, 1351 << 16);
    write_u32(bytes, 1 << 16);
    bytes.extend(b"/usr/lib/libSystem.B.dylib\0");
    bytes.resize(start + MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE, 0);
}

pub(super) fn write_macho_linkedit_segment(
    bytes: &mut Vec<u8>,
    vmaddr: u64,
    file_offset: usize,
    file_size: usize,
    vm_size: usize,
) {
    write_u32(bytes, 0x19);
    write_u32(bytes, MACHO_SEGMENT_COMMAND_SIZE as u32);
    write_fixed_string_16(bytes, "__LINKEDIT");
    write_u64(bytes, vmaddr);
    write_u64(
        bytes,
        u64::try_from(vm_size).expect("Mach-O LINKEDIT vm size overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(file_offset).expect("Mach-O LINKEDIT file offset overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(file_size).expect("Mach-O LINKEDIT file size overflow"),
    );
    write_u32(bytes, 1);
    write_u32(bytes, 1);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

pub(super) fn write_macho_code_signature_command(
    bytes: &mut Vec<u8>,
    code_signature_offset: usize,
    code_signature_size: usize,
) {
    write_u32(bytes, 0x1d);
    write_u32(bytes, MACHO_CODE_SIGNATURE_COMMAND_SIZE as u32);
    write_u32(
        bytes,
        u32::try_from(code_signature_offset).expect("Mach-O code signature offset overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(code_signature_size).expect("Mach-O code signature size overflow"),
    );
}

pub(super) fn write_empty_macho_symtab_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x2);
    write_u32(bytes, MACHO_SYMTAB_COMMAND_SIZE as u32);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

pub(super) fn write_empty_macho_dysymtab_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0xb);
    write_u32(bytes, MACHO_DYSYMTAB_COMMAND_SIZE as u32);
    for _ in 0..18 {
        write_u32(bytes, 0);
    }
}
