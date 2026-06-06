// The operation kind is structurally identical to the target layer's -- the only
// thing the assigned layer adds is value-operand homes (see AssignedValueOperand).
// So share the ONE canonical operation kind and its `semantic_domain`
// classification (impl'd on the shared type in omega-target-operations) instead of
// re-declaring 63 variants and the whole match. The domain enum and the
// OperationSemanticQuery trait already live in shared omega-core.
pub use omega_target_operations::{
    TargetOperationDomain as AssignedOperationDomain, TargetOperationKind as AssignedOperationKind,
};

pub type SelectedInstructionKind = AssignedOperationKind;
pub type TargetOperationKind = AssignedOperationKind;
