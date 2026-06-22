use crate::bytes::{write_u32, write_u64};
use crate::constants::{
    MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE, MACHO_LOAD_DYLINKER_COMMAND_SIZE,
    MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE, MACHO_MAIN_COMMAND_SIZE,
};

pub(crate) fn write_macho_load_dylinker_command(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    write_u32(bytes, 0xe);
    write_u32(bytes, MACHO_LOAD_DYLINKER_COMMAND_SIZE as u32);
    write_u32(bytes, 12);
    bytes.extend(b"/usr/lib/dyld\0");
    bytes.resize(start + MACHO_LOAD_DYLINKER_COMMAND_SIZE, 0);
}

pub(crate) fn write_macho_main_command(bytes: &mut Vec<u8>, entry_offset: usize) {
    write_u32(bytes, 0x80000028);
    write_u32(bytes, MACHO_MAIN_COMMAND_SIZE as u32);
    write_u64(
        bytes,
        u64::try_from(entry_offset).expect("Mach-O entry offset overflow"),
    );
    write_u64(bytes, 0);
}

pub(crate) fn write_macho_executable_build_version_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x32);
    write_u32(bytes, MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE as u32);
    write_u32(bytes, 1);
    write_u32(bytes, 13 << 16);
    write_u32(bytes, 13 << 16);
    write_u32(bytes, 1);
    write_u32(bytes, 3);
    write_u32(bytes, 0);
}

pub(crate) fn write_macho_load_libsystem_command(bytes: &mut Vec<u8>) {
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
