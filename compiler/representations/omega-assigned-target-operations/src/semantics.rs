pub type AssignedBoundarySummary = omega_target_operations::TargetBoundarySummary;
pub type AssignedOwnershipSummary = omega_target_operations::TargetOwnershipSummary;
pub type AssignedValueSummary = omega_target_operations::TargetValueSummary;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssignedSemanticSummary {
    pub values: AssignedValueSummary,
    pub boundary_edges: AssignedBoundarySummary,
    pub ownership: AssignedOwnershipSummary,
}
