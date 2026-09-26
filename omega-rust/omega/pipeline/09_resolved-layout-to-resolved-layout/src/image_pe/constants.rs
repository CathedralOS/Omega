//! PE32+ fixed geometry: the two alignments, the image base, and the header sizes
//! that decide where .text can start.

pub(crate) const DOS_HEADER_SIZE: usize = 0x80;
pub(crate) const COFF_HEADER_SIZE: usize = 20;
pub(crate) const OPTIONAL_HEADER_SIZE: usize = 240;
pub(crate) const SECTION_HEADER_SIZE: usize = 40;
pub(crate) const IMAGE_BASE: u64 = 0x1_4000_0000;
pub(crate) const SECTION_ALIGNMENT: usize = 0x1000;
pub(crate) const FILE_ALIGNMENT: usize = 0x200;
pub(crate) const TEXT_RVA: u32 = 0x1000;
