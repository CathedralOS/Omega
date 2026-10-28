//! The byte layer both ELF lanes share: the append encoders, and the one
//! reader that turns a stored name-table offset back into the exact name the
//! producer meant.

use diagnostics::Diagnostic;
pub(crate) use image::codec_bytes::{write_u16, write_u32, write_u64};

/// The exact NUL-terminated name `offset` selects from its name table,
/// refusing an offset outside the table or a name that does not terminate
/// inside it. Every `sh_name`/`st_name`/`DT_NEEDED` reader goes through here,
/// so the bounds and termination rule is stated once.
pub(crate) fn name_at<'a>(
    bytes: &'a [u8],
    offset: u32,
    context: &'static str,
) -> Result<&'a [u8], Diagnostic> {
    let tail = bytes
        .get(offset as usize..)
        .ok_or_else(|| Diagnostic::error(format!("{context} is outside its name table")))?;
    let terminator = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| Diagnostic::error(format!("{context} is not NUL-terminated")))?;
    Ok(&tail[..terminator])
}

/// The exact name `offset` selects, required to be `expected`.
pub(crate) fn require_name(
    bytes: &[u8],
    offset: u32,
    expected: &[u8],
    context: &'static str,
) -> Result<(), Diagnostic> {
    if name_at(bytes, offset, context)? == expected {
        Ok(())
    } else {
        Err(Diagnostic::error(format!(
            "{context} does not select its exact name"
        )))
    }
}
