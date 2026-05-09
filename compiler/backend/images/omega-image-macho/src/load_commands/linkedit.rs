use crate::bytes::write_u32;
use crate::constants::{
    MACHO_CODE_SIGNATURE_COMMAND_SIZE, MACHO_DYLD_INFO_COMMAND_SIZE, MACHO_DYSYMTAB_COMMAND_SIZE,
    MACHO_SYMTAB_COMMAND_SIZE,
};

pub(crate) fn write_macho_code_signature_command(
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

pub(crate) fn write_empty_macho_symtab_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x2);
    write_u32(bytes, MACHO_SYMTAB_COMMAND_SIZE as u32);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

pub(crate) fn write_empty_macho_dysymtab_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0xb);
    write_u32(bytes, MACHO_DYSYMTAB_COMMAND_SIZE as u32);
    for _ in 0..18 {
        write_u32(bytes, 0);
    }
}

pub(crate) fn write_macho_dyld_info_command(
    bytes: &mut Vec<u8>,
    bind_offset: usize,
    bind_size: usize,
) {
    write_u32(bytes, 0x8000_0022);
    write_u32(bytes, MACHO_DYLD_INFO_COMMAND_SIZE as u32);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(
        bytes,
        u32::try_from(bind_offset).expect("Mach-O bind offset overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(bind_size).expect("Mach-O bind size overflow"),
    );
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}
