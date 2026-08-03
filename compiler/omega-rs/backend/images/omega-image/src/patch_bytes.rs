use psi_diagnostics::Diagnostic;

pub(crate) fn read_u32(
    bytes: &[u8],
    offset: usize,
    relocation_name: &str,
) -> Result<u32, Diagnostic> {
    let slice = relocation_slice(bytes, offset, 4, relocation_name)?;
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
    let slice = relocation_slice_mut(bytes, offset, value.len(), relocation_name)?;
    slice.copy_from_slice(value);
    Ok(())
}

fn relocation_slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    width: usize,
    relocation_name: &str,
) -> Result<&'a [u8], Diagnostic> {
    let end = offset.checked_add(width).ok_or_else(|| {
        Diagnostic::error(format!("{relocation_name} relocation offset overflow"))
    })?;
    bytes.get(offset..end).ok_or_else(|| {
        Diagnostic::error(format!(
            "{relocation_name} relocation offset {offset} is outside text section"
        ))
    })
}

fn relocation_slice_mut<'a>(
    bytes: &'a mut [u8],
    offset: usize,
    width: usize,
    relocation_name: &str,
) -> Result<&'a mut [u8], Diagnostic> {
    let end = offset.checked_add(width).ok_or_else(|| {
        Diagnostic::error(format!("{relocation_name} relocation offset overflow"))
    })?;
    bytes.get_mut(offset..end).ok_or_else(|| {
        Diagnostic::error(format!(
            "{relocation_name} relocation offset {offset} is outside text section"
        ))
    })
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
