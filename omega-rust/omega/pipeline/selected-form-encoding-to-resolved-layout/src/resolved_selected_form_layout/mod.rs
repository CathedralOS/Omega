//! Optimizer module role: executable entrance.
mod compute;
mod error;
mod model;
mod optimization;
mod ordinary;
mod stage;
mod structural;
mod validation;

pub use error::OptimizedResolvedSelectedFormLayoutError;
pub use model::{
    ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout, ResolvedStructuralUnitCallLayout,
    ResolvedStructuralUnitFunctionLayout, SelectedFunctionLayoutPolicy,
    StagedOptimizedResolvedSelectedFormLayout,
};
pub use stage::{
    admit_resolved_machine_layout, stage_optimized_resolved_selected_form_layout,
    stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    stage_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization,
    validate_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion,
    validate_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization,
};

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use post_allocation_machine_to_post_allocation_machine::StagedOptimizedPostAllocationMachineOptimization;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

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
    admit_resolved_machine_layout(
        selected,
        machine,
        physical,
        pre_layout,
        optimization,
        artifact.program,
    )
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
