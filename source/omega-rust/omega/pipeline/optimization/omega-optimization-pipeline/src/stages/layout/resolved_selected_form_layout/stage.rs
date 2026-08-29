use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedSelectedFormEncoding,
};

use super::error::OptimizedResolvedSelectedFormLayoutError;
use super::model::StagedOptimizedResolvedSelectedFormLayout;
use super::{
    stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
};

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
