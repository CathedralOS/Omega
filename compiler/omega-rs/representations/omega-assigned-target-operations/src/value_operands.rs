use crate::AssignedValueHomeKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedValueOperand {
    pub kind: AssignedValueOperandKind,
    pub home: AssignedValueHomeKind,
}

impl Default for AssignedValueOperand {
    fn default() -> Self {
        Self {
            kind: AssignedValueOperandKind::Immediate(0),
            home: AssignedValueHomeKind::Immediate,
        }
    }
}

// The assigned layer shares the ONE canonical value-operand kind; it only adds a
// per-operand `home` (the wrapper struct above). The target->assigned conversion
// is a verbatim 1:1 arena copy (no handle remap), so it is now just a clone --
// the two hand-written `From` impls (which had become reflexive once the type was
// shared) are gone.
pub use omega_target_operations::TargetValueOperand as AssignedValueOperandKind;

pub type AssignedValueOperandHandle = omega_target_operations::TargetValueOperandHandle;
pub type RuntimeValueOperand = AssignedValueOperandKind;
pub type RuntimeValueOperandHandle = AssignedValueOperandHandle;
pub type TargetValueOperand = AssignedValueOperandKind;
pub type TargetValueOperandHandle = AssignedValueOperandHandle;

pub(crate) fn assigned_value_handle(
    handle: RuntimeValueOperandHandle,
) -> psi_arena::Handle<AssignedValueOperand> {
    psi_arena::Handle::from_parts(handle.arena_index(), handle.generation())
}

pub(crate) fn target_value_handle(
    handle: psi_arena::Handle<AssignedValueOperand>,
) -> RuntimeValueOperandHandle {
    psi_arena::Handle::from_parts(handle.arena_index(), handle.generation())
}
