use machine_code::resolved_machine_layout_identity as layout_identity;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use physical_instructions::PostAllocationMachineOptimizationCustody;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use super::super::{
    OptimizedResolvedSelectedFormLayoutError, SelectedFunctionLayoutPolicy,
    StagedOptimizedResolvedSelectedFormLayout,
};

pub(super) fn validate_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<PostAllocationMachineOptimizationCustody>,
    policy: SelectedFunctionLayoutPolicy,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    if artifact.selected() != selected.selected_identity()
        || artifact.machine() != machine.machine().receipt().identity()
        || artifact.pre_layout() != pre_layout.identity()
        || artifact.post_allocation_machine_optimization() != optimization
        || artifact.target() != selected.selected_plan().target
        || artifact.policy() != policy
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    Ok(())
}

pub(super) fn validate_identity(
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let identity = layout_identity(
        artifact.selected(),
        artifact.machine(),
        artifact.pre_layout(),
        artifact.post_allocation_machine_optimization(),
        artifact.target(),
        artifact.policy(),
        artifact.functions(),
        artifact.structural_unit_functions(),
    );
    if artifact.identity() != identity {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    Ok(())
}
