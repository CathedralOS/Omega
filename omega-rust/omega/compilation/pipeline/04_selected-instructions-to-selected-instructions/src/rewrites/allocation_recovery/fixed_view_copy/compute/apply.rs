//! Selected-CFG copy insertion and operand rewriting mechanics.
use super::{RegisterInstructionConstraint, ScalarType};
use crate::{FixedViewCopy, FixedViewCopyError};

pub(super) fn apply_copy(
    function_index: usize,
    function: &mut selected_instructions::SelectedFunction,
    copy: &FixedViewCopy,
    row: &RegisterInstructionConstraint,
) -> Result<(), FixedViewCopyError> {
    super::super::work::apply_fixed_view_copy(function_index, function, copy, row, false)
}

/// `CopyI64` is a raw 64-bit register move: it preserves the source
/// bit-pattern without interpreting sign, so a fixed-view copy admits any
/// 64-bit integer source — signed or unsigned — and no narrower or
/// non-integer scalar.
pub(super) fn is_i64(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::Integer(integer) if integer.bits() == 64
    )
}
