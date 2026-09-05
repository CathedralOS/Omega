#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Layout-independent selected-form encoding, replay, and optimization custody.
//!
//! This stage serializes selected instructions before any address-dependent
//! layout and retains normalized custody for optional machine rewrites.

use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use omega_post_allocation_machine_to_optimized_machine::{
    PostAllocationMachineOptimizationCustody, StagedOptimizedAarch64CbnzFusion,
    StagedOptimizedAarch64MovnMaterialization, StagedOptimizedPostAllocationMachineOptimization,
};
use omega_register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

mod compute;
mod custody;
mod error;
mod identity;
mod materialization;
mod model;
mod row_encoding;
mod stage;
mod structural_encoding;
mod validation;

pub use error::*;
pub use model::*;
pub use stage::*;

/// Canonical selected-form encoding join. The optional typed machine result
/// owns rule selection; the returned artifact retains only normalized custody.
pub fn stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let artifact = compute::compute(selected, machine, physical, optimization)?;
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        optimization,
        &artifact,
    )?;
    Ok(artifact)
}

/// Replay the canonical selected-form encoding join against all retained
/// selected, physical-machine, and normalized optimization roots.
pub fn validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    validation::validate(selected, machine, physical, optimization, artifact)
}
