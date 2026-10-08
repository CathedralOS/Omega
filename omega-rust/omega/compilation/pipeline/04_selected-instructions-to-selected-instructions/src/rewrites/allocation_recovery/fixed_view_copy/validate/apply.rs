use register_model::RegisterInstructionConstraint;

use crate::{FixedViewCopy, FixedViewCopyError};

pub(super) fn replay_apply(
    function_index: usize,
    function: &mut selected_instructions::SelectedFunction,
    copy: &FixedViewCopy,
    row: &RegisterInstructionConstraint,
) -> Result<(), FixedViewCopyError> {
    super::super::work::apply_fixed_view_copy(function_index, function, copy, row, true)
}
