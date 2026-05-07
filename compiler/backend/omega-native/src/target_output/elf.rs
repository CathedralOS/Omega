use std::collections::BTreeMap;

use crate::emitter::EmittedNativeOutput;
use crate::final_image::{FinalImage, FinalImageSection, build_final_image};
use crate::plan::NativePlan;
use crate::relocations::RelocationKind;
use omega_core::diagnostics::Diagnostic;

const ELF_HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: usize = 56;
const PROGRAM_HEADER_COUNT: usize = 2;
const IMAGE_BASE: u64 = 0x400000;
const PAGE_SIZE: usize = 0x1000;

pub fn emit_elf_arm64_executable(
    native_plan: &NativePlan,
) -> Result<EmittedNativeOutput, Diagnostic> {
    let mut image = build_final_image(native_plan);
    let text_offset = align_to(
        ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE * PROGRAM_HEADER_COUNT,
        PAGE_SIZE,
    );
    let data_offset = align_to(text_offset + image.text.len(), PAGE_SIZE);
    let text_address = IMAGE_BASE + text_offset as u64;
    let data_address = IMAGE_BASE + data_offset as u64;
    let bss_address = align_to_u64(
        data_address + image.data.len() as u64,
        image.bss_alignment as u64,
    );
    let symbol_addresses =
        collect_symbol_addresses(&image, text_address, data_address, bss_address);

    apply_relocations(&mut image, text_address, &symbol_addresses)?;

    let data_memory_size = (bss_address - data_address)
        .checked_add(image.bss_size as u64)
        .expect("ELF data memory size overflow");
    let mut bytes = Vec::with_capacity(data_offset + image.data.len());
    write_elf_header(&mut bytes, text_address, text_offset, data_offset);
    write_text_program_header(&mut bytes, text_offset, image.text.len());
    write_data_program_header(&mut bytes, data_offset, image.data.len(), data_memory_size);
    bytes.resize(text_offset, 0);
    bytes.extend(&image.text);
    bytes.resize(data_offset, 0);
    bytes.extend(&image.data);

    Ok(EmittedNativeOutput {
        bytes,
        file_name: "omega-program".to_owned(),
        format: "elf64-aarch64-executable".to_owned(),
        text_bytes: image.text.len(),
        data_bytes: image.data.len(),
        bss_bytes: image.bss_size,
        symbols: image.symbols.len(),
        relocations: image.relocations.len(),
    })
}

fn collect_symbol_addresses(
    image: &FinalImage,
    text_address: u64,
    data_address: u64,
    bss_address: u64,
) -> BTreeMap<String, u64> {
    let mut symbol_addresses = BTreeMap::new();

    for (_, symbol) in image.symbols.iter() {
        let section_address = match symbol.section {
            FinalImageSection::Text => text_address,
            FinalImageSection::Data => data_address,
            FinalImageSection::Bss => bss_address,
            FinalImageSection::None => continue,
        };
        symbol_addresses.insert(symbol.name.clone(), section_address + symbol.offset as u64);
    }

    symbol_addresses
}

fn apply_relocations(
    image: &mut FinalImage,
    text_address: u64,
    symbol_addresses: &BTreeMap<String, u64>,
) -> Result<(), Diagnostic> {
    for (_, relocation) in image.relocations.iter() {
        let Some(symbol_address) = symbol_addresses.get(&relocation.symbol).copied() else {
            if image_imports_symbol(image, &relocation.symbol) {
                return Err(Diagnostic::error(format!(
                    "ELF direct image cannot import `{}` yet; use syscalls or add dynamic linking",
                    relocation.symbol
                )));
            }

            return Err(Diagnostic::error(format!(
                "ELF relocation references unknown symbol `{}`",
                relocation.symbol
            )));
        };

        match relocation.kind {
            RelocationKind::Aarch64Page21 => {
                patch_aarch64_adrp(
                    &mut image.text,
                    relocation.text_offset,
                    text_address + relocation.text_offset as u64,
                    symbol_address,
                )?;
            }
            RelocationKind::Aarch64PageOffset12 => {
                patch_aarch64_add_page_offset(
                    &mut image.text,
                    relocation.text_offset,
                    symbol_address,
                )?;
            }
            RelocationKind::Aarch64Branch26 => {
                patch_aarch64_branch26(
                    &mut image.text,
                    relocation.text_offset,
                    text_address + relocation.text_offset as u64,
                    symbol_address,
                )?;
            }
            RelocationKind::X86_64Absolute64 | RelocationKind::X86_64Relative32 => {
                return Err(Diagnostic::error(
                    "ELF AArch64 image writer received x86_64 relocation",
                ));
            }
        }
    }

    Ok(())
}

fn image_imports_symbol(image: &FinalImage, symbol_name: &str) -> bool {
    image
        .imports
        .iter()
        .any(|(_, import)| import.symbol == symbol_name)
}

fn patch_aarch64_adrp(
    text: &mut [u8],
    offset: usize,
    instruction_address: u64,
    symbol_address: u64,
) -> Result<(), Diagnostic> {
    let instruction_page = instruction_address & !0xfff;
    let symbol_page = symbol_address & !0xfff;
    let page_delta = (symbol_page as i64 - instruction_page as i64) / 4096;

    if !(-(1 << 20)..(1 << 20)).contains(&page_delta) {
        return Err(Diagnostic::error(format!(
            "AArch64 ADRP relocation is out of range: {page_delta} page(s)"
        )));
    }

    let immediate = (page_delta as u32) & 0x1f_ffff;
    let immediate_low = immediate & 0b11;
    let immediate_high = (immediate >> 2) & 0x7ffff;
    let mut instruction = read_u32(text, offset)?;
    instruction &= !((0b11 << 29) | (0x7ffff << 5));
    instruction |= (immediate_low << 29) | (immediate_high << 5);
    write_u32(text, offset, instruction)
}

fn patch_aarch64_add_page_offset(
    text: &mut [u8],
    offset: usize,
    symbol_address: u64,
) -> Result<(), Diagnostic> {
    let page_offset = (symbol_address & 0xfff) as u32;
    let mut instruction = read_u32(text, offset)?;
    instruction &= !(0xfff << 10);
    instruction |= page_offset << 10;
    write_u32(text, offset, instruction)
}

fn patch_aarch64_branch26(
    text: &mut [u8],
    offset: usize,
    instruction_address: u64,
    symbol_address: u64,
) -> Result<(), Diagnostic> {
    let byte_delta = symbol_address as i64 - instruction_address as i64;
    if byte_delta % 4 != 0 {
        return Err(Diagnostic::error(
            "AArch64 branch relocation target is not instruction-aligned",
        ));
    }
    let immediate = byte_delta / 4;
    if !(-(1 << 25)..(1 << 25)).contains(&immediate) {
        return Err(Diagnostic::error(format!(
            "AArch64 branch relocation is out of range: {immediate} instruction(s)"
        )));
    }

    let mut instruction = read_u32(text, offset)?;
    instruction &= !0x03ff_ffff;
    instruction |= (immediate as u32) & 0x03ff_ffff;
    write_u32(text, offset, instruction)
}

fn write_elf_header(
    bytes: &mut Vec<u8>,
    entry_address: u64,
    text_offset: usize,
    data_offset: usize,
) {
    bytes.extend([0x7f, b'E', b'L', b'F']);
    bytes.push(2);
    bytes.push(1);
    bytes.push(1);
    bytes.push(0);
    bytes.extend([0; 8]);
    write_u16(bytes, 2);
    write_u16(bytes, 183);
    write_vec_u32(bytes, 1);
    write_u64(bytes, entry_address);
    write_u64(bytes, ELF_HEADER_SIZE as u64);
    write_u64(bytes, 0);
    write_vec_u32(bytes, 0);
    write_u16(bytes, ELF_HEADER_SIZE as u16);
    write_u16(bytes, PROGRAM_HEADER_SIZE as u16);
    write_u16(bytes, PROGRAM_HEADER_COUNT as u16);
    write_u16(bytes, 0);
    write_u16(bytes, 0);
    write_u16(bytes, 0);

    debug_assert_eq!(bytes.len(), ELF_HEADER_SIZE);
    debug_assert!(text_offset >= bytes.len());
    debug_assert!(data_offset > text_offset);
}

fn write_text_program_header(bytes: &mut Vec<u8>, text_offset: usize, text_size: usize) {
    write_program_header(
        bytes,
        5,
        0,
        IMAGE_BASE,
        u64::try_from(text_offset + text_size).expect("ELF text segment size overflow"),
        u64::try_from(text_offset + text_size).expect("ELF text memory size overflow"),
    );
}

fn write_data_program_header(
    bytes: &mut Vec<u8>,
    data_offset: usize,
    data_size: usize,
    data_memory_size: u64,
) {
    write_program_header(
        bytes,
        6,
        u64::try_from(data_offset).expect("ELF data offset overflow"),
        IMAGE_BASE + data_offset as u64,
        u64::try_from(data_size).expect("ELF data segment size overflow"),
        data_memory_size,
    );
}

fn write_program_header(
    bytes: &mut Vec<u8>,
    flags: u32,
    offset: u64,
    virtual_address: u64,
    file_size: u64,
    memory_size: u64,
) {
    write_vec_u32(bytes, 1);
    write_vec_u32(bytes, flags);
    write_u64(bytes, offset);
    write_u64(bytes, virtual_address);
    write_u64(bytes, virtual_address);
    write_u64(bytes, file_size);
    write_u64(bytes, memory_size);
    write_u64(bytes, PAGE_SIZE as u64);
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Diagnostic> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| Diagnostic::error("ELF relocation offset overflow"))?;
    let Some(slice) = bytes.get(offset..end) else {
        return Err(Diagnostic::error(format!(
            "ELF relocation offset {offset} is outside text section"
        )));
    };

    Ok(u32::from_le_bytes(
        slice.try_into().expect("u32 relocation slice has length 4"),
    ))
}

fn write_u32_at(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), Diagnostic> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| Diagnostic::error("ELF relocation offset overflow"))?;
    let Some(slice) = bytes.get_mut(offset..end) else {
        return Err(Diagnostic::error(format!(
            "ELF relocation offset {offset} is outside text section"
        )));
    };

    slice.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend(value.to_le_bytes());
}

fn write_vec_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}

fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

fn align_to_u64(value: u64, alignment: u64) -> u64 {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

fn write_u32(text: &mut [u8], offset: usize, instruction: u32) -> Result<(), Diagnostic> {
    write_u32_at(text, offset, instruction)
}
