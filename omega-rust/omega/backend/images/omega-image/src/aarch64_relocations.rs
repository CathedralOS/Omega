//! In-place AArch64 ADRP, ADD-page-offset and B/BL field patching against the
//! layout an emitter chose.

use crate::{
    FinalImage, FinalImageLayout, FinalImageSection, final_image_imports_symbol,
    final_image_symbol_address, final_image_symbol_name, patch_bytes,
};
use omega_object_file::RelocationKind;
use psi_diagnostics::Diagnostic;

pub fn apply_aarch64_relocations(
    image: &mut FinalImage,
    layout: &FinalImageLayout,
    output_name: &str,
) -> Result<(), Diagnostic> {
    for (_, relocation) in image.relocation_table.relocations.iter() {
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
        let relocation_target = symbol_address
            .checked_add_signed(relocation.addend)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "{output_name} AArch64 relocation target overflows after addend {}",
                    relocation.addend
                ))
            })?;

        match relocation.kind {
            RelocationKind::Absolute64 => {
                let section = relocation.section;
                let section_bytes = image.memory.initialized_section_mut(section).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "{output_name} AArch64 absolute relocation targets non-materialized section {section:?}"
                    ))
                })?;
                patch_bytes::write_u64(
                    section_bytes,
                    relocation.offset,
                    relocation_target,
                    "AArch64",
                )?;
            }
            RelocationKind::Aarch64Page21 => {
                require_text_relocation(output_name, relocation.section)?;
                patch_aarch64_adrp(
                    &mut image.memory.text,
                    relocation.offset,
                    layout.text_address + relocation.offset as u64,
                    relocation_target,
                )?;
            }
            RelocationKind::Aarch64PageOffset12 => {
                require_text_relocation(output_name, relocation.section)?;
                patch_aarch64_add_page_offset(
                    &mut image.memory.text,
                    relocation.offset,
                    relocation_target,
                )?;
            }
            RelocationKind::Aarch64Branch26 => {
                require_text_relocation(output_name, relocation.section)?;
                patch_aarch64_branch26(
                    &mut image.memory.text,
                    relocation.offset,
                    layout.text_address + relocation.offset as u64,
                    relocation_target,
                )?;
            }
            RelocationKind::X86_64Relative32 => {
                return Err(Diagnostic::error(format!(
                    "{output_name} AArch64 image received x86_64 relocation"
                )));
            }
        }
    }

    Ok(())
}

fn require_text_relocation(
    output_name: &str,
    section: FinalImageSection,
) -> Result<(), Diagnostic> {
    if section == FinalImageSection::Text {
        Ok(())
    } else {
        Err(Diagnostic::error(format!(
            "{output_name} AArch64 instruction relocation targets non-text section {section:?}"
        )))
    }
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
    let mut instruction = patch_bytes::read_u32(text, offset, "AArch64")?;
    instruction &= !((0b11 << 29) | (0x7ffff << 5));
    instruction |= (immediate_low << 29) | (immediate_high << 5);
    patch_bytes::write_u32(text, offset, instruction, "AArch64")
}

fn patch_aarch64_add_page_offset(
    text: &mut [u8],
    offset: usize,
    symbol_address: u64,
) -> Result<(), Diagnostic> {
    let page_offset = (symbol_address & 0xfff) as u32;
    let mut instruction = patch_bytes::read_u32(text, offset, "AArch64")?;
    instruction &= !(0xfff << 10);
    instruction |= page_offset << 10;
    patch_bytes::write_u32(text, offset, instruction, "AArch64")
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

    let mut instruction = patch_bytes::read_u32(text, offset, "AArch64")?;
    instruction &= !0x03ff_ffff;
    instruction |= (immediate as u32) & 0x03ff_ffff;
    patch_bytes::write_u32(text, offset, instruction, "AArch64")
}

#[cfg(test)]
mod tests {
    use super::apply_aarch64_relocations;
    use crate::{
        FinalImage, FinalImageLayout, FinalImageMemory, FinalImageRelocation, FinalImageSection,
        FinalImageSymbol,
    };
    use omega_object_file::{RelocationKind, SymbolKind};
    use omega_target::NativeTarget;
    use psi_arena::Handle;

    #[test]
    fn absolute_relocation_applies_signed_addend() {
        let mut image = FinalImage::with_capacity(
            NativeTarget::linux_arm64(),
            FinalImageMemory {
                text: vec![0; 8],
                data: vec![0; 8],
                ..Default::default()
            },
            Handle::invalid(),
            1,
            0,
            1,
        );
        let target = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "entry".into(),
            section: FinalImageSection::Text,
            offset: 8,
            size: 1,
            kind: SymbolKind::Function,
        });
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Data,
                offset: 0,
                byte_width: 8,
                symbol_handle: target,
                addend: -4,
                kind: RelocationKind::Absolute64,
            });

        apply_aarch64_relocations(
            &mut image,
            &FinalImageLayout {
                text_address: 0x1000,
                data_address: 0x2000,
                bss_address: 0x3000,
            },
            "test image",
        )
        .expect("data relocation should apply");

        assert_eq!(
            u64::from_le_bytes(image.memory.data.try_into().unwrap()),
            0x1004
        );
    }
}
