//! Alignment rounding for segment and blob boundaries.

pub(crate) fn alignment_power(alignment: usize) -> u32 {
    alignment.max(1).trailing_zeros()
}

pub(crate) use image::{align_to, align_to_u64};
