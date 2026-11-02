use machine_code::resolved_machine_layout_identity as layout_identity;
use selected_instructions::CurrentSelectedProgram;

use machine_code::SelectedFormEncoding;
use physical_instructions::PostAllocationMachineOptimizationCustody;
use physical_instructions::PostAllocationMachinePlan;

use super::super::OptimizedResolvedSelectedFormLayoutError as Error;
use super::super::{SelectedFunctionLayoutPolicy, StagedOptimizedResolvedSelectedFormLayout};

pub(super) fn validate_roots(
    selected: CurrentSelectedProgram<'_>,
    machine: &PostAllocationMachinePlan,
    pre_layout: &SelectedFormEncoding,
    optimization: Option<PostAllocationMachineOptimizationCustody>,
    policy: SelectedFunctionLayoutPolicy,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), Error> {
    if artifact.selected() != selected.identity
        || artifact.machine() != machine.identity
        || artifact.pre_layout() != pre_layout.identity()
        || artifact.post_allocation_machine_optimization() != optimization
        || artifact.target() != selected.plan.target
        || artifact.policy() != policy
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok(())
}

pub(super) fn validate_identity(
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), Error> {
    let identity = layout_identity(
        artifact.selected(),
        artifact.machine(),
        artifact.pre_layout(),
        artifact.post_allocation_machine_optimization(),
        artifact.target(),
        artifact.policy(),
        artifact.functions(),
    );
    if artifact.identity() != identity {
        return Err(Error::ArtifactMismatch);
    }
    Ok(())
}
