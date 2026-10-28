//! Bounds-checked little-endian reads and writes into a section buffer, so a
//! relocation offset past the end is a diagnostic rather than a panic.

use std::ops::Range;

use diagnostics::Diagnostic;

pub(crate) fn read_u32(
    bytes: &[u8],
    offset: usize,
    relocation_name: &str,
) -> Result<u32, Diagnostic> {
    let slice = &bytes[relocation_range(bytes.len(), offset, 4, relocation_name)?];
    Ok(u32::from_le_bytes(
        slice.try_into().expect("u32 relocation slice has length 4"),
    ))
}

pub(crate) fn write_u32(
    bytes: &mut [u8],
    offset: usize,
    value: u32,
    relocation_name: &str,
) -> Result<(), Diagnostic> {
    write_bytes(bytes, offset, &value.to_le_bytes(), relocation_name)
}

pub(crate) fn write_u64(
    bytes: &mut [u8],
    offset: usize,
    value: u64,
    relocation_name: &str,
) -> Result<(), Diagnostic> {
    write_bytes(bytes, offset, &value.to_le_bytes(), relocation_name)
}

pub(crate) fn write_i32(
    bytes: &mut [u8],
    offset: usize,
    value: i32,
    relocation_name: &str,
) -> Result<(), Diagnostic> {
    write_bytes(bytes, offset, &value.to_le_bytes(), relocation_name)
}

fn write_bytes(
    bytes: &mut [u8],
    offset: usize,
    value: &[u8],
    relocation_name: &str,
) -> Result<(), Diagnostic> {
    let range = relocation_range(bytes.len(), offset, value.len(), relocation_name)?;
    bytes[range].copy_from_slice(value);
    Ok(())
}

/// The one bounds rule for a relocation field: the half-open range a
/// `width`-byte field at `offset` occupies, or the diagnostic that names the
/// section it missed.
fn relocation_range(
    bytes_len: usize,
    offset: usize,
    width: usize,
    relocation_name: &str,
) -> Result<Range<usize>, Diagnostic> {
    let end = offset.checked_add(width).ok_or_else(|| {
        Diagnostic::error(format!("{relocation_name} relocation offset overflow"))
    })?;
    if end > bytes_len {
        return Err(Diagnostic::error(format!(
            "{relocation_name} relocation offset {offset} is outside text section"
        )));
    }
    Ok(offset..end)
}

#[cfg(test)]
mod tests {
    use super::write_u64;

    #[test]
    fn reports_out_of_bounds_relocation_patches() {
        let mut bytes = [0u8; 4];
        let diagnostic =
            write_u64(&mut bytes, 1, 0x1234, "test").expect_err("patch should exceed text");

        assert!(
            diagnostic
                .message
                .contains("test relocation offset 1 is outside text section")
        );
    }
}
