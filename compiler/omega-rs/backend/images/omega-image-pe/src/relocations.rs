//! The PE base-relocation (`.reloc`) section. Every `X86_64Absolute64`
//! relocation bakes a full `ImageBase + rva` virtual address into `.text`; a
//! loader that places the image at any base OTHER than the preferred one
//! (Windows ASLR under DYNAMICBASE, and UEFI/OVMF which loads at an arbitrary
//! base) must fix those sites up. The `.reloc` section is the list of them.
//!
//! Without it the image is silently fixed-base -- it happens to work on
//! Windows only while the preferred base is free, and breaks under EFI. With
//! it (and DYNAMICBASE set), Windows relocates the image on every run using
//! these entries, so the ordinary run-tests become a live correctness oracle.

use crate::constants::TEXT_RVA;
use omega_image::FinalImage;
use omega_object_file::RelocationKind;

/// IMAGE_REL_BASED_DIR64: the loader adds the base delta to the 64-bit value
/// at this offset.
const REL_BASED_DIR64: u16 = 10;
/// IMAGE_REL_BASED_ABSOLUTE: a no-op used only to pad a block to a 4-byte
/// boundary.
const REL_BASED_ABSOLUTE: u16 = 0;

pub(crate) struct BaseRelocations {
    /// The fully-encoded `.reloc` section bytes (empty when there is nothing
    /// to relocate).
    pub(crate) bytes: Vec<u8>,
}

impl BaseRelocations {
    pub(crate) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Build the `.reloc` section from the image's `X86_64Absolute64` relocations
/// (all of which patch `.text`, so each site's RVA is `TEXT_RVA + offset`).
/// Entries are grouped into per-4KiB-page blocks in ascending RVA order, and
/// each block is padded to a 4-byte boundary with an ABSOLUTE no-op entry.
pub(crate) fn build_base_relocations(image: &FinalImage) -> BaseRelocations {
    let mut rvas: Vec<u32> = image
        .relocation_table
        .relocations
        .iter()
        .filter(|(_, relocation)| relocation.kind == RelocationKind::X86_64Absolute64)
        .map(|(_, relocation)| TEXT_RVA + relocation.text_offset as u32)
        .collect();
    if rvas.is_empty() {
        return BaseRelocations { bytes: Vec::new() };
    }
    rvas.sort_unstable();

    let mut bytes = Vec::new();
    let mut index = 0;
    while index < rvas.len() {
        let page = rvas[index] & !0xFFF;
        let block_start = index;
        while index < rvas.len() && (rvas[index] & !0xFFF) == page {
            index += 1;
        }
        let entries = &rvas[block_start..index];
        // Block = { PageRVA: u32, BlockSize: u32, [Entry: u16]* }, padded so
        // BlockSize is a multiple of 4.
        let mut entry_words: Vec<u16> = entries
            .iter()
            .map(|rva| (REL_BASED_DIR64 << 12) | (rva & 0xFFF) as u16)
            .collect();
        if entry_words.len() % 2 == 1 {
            entry_words.push(REL_BASED_ABSOLUTE << 12);
        }
        let block_size = 8 + entry_words.len() * 2;
        bytes.extend(page.to_le_bytes());
        bytes.extend((block_size as u32).to_le_bytes());
        for word in entry_words {
            bytes.extend(word.to_le_bytes());
        }
    }

    BaseRelocations { bytes }
}
