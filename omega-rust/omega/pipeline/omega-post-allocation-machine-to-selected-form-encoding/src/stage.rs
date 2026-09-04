use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedAarch64MovnMaterialization,
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
};

use super::{
    OptimizedSelectedFormEncodingError, StagedOptimizedSelectedFormEncoding,
    stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization,
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization,
};

pub fn stage_optimized_layout_independent_selected_form_encoding<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected, machine, physical, None,
    )
}

pub fn validate_optimized_layout_independent_selected_form_encoding<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected, machine, physical, None, artifact,
    )
}

/// Bind an independently validated symbolic CBNZ disposition into pre-layout
/// custody. Scalar bytes still validate the source forms; the disposition, not
/// this artifact, authorizes the resolved layout to omit or replace them.
/// This grants no layout, emission, section, or publication authority.
pub fn stage_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: &StagedOptimizedAarch64CbnzFusion,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let optimization =
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(fusion.clone());
    stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        Some(&optimization),
    )
}

/// Replay the complete pre-layout roster and machine-optimization custody.
pub fn validate_optimized_layout_independent_selected_form_encoding_after_aarch64_cbnz_fusion<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: &StagedOptimizedAarch64CbnzFusion,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let optimization =
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(fusion.clone());
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        Some(&optimization),
        artifact,
    )
}

/// Apply an independently validated shortest-MOVN recipe to pre-layout scalar
/// bytes. The artifact still grants no layout, emission, or publication
/// authority.
pub fn stage_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    materialization: &StagedOptimizedAarch64MovnMaterialization,
) -> Result<StagedOptimizedSelectedFormEncoding, OptimizedSelectedFormEncodingError> {
    let optimization =
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization.clone());
    stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        Some(&optimization),
    )
}

pub fn validate_optimized_layout_independent_selected_form_encoding_after_aarch64_movn_materialization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    materialization: &StagedOptimizedAarch64MovnMaterialization,
    artifact: &StagedOptimizedSelectedFormEncoding,
) -> Result<(), OptimizedSelectedFormEncodingError> {
    let optimization =
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization.clone());
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        Some(&optimization),
        artifact,
    )
}
