#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Layout-independent selected-form encoding, replay, and optimization custody.
//!
//! This stage serializes selected instructions before any address-dependent
//! layout and retains normalized custody for optional machine rewrites.

use machine_code::{
    DeferredControlEncodingReason, SelectedFormDecodedFootprint, SelectedFormEncoding,
    SelectedFormEncodingCounts, SelectedFormEncodingIdentity, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormInternalMachineFixup,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
    SelectedFormMachineDisposition,
};
use physical_instructions::PostAllocationMachineOptimizationCustody;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use post_allocation_machine_to_post_allocation_machine::{
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedPostAllocationMachineOptimization,
};
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

mod compute;
mod custody;
mod error;
mod frame_address;
mod materialization;
mod model;
mod row_encoding;
mod stage;
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
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let artifact = compute::compute(selected, machine, physical, frame, optimization)?;
    validation::validate(selected, machine, physical, frame, optimization, &artifact)?;
    Ok(StagedOptimizedSelectedFormEncoding {
        program: std::sync::Arc::new(artifact),
    })
}

/// Replay the canonical selected-form encoding join against all retained
/// selected, physical-machine, and normalized optimization roots.
pub fn validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    frame: Option<&machine_code::TargetFrameLayoutPlan>,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    validation::validate(
        selected,
        machine,
        physical,
        frame,
        optimization,
        artifact.program(),
    )
}
