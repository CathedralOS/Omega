//! The PE base-relocation (`.reloc`) section. Every `Absolute64`
//! relocation bakes a full `ImageBase + rva` virtual address into an initialized section; a
//! loader that places the image at any base OTHER than the preferred one
//! (Windows ASLR under DYNAMICBASE, and UEFI/OVMF which loads at an arbitrary
//! base) must fix those sites up. The `.reloc` section is the list of them.
//!
//! Without it the image is silently fixed-base -- it happens to work on
//! Windows only while the preferred base is free, and breaks under EFI. With
//! it (and DYNAMICBASE set), Windows relocates the image on every run using
//! these entries, so the ordinary run-tests become a live correctness oracle.

use omega_image::{FinalImage, FinalImageSection};
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

/// Build the `.reloc` section from the image's `Absolute64` relocations
/// Entries are grouped into per-4KiB-page blocks in ascending RVA order, and
/// each block is padded to a 4-byte boundary with an ABSOLUTE no-op entry.
pub(crate) fn build_base_relocations(
    image: &FinalImage,
    text_rva: u32,
    data_rva: u32,
) -> BaseRelocations {
    let mut rvas: Vec<u32> = image
        .relocation_table
        .relocations
        .iter()
        .filter(|(_, relocation)| relocation.kind == RelocationKind::Absolute64)
        .filter_map(|(_, relocation)| {
            let section_rva = match relocation.section {
                FinalImageSection::Text => text_rva,
                FinalImageSection::Data => data_rva,
                FinalImageSection::Bss | FinalImageSection::None => return None,
            };
            Some(section_rva + relocation.offset as u32)
        })
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

#[cfg(test)]
mod tests {
    use super::build_base_relocations;
    use omega_image::{FinalImage, FinalImageRelocation, FinalImageSection};
    use omega_object_file::RelocationKind;
    use psi_arena::Handle;

    #[test]
    fn data_absolute_relocation_uses_data_rva() {
        let mut image = FinalImage::with_capacity(
            FinalImage::default().target,
            Default::default(),
            Handle::invalid(),
            0,
            0,
            1,
        );
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Data,
                offset: 0x28,
                byte_width: 8,
                symbol_handle: Handle::invalid(),
                addend: 0,
                kind: RelocationKind::Absolute64,
            });

        let relocations = build_base_relocations(&image, 0x1000, 0x3000);

        assert_eq!(&relocations.bytes[0..4], &0x3000_u32.to_le_bytes());
        assert_eq!(
            &relocations.bytes[8..10],
            &(0xa000_u16 | 0x28).to_le_bytes()
        );
    }
}
