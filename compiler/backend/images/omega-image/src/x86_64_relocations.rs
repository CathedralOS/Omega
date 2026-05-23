use crate::{FinalImage, FinalImageLayout, final_image_symbol_address, final_image_symbol_name};
use omega_core::diagnostics::Diagnostic;
use omega_object::RelocationKind;

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
                write_u64(&mut image.text, relocation.text_offset, symbol_address)?;
            }
            RelocationKind::X86_64Relative32 => {
                let relocation_address = layout.text_address + relocation.text_offset as u64 + 4;
                let delta = symbol_address as i64 - relocation_address as i64;
                let value = i32::try_from(delta).map_err(|_| {
                    Diagnostic::error(format!(
                        "{output_name} x86_64 relative relocation is out of range: {delta} byte(s)"
                    ))
                })?;
                write_i32(&mut image.text, relocation.text_offset, value)?;
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

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), Diagnostic> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| Diagnostic::error("x86_64 relocation offset overflow"))?;
    let Some(slice) = bytes.get_mut(offset..end) else {
        return Err(Diagnostic::error(format!(
            "x86_64 relocation offset {offset} is outside text section"
        )));
    };

    slice.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_i32(bytes: &mut [u8], offset: usize, value: i32) -> Result<(), Diagnostic> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| Diagnostic::error("x86_64 relocation offset overflow"))?;
    let Some(slice) = bytes.get_mut(offset..end) else {
        return Err(Diagnostic::error(format!(
            "x86_64 relocation offset {offset} is outside text section"
        )));
    };

    slice.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
