//! Mach-O fixed geometry: the shared four-gigabyte load base and every load
//! command's byte size. Per-ISA page sizes and CPU fields live in `crate::isa`.

pub(crate) const MACHO_EXECUTABLE_BASE: u64 = 0x1_0000_0000;
pub(super) const MACHO_HEADER_SIZE: usize = 32;
pub(crate) const MACHO_SEGMENT_COMMAND_SIZE: usize = 72;
pub(crate) const MACHO_SECTION_SIZE: usize = 80;
pub(crate) const MACHO_LOAD_DYLINKER_COMMAND_SIZE: usize = 32;
pub(crate) const MACHO_UUID_COMMAND_SIZE: usize = 24;
pub(crate) const MACHO_MAIN_COMMAND_SIZE: usize = 24;
pub(crate) const MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE: usize = 32;
// LC_LOAD_DYLIB command sizes are per-dylib now (24-byte header + padded install
// name) — see `MachoDylib::command_size`. libSystem's is 56, its historical value.
pub(crate) const MACHO_DYLD_INFO_COMMAND_SIZE: usize = 48;
pub(crate) const MACHO_SYMTAB_COMMAND_SIZE: usize = 24;
pub(crate) const MACHO_DYSYMTAB_COMMAND_SIZE: usize = 80;
pub(crate) const MACHO_CODE_SIGNATURE_COMMAND_SIZE: usize = 16;
pub(crate) const MACHO_HEADER_FLAGS_NOUNDEFS: u32 = 0x1;
pub(crate) const MACHO_HEADER_FLAGS_DYLDLINK: u32 = 0x4;
pub(crate) const MACHO_HEADER_FLAGS_TWOLEVEL: u32 = 0x80;
pub(crate) const MACHO_HEADER_FLAGS_PIE: u32 = 0x20_0000;
