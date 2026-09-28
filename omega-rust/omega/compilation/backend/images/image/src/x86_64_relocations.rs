//! In-place x86-64 rel32 and absolute-64 field patching against the layout an
//! emitter chose.

use crate::{
    FinalImage, FinalImageLayout, final_image_symbol_address, final_image_symbol_name, patch_bytes,
};
use diagnostics::Diagnostic;
use object_file::RelocationKind;

pub fn apply_x86_64_relocations(
    image: &mut FinalImage,
    layout: &FinalImageLayout,
    output_name: &str,
) -> Result<(), Diagnostic> {
    for (_, relocation) in image.relocation_table.relocations.iter() {
        let Some(symbol_address) =
            final_image_symbol_address(image, relocation.symbol_handle, layout)
        else {
            return Err(Diagnostic::error(format!(
                "{output_name} relocation references unknown symbol `{}`",
                final_image_symbol_name(image, relocation.symbol_handle)
            )));
        };
        let relocation_target = symbol_address
            .checked_add_signed(relocation.addend)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "{output_name} x86_64 relocation target overflows after addend {}",
                    relocation.addend
                ))
            })?;

        match relocation.kind {
            RelocationKind::Absolute64 => {
                let section = relocation.section;
                let section_bytes = image.memory.initialized_section_mut(section).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "{output_name} x86_64 absolute relocation targets non-materialized section {section:?}"
                    ))
                })?;
                patch_bytes::write_u64(
                    section_bytes,
                    relocation.offset,
                    relocation_target,
                    "x86_64",
                )?;
            }
            RelocationKind::X86_64Relative32 => {
                let section = relocation.section;
                let section_address = layout.section_address(section).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "{output_name} x86_64 relative relocation has no section"
                    ))
                })?;
                let relocation_address = section_address + relocation.offset as u64 + 4;
                let delta = i128::from(relocation_target) - i128::from(relocation_address);
                let value = i32::try_from(delta).map_err(|_| {
                    Diagnostic::error(format!(
                        "{output_name} x86_64 relative relocation is out of range: {delta} byte(s)"
                    ))
                })?;
                let section_bytes = image.memory.initialized_section_mut(section).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "{output_name} x86_64 relative relocation targets non-materialized section {section:?}"
                    ))
                })?;
                patch_bytes::write_i32(section_bytes, relocation.offset, value, "x86_64")?;
            }
            RelocationKind::Aarch64Page21
            | RelocationKind::Aarch64PageOffset12
            | RelocationKind::Aarch64Branch26 => {
                return Err(Diagnostic::error(format!(
                    "{output_name} x86_64 image received AArch64 relocation"
                )));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::apply_x86_64_relocations;
    use crate::{
        FinalImage, FinalImageLayout, FinalImageMemory, FinalImageRelocation, FinalImageSection,
        FinalImageSymbol,
    };
    use arena::Handle;
    use object_file::{RelocationKind, SymbolKind};
    use target::NativeTarget;

    #[test]
    fn absolute_relocation_can_patch_initialized_data() {
        let mut image = FinalImage::with_capacity(
            NativeTarget::linux_x64(),
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
            offset: 4,
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
                addend: 7,
                kind: RelocationKind::Absolute64,
            });

        apply_x86_64_relocations(
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
            0x100b
        );
    }
}
