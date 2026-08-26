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
    rebase_offset: usize,
    rebase_size: usize,
    bind_offset: usize,
    bind_size: usize,
) {
    write_u32(bytes, 0x8000_0022);
    write_u32(bytes, MACHO_DYLD_INFO_COMMAND_SIZE as u32);
    write_u32(
        bytes,
        u32::try_from(rebase_offset).expect("Mach-O rebase offset overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(rebase_size).expect("Mach-O rebase size overflow"),
    );
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

#[cfg(test)]
mod tests {
    use super::write_macho_dyld_info_command;

    #[test]
    fn dyld_info_retains_rebase_and_bind_ranges_independently() {
        let mut bytes = Vec::new();
        write_macho_dyld_info_command(&mut bytes, 0x8000, 5, 0x8005, 0);

        let word = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        assert_eq!(word(0), 0x8000_0022);
        assert_eq!(word(8), 0x8000);
        assert_eq!(word(12), 5);
        assert_eq!(word(16), 0x8005);
        assert_eq!(word(20), 0);
    }
}
