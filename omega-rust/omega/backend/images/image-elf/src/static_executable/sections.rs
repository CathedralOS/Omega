//! The static lane's whole layout decision: text at the first page boundary past
//! the headers, data at the next, bss aligned after that.

use crate::constants::{
    ELF_HEADER_SIZE, IMAGE_BASE, PAGE_SIZE, PROGRAM_HEADER_COUNT, PROGRAM_HEADER_SIZE,
};
use crate::static_executable::layout::{align_to, align_to_u64};
use image::{FinalImage, FinalImageLayout};

pub(crate) struct ElfSections {
    pub(crate) text_offset: usize,
    pub(crate) data_offset: usize,
    pub(crate) text_address: u64,
    pub(crate) data_address: u64,
    pub(crate) bss_address: u64,
    pub(crate) data_memory_size: u64,
}

impl ElfSections {
    pub(crate) fn final_image_layout(&self) -> FinalImageLayout {
        FinalImageLayout {
            text_address: self.text_address,
            data_address: self.data_address,
            bss_address: self.bss_address,
        }
    }
}

pub(crate) fn plan_elf_sections(image: &FinalImage) -> ElfSections {
    let text_offset = align_to(
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE * PROGRAM_HEADER_COUNT,
        PAGE_SIZE,
    );
    let data_offset = align_to(text_offset + image.memory.text.len(), PAGE_SIZE);
    let text_address = IMAGE_BASE + text_offset as u64;
    let data_address = IMAGE_BASE + data_offset as u64;
    let bss_address = align_to_u64(
        data_address + image.memory.data.len() as u64,
        image.memory.bss_alignment as u64,
    );
    let data_memory_size = (bss_address - data_address)
        .checked_add(image.memory.bss_size as u64)
        .expect("ELF data memory size overflow");

    ElfSections {
        text_offset,
        data_offset,
        text_address,
        data_address,
        bss_address,
        data_memory_size,
    }
}

#[cfg(test)]
mod tests {}
