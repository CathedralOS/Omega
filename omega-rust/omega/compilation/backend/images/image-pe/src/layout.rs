//! Alignment rounding for the two alignments PE keeps at once.

pub(crate) use image::align_to;

pub(crate) fn align_to_u32(value: u32, alignment: usize) -> u32 {
    let alignment = u32::try_from(alignment.max(1)).expect("alignment overflow");
    value.div_ceil(alignment) * alignment
}
