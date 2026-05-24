use crate::{
    FinalImage, FinalImageLayout, final_image_imports_symbol, final_image_symbol_address,
    final_image_symbol_name,
};
use omega_core::diagnostics::Diagnostic;
use omega_object_file::RelocationKind;

pub fn apply_aarch64_relocations(
    image: &mut FinalImage,
    layout: &FinalImageLayout,
    output_name: &str,
) -> Result<(), Diagnostic> {
    for (_, relocation) in image.relocations.iter() {
        let Some(symbol_address) =
            final_image_symbol_address(image, relocation.symbol_handle, layout)
        else {
            let symbol_name = final_image_symbol_name(image, relocation.symbol_handle);
            if final_image_imports_symbol(image, relocation.symbol_handle) {
                return Err(Diagnostic::error(format!(
                    "{output_name} cannot import `{}` yet; use syscalls or add dynamic binding",
                    symbol_name
                )));
            }

            return Err(Diagnostic::error(format!(
                "{output_name} relocation references unknown symbol `{}`",
                symbol_name
            )));
        };

        match relocation.kind {
            RelocationKind::Aarch64Page21 => {
                patch_aarch64_adrp(
                    &mut image.text,
                    relocation.text_offset,
                    layout.text_address + relocation.text_offset as u64,
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
                    layout.text_address + relocation.text_offset as u64,
                    symbol_address,
                )?;
            }
            RelocationKind::X86_64Absolute64 | RelocationKind::X86_64Relative32 => {
                return Err(Diagnostic::error(format!(
                    "{output_name} AArch64 image received x86_64 relocation"
                )));
            }
        }
    }

    Ok(())
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

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Diagnostic> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| Diagnostic::error("AArch64 relocation offset overflow"))?;
    let Some(slice) = bytes.get(offset..end) else {
        return Err(Diagnostic::error(format!(
            "AArch64 relocation offset {offset} is outside text section"
        )));
    };

    Ok(u32::from_le_bytes(
        slice.try_into().expect("u32 relocation slice has length 4"),
    ))
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), Diagnostic> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| Diagnostic::error("AArch64 relocation offset overflow"))?;
    let Some(slice) = bytes.get_mut(offset..end) else {
        return Err(Diagnostic::error(format!(
            "AArch64 relocation offset {offset} is outside text section"
        )));
    };

    slice.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
