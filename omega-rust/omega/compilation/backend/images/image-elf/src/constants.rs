//! The static emitter's fixed ELF64 geometry: 64-byte header, two 56-byte program
//! headers, image base 0x400000, 4 KiB pages.

pub(crate) const ELF_HEADER_SIZE: usize = 64;
pub(crate) const PROGRAM_HEADER_SIZE: usize = 56;
pub(crate) const PROGRAM_HEADER_COUNT: usize = 2;
pub(crate) const IMAGE_BASE: u64 = 0x400000;
pub(crate) const PAGE_SIZE: usize = 0x1000;

// Dynamic-executable ELF64 vocabulary shared by every module under
// `dynamic_executable/` — section rosters, load placement, relocation
// tables and the file envelope all describe the same artifact shape.
pub(crate) const SECTION_COUNT: usize = 14;
pub(crate) const ELF64_SECTION_HEADER_SIZE: usize = 64;
pub(crate) const ELF64_DYNAMIC_ROW_SIZE: usize = 16;
pub(crate) const ELF64_DYNAMIC_VALUE_OFFSET: usize = 8;
pub(crate) const ELF64_DYNAMIC_VALUE_SIZE: u8 = 8;
pub(crate) const ELF64_RELA_SIZE: usize = 24;
pub(crate) const SHF_WRITE: u64 = 0x1;
pub(crate) const SHF_ALLOC: u64 = 0x2;
pub(crate) const SHF_EXECINSTR: u64 = 0x4;
