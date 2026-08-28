use omega_target_operations::{
    TargetInstructionOperand, TargetOperation, TargetValueOperandHandle,
};
use psi_arena::{Handle, HandleSpan};

pub(crate) fn instruction_handle(
    handle: Handle<omega_abstract_operations::AbstractOperation>,
) -> Handle<TargetOperation> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

pub(crate) fn instruction_span(
    span: HandleSpan<omega_abstract_operations::AbstractOperation>,
) -> HandleSpan<TargetOperation> {
    if span.is_empty() {
        return HandleSpan::empty();
    }

    HandleSpan::from_parts(
        Handle::from_parts(span.start().arena_index(), span.start().generation()),
        span.count(),
    )
}

pub(crate) fn operand_span(
    span: HandleSpan<omega_abstract_operations::InstructionOperand>,
) -> HandleSpan<TargetInstructionOperand> {
    if span.is_empty() {
        return HandleSpan::empty();
    }

    HandleSpan::from_parts(
        Handle::from_parts(span.start().arena_index(), span.start().generation()),
        span.count(),
    )
}

pub(crate) fn operand_handle(
    handle: Handle<omega_abstract_operations::InstructionOperand>,
) -> Handle<TargetInstructionOperand> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

pub(crate) fn runtime_value_handle(
    handle: omega_abstract_operations::AbstractValueOperandHandle,
) -> TargetValueOperandHandle {
    Handle::from_parts(handle.arena_index(), handle.generation())
}
