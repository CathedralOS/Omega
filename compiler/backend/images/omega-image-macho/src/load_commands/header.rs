use crate::bytes::write_u32;
use crate::constants::{
    MACHO_HEADER_FLAGS_DYLDLINK, MACHO_HEADER_FLAGS_NOUNDEFS, MACHO_HEADER_FLAGS_PIE,
    MACHO_HEADER_FLAGS_TWOLEVEL,
};

pub(crate) fn write_macho_executable_header(
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

pub(crate) fn write_macho_uuid_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x1b);
    write_u32(bytes, crate::constants::MACHO_UUID_COMMAND_SIZE as u32);
    bytes.extend([
        0x4f, 0x6d, 0x65, 0x67, 0x61, 0x2d, 0x64, 0x69, 0x72, 0x65, 0x63, 0x74, 0x2d, 0x6d, 0x30,
        0x31,
    ]);
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
