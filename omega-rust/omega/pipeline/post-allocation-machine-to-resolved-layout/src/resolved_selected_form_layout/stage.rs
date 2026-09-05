use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use post_allocation_machine_to_post_allocation_machine::{
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedPostAllocationMachineOptimization,
};
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use super::error::OptimizedResolvedSelectedFormLayoutError;
use super::model::StagedOptimizedResolvedSelectedFormLayout;
use super::{
    stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
};

/// Resolve and independently admit the baseline function-relative layout.
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

/// Independently admit a candidate baseline function-relative layout.
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

/// Resolve the validated symbolic CBNZ disposition after function-relative
/// offsets exist. The compare retains a zero-byte roster row and the branch is
/// independently target-decoded as CBNZ. The result remains separate
/// fragments with no emission, relocation, image, or publication authority.
pub fn stage_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    fusion: &StagedOptimizedAarch64CbnzFusion,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let optimization =
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(fusion.clone());
    stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        pre_layout,
        Some(&optimization),
    )
}

/// Independently reconstruct every offset, byte string, target footprint, and
/// symbolic-fusion custody field.
pub fn validate_optimized_resolved_selected_form_layout_after_aarch64_cbnz_fusion<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let optimization =
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(fusion.clone());
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        pre_layout,
        Some(&optimization),
        artifact,
    )
}

/// Carry an independently validated shortest-MOVN materialization through
/// required function-relative layout. Pre-layout already owns target-decoded
/// scalar bytes; this boundary independently rebuilds every offset and branch.
pub fn stage_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    materialization: &StagedOptimizedAarch64MovnMaterialization,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let optimization =
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization.clone());
    stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        pre_layout,
        Some(&optimization),
    )
}

/// Independently replay MOVN pre-layout custody plus every resolved offset,
/// branch byte sequence, and layout identity field.
pub fn validate_optimized_resolved_selected_form_layout_after_aarch64_movn_materialization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    materialization: &StagedOptimizedAarch64MovnMaterialization,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let optimization =
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization.clone());
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        pre_layout,
        Some(&optimization),
        artifact,
    )
}

/// Independently admit retained current data, without a producer-stage history.
/// Content identity alone is insufficient: replay checks the selected program,
/// machine, target decoding, offsets, fixups, and exact optimization records.
pub fn admit_resolved_machine_layout<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    program: std::sync::Arc<machine_code::ResolvedMachineLayout>,
) -> Result<StagedOptimizedResolvedSelectedFormLayout, OptimizedResolvedSelectedFormLayoutError> {
    let artifact = StagedOptimizedResolvedSelectedFormLayout { program };
    super::validation::validate(
        selected,
        machine,
        physical,
        pre_layout,
        optimization,
        &artifact,
    )?;
    Ok(artifact)
}
