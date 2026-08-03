use crate::AssignedInstructionOperand;
use psi_arena::{Handle, HandleSpan};

pub(super) fn assigned_instruction_handle(
    handle: Handle<omega_target_operations::TargetInstructionOperand>,
) -> Handle<AssignedInstructionOperand> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

pub(super) fn assigned_instruction_span(
    span: HandleSpan<omega_target_operations::TargetInstructionOperand>,
) -> HandleSpan<AssignedInstructionOperand> {
    if span.is_empty() {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(assigned_instruction_handle(span.start()), span.count())
    }
}
