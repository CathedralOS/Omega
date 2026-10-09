//! Checked arithmetic and little-endian reads for the dynamic lane, each
//! failing with a diagnostic that names the context it was checking.
use diagnostics::Diagnostic;

pub(super) fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

pub(super) fn checked_sum(
    left: usize,
    right: usize,
    context: &'static str,
) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

pub(super) fn checked_product(
    left: usize,
    right: usize,
    context: &'static str,
) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

pub(super) fn checked_u32(value: usize, context: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Word")))
}

fn read_array<const N: usize>(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<[u8; N], Diagnostic> {
    let end = checked_sum(offset, N, context)?;
    bytes
        .get(offset..end)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))
}

pub(super) fn read_u64(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u64, Diagnostic> {
    Ok(u64::from_le_bytes(read_array::<8>(bytes, offset, context)?))
}

pub(super) fn read_u16(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u16, Diagnostic> {
    Ok(u16::from_le_bytes(read_array::<2>(bytes, offset, context)?))
}

pub(super) fn read_u32(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u32, Diagnostic> {
    Ok(u32::from_le_bytes(read_array::<4>(bytes, offset, context)?))
}

pub(super) fn read_i64(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<i64, Diagnostic> {
    Ok(i64::from_le_bytes(read_array::<8>(bytes, offset, context)?))
}

pub(super) fn checked_sum_u64(
    left: u64,
    right: u64,
    context: &str,
    bound: &'static str,
) -> Result<u64, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows {bound}")))
}

pub(super) fn checked_product_u64(
    left: u64,
    right: u64,
    context: &str,
    bound: &'static str,
) -> Result<u64, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows {bound}")))
}
