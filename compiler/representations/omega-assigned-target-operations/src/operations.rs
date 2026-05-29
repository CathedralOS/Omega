use crate::AssignedOperationKind;
use omega_control_flow::StateKey;
use omega_core::arena::{Handle, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedOperation {
    pub kind: AssignedOperationKind,
    pub source_key: StateKey,
    pub source_statement: usize,
}

pub type SelectedInstruction = AssignedOperation;
pub type TargetOperation = AssignedOperation;

impl Default for AssignedOperation {
    fn default() -> Self {
        Self {
            kind: AssignedOperationKind::EnterFunction,
            source_key: StateKey::default(),
            source_statement: 0,
        }
    }
}

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
