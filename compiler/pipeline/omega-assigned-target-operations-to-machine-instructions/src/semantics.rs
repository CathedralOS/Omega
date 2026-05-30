use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_machine_instructions::MachineInstructionSemanticSummary;

pub(crate) fn build_machine_instruction_semantic_summary(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> MachineInstructionSemanticSummary {
    MachineInstructionSemanticSummary::with_roots(
        assigned_target_operations.semantics.values.clone(),
        assigned_target_operations.semantics.boundaries.clone(),
        assigned_target_operations.semantics.ownership.clone(),
    )
}
