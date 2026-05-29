pub type MachineInstructionBoundarySummary =
    omega_assigned_target_operations::AssignedBoundarySummary;
pub type MachineInstructionOwnershipSummary =
    omega_assigned_target_operations::AssignedOwnershipSummary;
pub type MachineInstructionValueSummary = omega_assigned_target_operations::AssignedValueSummary;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineInstructionSemanticSummary {
    pub values: MachineInstructionValueSummary,
    pub boundary_edges: MachineInstructionBoundarySummary,
    pub ownership: MachineInstructionOwnershipSummary,
}
