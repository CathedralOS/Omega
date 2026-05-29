pub type TargetBoundarySummary = omega_abstract_operations::AbstractBoundarySummary;
pub type TargetOwnershipSummary = omega_abstract_operations::AbstractOwnershipSummary;
pub type TargetValueSummary = omega_abstract_operations::AbstractValueSummary;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TargetSemanticSummary {
    pub values: TargetValueSummary,
    pub boundary_edges: TargetBoundarySummary,
    pub ownership: TargetOwnershipSummary,
}
