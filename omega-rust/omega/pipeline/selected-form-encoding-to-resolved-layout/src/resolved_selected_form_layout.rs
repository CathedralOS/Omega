//! Optimizer module role: executable entrance.
mod compute;
mod error;
mod model;
mod ordinary;
mod validation;

pub use error::OptimizedResolvedSelectedFormLayoutError;
pub use model::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedJumpEvidence, ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity,
    ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy,
    StagedOptimizedResolvedSelectedFormLayout,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

/// Resolve the current encoded program and independently admit its layout.
pub fn stage_optimized_resolved_selected_form_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = compute::compute(selected, machine, physical, pre_layout)?;
    admit_resolved_machine_layout(selected, machine, physical, pre_layout, artifact.program)
}

/// Replay the exact encoding roots, byte accounting, and all resolved rows.
pub fn validate_optimized_resolved_selected_form_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    validation::validate(selected, machine, physical, pre_layout, artifact)
}

/// Independently admit retained current data, without a producer-stage history.
/// Content identity alone is insufficient: replay checks the selected program,
/// machine, target decoding, offsets, fixups, and exact optimization records.
pub fn admit_resolved_machine_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    program: std::sync::Arc<machine_code::ResolvedMachineLayout>,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = StagedOptimizedResolvedSelectedFormLayout { program };
    validation::validate(selected, machine, physical, pre_layout, &artifact)?;
    Ok(artifact)
}
