//! Alignment rounding for segment and blob boundaries.

pub(super) fn alignment_power(alignment: usize) -> u32 {
    alignment.max(1).trailing_zeros()
}

pub(super) fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

pub(super) fn align_to_u64(value: u64, alignment: u64) -> u64 {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}
