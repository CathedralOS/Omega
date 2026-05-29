use crate::{
    FinalImage, FinalImageLayout, final_image_symbol_address, final_image_symbol_name, patch_bytes,
};
use omega_core::diagnostics::Diagnostic;
use omega_object_file::RelocationKind;

pub fn apply_x86_64_relocations(
    image: &mut FinalImage,
    layout: &FinalImageLayout,
    output_name: &str,
) -> Result<(), Diagnostic> {
    for (_, relocation) in image.relocations.iter() {
        let Some(symbol_address) =
            final_image_symbol_address(image, relocation.symbol_handle, layout)
        else {
            return Err(Diagnostic::error(format!(
                "{output_name} relocation references unknown symbol `{}`",
                final_image_symbol_name(image, relocation.symbol_handle)
            )));
        };

        match relocation.kind {
            RelocationKind::X86_64Absolute64 => {
                patch_bytes::write_u64(
                    &mut image.text,
                    relocation.text_offset,
                    symbol_address,
                    "x86_64",
                )?;
            }
            RelocationKind::X86_64Relative32 => {
                let relocation_address = layout.text_address + relocation.text_offset as u64 + 4;
                let delta = symbol_address as i64 - relocation_address as i64;
                let value = i32::try_from(delta).map_err(|_| {
                    Diagnostic::error(format!(
                        "{output_name} x86_64 relative relocation is out of range: {delta} byte(s)"
                    ))
                })?;
                patch_bytes::write_i32(&mut image.text, relocation.text_offset, value, "x86_64")?;
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
