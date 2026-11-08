/*
We compute the single-pass PE32+ section layout (`PeSections`) and deterministic
4 KiB-paged base relocation blocks (`BaseRelocations` in `.reloc`) from the
final `.text`, `.rdata`, `.data`, and `.bss` extents.
*/

use crate::constants::{
    COFF_HEADER_SIZE, DOS_HEADER_SIZE, FILE_ALIGNMENT, IMAGE_BASE, OPTIONAL_HEADER_SIZE,
    SECTION_ALIGNMENT, SECTION_HEADER_SIZE, TEXT_RVA,
};
use crate::layout::{align_to, align_to_u32};
use image::{FinalImage, FinalImageLayout};
use relocations::{BaseRelocations, build_base_relocations};

pub(crate) struct PeSections {
    pub(crate) text_virtual_size: usize,
    pub(crate) rdata_virtual_size: usize,
    pub(crate) has_rdata: bool,
    pub(crate) has_data: bool,
    pub(crate) has_reloc: bool,
    pub(crate) has_bss: bool,
    pub(crate) section_count: usize,
    pub(crate) headers_size: usize,
    pub(crate) text_raw_size: usize,
    pub(crate) rdata_raw_size: usize,
    pub(crate) data_raw_size: usize,
    pub(crate) reloc_raw_size: usize,
    pub(crate) text_raw: usize,
    pub(crate) rdata_raw: usize,
    pub(crate) data_raw: usize,
    pub(crate) reloc_raw: usize,
    pub(crate) rdata_rva: u32,
    pub(crate) data_rva: u32,
    pub(crate) reloc_rva: u32,
    pub(crate) bss_rva: u32,
    pub(crate) size_of_image: u32,
    pub(crate) reloc: BaseRelocations,
}

impl PeSections {
    pub(crate) fn final_image_layout(&self) -> FinalImageLayout {
        FinalImageLayout {
            text_address: IMAGE_BASE + u64::from(TEXT_RVA),
            data_address: IMAGE_BASE + u64::from(self.data_rva),
            bss_address: IMAGE_BASE + u64::from(self.bss_rva),
        }
    }
}

pub(crate) fn rdata_rva_after_text(image: &FinalImage) -> u32 {
    align_to_u32(TEXT_RVA + image.memory.text.len() as u32, SECTION_ALIGNMENT)
}

pub(crate) fn plan_pe_sections(image: &FinalImage, rdata_virtual_size: usize) -> PeSections {
    let text_virtual_size = image.memory.text.len();
    let rdata_rva = rdata_rva_after_text(image);
    let data_rva = align_to_u32(rdata_rva + rdata_virtual_size as u32, SECTION_ALIGNMENT);
    let reloc = build_base_relocations(image, TEXT_RVA, data_rva);
    let has_reloc = !reloc.bytes.is_empty();
    let reloc_rva = align_to_u32(data_rva + image.memory.data.len() as u32, SECTION_ALIGNMENT);
    let bss_rva = align_to_u32(reloc_rva + reloc.bytes.len() as u32, SECTION_ALIGNMENT);
    let has_data = !image.memory.data.is_empty();
    let has_bss = image.memory.bss_size > 0;
    let has_rdata = rdata_virtual_size > 0;
    let section_count = 1
        + usize::from(has_rdata)
        + usize::from(has_data)
        + usize::from(has_reloc)
        + usize::from(has_bss);
    let headers_size = align_to(
        DOS_HEADER_SIZE
            + 4
            + COFF_HEADER_SIZE
            + OPTIONAL_HEADER_SIZE
            + section_count * SECTION_HEADER_SIZE,
        FILE_ALIGNMENT,
    );
    let text_raw_size = align_to(image.memory.text.len(), FILE_ALIGNMENT);
    let rdata_raw_size = align_to(rdata_virtual_size, FILE_ALIGNMENT);
    let data_raw_size = align_to(image.memory.data.len(), FILE_ALIGNMENT);
    let reloc_raw_size = align_to(reloc.bytes.len(), FILE_ALIGNMENT);
    let text_raw = headers_size;
    let rdata_raw = text_raw + text_raw_size;
    let data_raw = rdata_raw + rdata_raw_size;
    let reloc_raw = data_raw + data_raw_size;
    let size_of_image = align_to_u32(bss_rva + image.memory.bss_size as u32, SECTION_ALIGNMENT);

    PeSections {
        text_virtual_size,
        rdata_virtual_size,
        has_rdata,
        has_data,
        has_reloc,
        has_bss,
        section_count,
        headers_size,
        text_raw_size,
        rdata_raw_size,
        data_raw_size,
        reloc_raw_size,
        text_raw,
        rdata_raw,
        data_raw,
        reloc_raw,
        rdata_rva,
        data_rva,
        reloc_rva,
        bss_rva,
        size_of_image,
        reloc,
    }
}

#[cfg(test)]
mod tests {
    use super::{plan_pe_sections, rdata_rva_after_text};
    use crate::constants::{FILE_ALIGNMENT, SECTION_ALIGNMENT, TEXT_RVA};
    use arena::Handle;
    use image::FinalImage;

    #[test]
    fn plans_pe_sections_with_data_and_bss() {
        let image = FinalImage::with_capacity(
            FinalImage::default().target,
            image::FinalImageMemory {
                text: vec![0; 3],
                data: vec![0; 5],
                bss_size: 7,
                ..Default::default()
            },
            Handle::invalid(),
            0,
            0,
            0,
        );

        let sections = plan_pe_sections(&image, 9);

        assert_eq!(sections.section_count, 4);
        assert_eq!(sections.text_raw_size, FILE_ALIGNMENT);
        assert_eq!(sections.rdata_raw_size, FILE_ALIGNMENT);
        assert_eq!(sections.data_raw_size, FILE_ALIGNMENT);
        assert_eq!(sections.rdata_rva, TEXT_RVA + SECTION_ALIGNMENT as u32);
        assert_eq!(sections.rdata_rva, rdata_rva_after_text(&image));
        assert_eq!(sections.data_rva, TEXT_RVA + SECTION_ALIGNMENT as u32 * 2);
        assert_eq!(sections.bss_rva, TEXT_RVA + SECTION_ALIGNMENT as u32 * 3);
        assert_eq!(
            sections.size_of_image,
            TEXT_RVA + SECTION_ALIGNMENT as u32 * 4
        );
        assert!(!sections.has_reloc);
        assert!(sections.reloc.bytes.is_empty());
    }
}

pub(crate) mod relocations {
    use image::{FinalImage, FinalImageSection};
    use object_file::RelocationKind;

    const REL_BASED_DIR64: u16 = 10;
    const REL_BASED_ABSOLUTE: u16 = 0;

    pub(crate) struct BaseRelocations {
        pub(crate) bytes: Vec<u8>,
    }

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
        use arena::Handle;
        use image::{FinalImage, FinalImageRelocation, FinalImageSection};
        use object_file::RelocationKind;

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
}
