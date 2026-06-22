use crate::bytes::{write_fixed_string_16, write_u32, write_u64};
use crate::constants::{
    MACHO_ARM64_PAGE_SIZE, MACHO_EXECUTABLE_BASE, MACHO_SECTION_SIZE, MACHO_SEGMENT_COMMAND_SIZE,
};
use crate::layout::{align_to_u64, alignment_power};

pub(crate) fn write_macho_pagezero_segment(bytes: &mut Vec<u8>) {
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

pub(crate) fn write_macho_executable_text_segment(
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

pub(crate) fn write_macho_executable_data_segment(
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

pub(crate) fn write_macho_linkedit_segment(
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
