mod compute;
mod error;
mod identity;
mod model;
mod optimization;
mod rules;
mod stage;
mod structural;

pub use error::OptimizedResolvedSelectedFormLayoutError;
pub use model::{
    ResolvedConditionalBranchEvidence, ResolvedSelectedBlockLayout,
    ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
    ResolvedStructuralUnitCallLayout, ResolvedStructuralUnitFunctionLayout,
    SelectedFunctionLayoutPolicy, StagedOptimizedResolvedSelectedFormLayout,
};
pub use stage::{
    stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    stage_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization,
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    validate_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization,
};

use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedSelectedFormEncoding,
};

use compute::compute;

/// Canonical resolved-layout join. The optional typed post-allocation result
/// owns rule identity while the layout retains one normalized custody token.
pub fn stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = compute(selected, machine, physical, pre_layout, optimization)?;
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        pre_layout,
        optimization,
        &artifact,
    )?;
    Ok(artifact)
}

/// Replay the canonical resolved-layout join against the typed rule result,
/// normalized pre-layout custody, byte accounting, and all resolved rows.
pub fn validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let replayed = compute(selected, machine, physical, pre_layout, optimization)?;
    if artifact != &replayed {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    Ok(())
}

pub fn stage_optimized_resolved_selected_form_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected, machine, physical, pre_layout, None,
    )
}

pub fn validate_optimized_resolved_selected_form_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected, machine, physical, pre_layout, None, artifact,
    )
}
