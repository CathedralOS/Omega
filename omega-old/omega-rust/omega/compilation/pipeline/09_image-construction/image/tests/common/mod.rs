//! Shared layout fixture for the image integration targets: one canonical
//! text/data/bss address triple the envelope tests patch and replay against.

use image::FinalImageLayout;

pub const TEXT_ADDRESS: u64 = 0x40_0000;
pub const DATA_ADDRESS: u64 = 0x41_0000;
pub const BSS_ADDRESS: u64 = 0x42_0000;

pub fn layout() -> FinalImageLayout {
    FinalImageLayout {
        text_address: TEXT_ADDRESS,
        data_address: DATA_ADDRESS,
        bss_address: BSS_ADDRESS,
    }
}
