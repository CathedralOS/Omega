use crate::rewrites::selected_operands::selected_operand;
use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
};
use selected_instructions::{
    SelectedInstruction, SelectedInstructionKind, SelectedInstructionProvenance, SelectedOperand,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};

use crate::{FixedViewCopy, FixedViewCopyError};
use selected_instructions::VirtualFixedConstraintSite;

pub(super) fn replay_apply(
    function_index: usize,
    function: &mut selected_instructions::SelectedFunction,
    copy: &FixedViewCopy,
    row: &RegisterInstructionConstraint,
) -> Result<(), FixedViewCopyError> {
    super::super::work::apply_fixed_view_copy(function_index, function, copy, row, true)
}
