pub type MachineBoundarySummary = omega_target_operations::TargetBoundarySummary;
pub type MachineOwnershipSummary = omega_target_operations::TargetOwnershipSummary;
pub type MachineValueSummary = omega_target_operations::TargetValueSummary;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineSemanticSummary {
    pub values: MachineValueSummary,
    pub boundary_edges: MachineBoundarySummary,
    pub ownership: MachineOwnershipSummary,
}
