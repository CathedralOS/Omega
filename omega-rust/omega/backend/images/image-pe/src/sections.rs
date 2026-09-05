//! The whole PE layout decision in one pass: virtual and raw sizes, RVAs, and file
//! offsets for every section that exists.

use crate::constants::{
    COFF_HEADER_SIZE, DOS_HEADER_SIZE, FILE_ALIGNMENT, IMAGE_BASE, OPTIONAL_HEADER_SIZE,
    SECTION_ALIGNMENT, SECTION_HEADER_SIZE, TEXT_RVA,
};
use crate::layout::{align_to, align_to_u32};
use image::{FinalImage, FinalImageLayout};

pub(crate) struct PeSections {
    pub(crate) text_virtual_size: usize,
    pub(crate) rdata_virtual_size: usize,
    pub(crate) has_data: bool,
    pub(crate) has_reloc: bool,
    pub(crate) has_bss: bool,
    pub(crate) section_count: usize,
    pub(crate) headers_size: usize,
    pub(crate) text_raw_size: usize,
    pub(crate) rdata_raw_size: usize,
    pub(crate) data_raw_size: usize,
    pub(crate) reloc_raw_size: usize,
    pub(crate) reloc_virtual_size: usize,
    pub(crate) text_raw: usize,
    pub(crate) rdata_raw: usize,
    pub(crate) data_raw: usize,
    pub(crate) reloc_raw: usize,
    pub(crate) rdata_rva: u32,
    pub(crate) data_rva: u32,
    pub(crate) reloc_rva: u32,
    pub(crate) bss_rva: u32,
    pub(crate) size_of_image: u32,
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

pub(crate) fn plan_pe_sections(image: &FinalImage, rdata_virtual_size: usize) -> PeSections {
    let text_virtual_size = image.memory.text.len();
    let rdata_rva = align_to_u32(TEXT_RVA + text_virtual_size as u32, SECTION_ALIGNMENT);
    let data_rva = align_to_u32(rdata_rva + rdata_virtual_size as u32, SECTION_ALIGNMENT);
    // Relocation sites may live in text or data. Both RVAs are known before
    // the relocation section itself is placed.
    let reloc_virtual_size = crate::relocations::build_base_relocations(image, TEXT_RVA, data_rva)
        .bytes
        .len();
    let has_reloc = reloc_virtual_size > 0;
    let reloc_rva = align_to_u32(data_rva + image.memory.data.len() as u32, SECTION_ALIGNMENT);
    let bss_rva = align_to_u32(reloc_rva + reloc_virtual_size as u32, SECTION_ALIGNMENT);
    let has_data = !image.memory.data.is_empty();
    let has_bss = image.memory.bss_size > 0;
    let section_count = 2 + usize::from(has_data) + usize::from(has_reloc) + usize::from(has_bss);
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
    let reloc_raw_size = align_to(reloc_virtual_size, FILE_ALIGNMENT);
    let text_raw = headers_size;
    let rdata_raw = text_raw + text_raw_size;
    let data_raw = rdata_raw + rdata_raw_size;
    let reloc_raw = data_raw + data_raw_size;
    let size_of_image = align_to_u32(bss_rva + image.memory.bss_size as u32, SECTION_ALIGNMENT);

    PeSections {
        text_virtual_size,
        rdata_virtual_size,
        has_data,
        has_reloc,
        has_bss,
        section_count,
        headers_size,
        text_raw_size,
        rdata_raw_size,
        data_raw_size,
        reloc_raw_size,
        reloc_virtual_size,
        text_raw,
        rdata_raw,
        data_raw,
        reloc_raw,
        rdata_rva,
        data_rva,
        reloc_rva,
        bss_rva,
        size_of_image,
    }
}

#[cfg(test)]
mod tests {
    use super::plan_pe_sections;
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
        assert_eq!(sections.data_rva, TEXT_RVA + SECTION_ALIGNMENT as u32 * 2);
        assert_eq!(sections.bss_rva, TEXT_RVA + SECTION_ALIGNMENT as u32 * 3);
        assert_eq!(
            sections.size_of_image,
            TEXT_RVA + SECTION_ALIGNMENT as u32 * 4
        );
    }
}
