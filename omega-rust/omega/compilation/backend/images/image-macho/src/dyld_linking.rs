//! What dyld sees: the dylib roster and bind stream, rebase opcodes, the
//! load commands, and the replayed mapping and fixups the loader performs.

pub(crate) mod imports;
pub(crate) mod load_commands;
pub(crate) mod loader_fixups;
pub(crate) mod loader_mapping;
pub(crate) mod rebases;

pub(crate) fn write_uleb128(bytes: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            break;
        }
    }
}
