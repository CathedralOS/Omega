use psi_arena::{Handle, HandleSpan};

// AssignedOperation is identical to TargetOperation -- the assigned layer adds
// value-operand homes, not operation fields. Share the one definition.
pub use omega_target_operations::TargetOperation as AssignedOperation;

pub type SelectedInstruction = AssignedOperation;
pub type TargetOperation = AssignedOperation;

// The assigned operation arena is a 1:1 copy of the target one (same indices), so
// these span translations are now the identity; kept as named helpers so the
// pipeline call sites stay unchanged.
pub fn assigned_operation_span_from_target(
    span: HandleSpan<omega_target_operations::TargetOperation>,
) -> HandleSpan<AssignedOperation> {
    if span.is_empty() {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(
            Handle::from_parts(span.start().arena_index(), span.start().generation()),
            span.count(),
        )
    }
}

pub fn target_operation_span_from_assigned(
    span: HandleSpan<AssignedOperation>,
) -> HandleSpan<omega_target_operations::TargetOperation> {
    if span.is_empty() {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(
            Handle::from_parts(span.start().arena_index(), span.start().generation()),
            span.count(),
        )
    }
}
