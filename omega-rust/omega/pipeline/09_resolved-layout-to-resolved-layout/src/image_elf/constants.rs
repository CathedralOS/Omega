//! The static emitter's fixed ELF64 geometry: 64-byte header, two 56-byte program
//! headers, image base 0x400000, 4 KiB pages.

pub(crate) const ELF_HEADER_SIZE: usize = 64;
pub(crate) const PROGRAM_HEADER_SIZE: usize = 56;
pub(crate) const PROGRAM_HEADER_COUNT: usize = 2;
pub(crate) const IMAGE_BASE: u64 = 0x400000;
pub(crate) const PAGE_SIZE: usize = 0x1000;
