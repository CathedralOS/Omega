pub type EncodedMachineBoundarySummary = omega_target_operations::TargetBoundarySummary;
pub type EncodedMachineOwnershipSummary = omega_target_operations::TargetOwnershipSummary;
pub type EncodedMachineValueSummary = omega_target_operations::TargetValueSummary;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EncodedMachineSemanticSummary {
    pub values: EncodedMachineValueSummary,
    pub boundary_edges: EncodedMachineBoundarySummary,
    pub ownership: EncodedMachineOwnershipSummary,
}
