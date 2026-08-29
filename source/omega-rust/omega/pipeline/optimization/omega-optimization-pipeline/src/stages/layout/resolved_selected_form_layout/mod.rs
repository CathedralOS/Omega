mod compute;
mod error;
mod identity;
mod model;
mod optimization;
mod ordinary;
mod stage;
mod structural;
mod validation;

pub use error::OptimizedResolvedSelectedFormLayoutError;
pub use model::{
    ResolvedConditionalBranchEvidence, ResolvedSelectedBlockLayout,
    ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
    ResolvedStructuralUnitCallLayout, ResolvedStructuralUnitFunctionLayout,
    SelectedFunctionLayoutPolicy, StagedOptimizedResolvedSelectedFormLayout,
};
pub use stage::{
    stage_optimized_resolved_selected_form_layout,
    stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    stage_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization,
    validate_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    validate_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization,
};

use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedSelectedFormEncoding,
};

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
    let artifact = compute::compute(selected, machine, physical, pre_layout, optimization)?;
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
    validation::validate(
        selected,
        machine,
        physical,
        pre_layout,
        optimization,
        artifact,
    )
}
