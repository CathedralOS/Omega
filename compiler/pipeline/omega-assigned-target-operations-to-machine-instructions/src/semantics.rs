use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_machine_instructions::MachineInstructionSemanticSummary;

pub(crate) fn build_machine_instruction_semantic_summary(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> MachineInstructionSemanticSummary {
    assigned_target_operations.semantics.clone()
}
