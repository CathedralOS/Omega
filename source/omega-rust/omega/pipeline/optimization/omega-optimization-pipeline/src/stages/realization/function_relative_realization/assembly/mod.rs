//! Optimizer module role: executable entrance.
use super::prelude::*;
use super::{error::*, model::*};

mod custody;
mod manifests;
mod rel8;
mod statistics;

pub(super) use custody::*;
pub(super) use manifests::{
    expected_direct_manifest, expected_direct_post_allocation_machine_manifest, expected_manifest,
    expected_selected_lowering_post_allocation_machine_manifest,
};
pub(super) use rel8::*;
pub(crate) use statistics::{function_relative_statistics, seal_function_relative_manifest};

pub(super) fn build_realization<S: ValidatedSelectedAnalysis>(
    selected: &S,
    homes: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<
    (
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedResolvedSelectedFormLayout,
        Option<StagedOptimizedX86BranchRelaxation>,
        ValidatedWholeFunctionExitContract,
        ValidatedFunctionRelativeOptimizationRealizationManifest,
    ),
    FunctionRelativeOptimizationRealizationError,
> {
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, machine, physical)
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    let baseline_layout =
        stage_optimized_resolved_selected_form_layout(selected, machine, physical, &encoding)
            .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    let relaxation = stage_selected_relaxation(
        selected,
        machine,
        physical,
        &encoding,
        &baseline_layout,
        selections,
        budget,
    )?;
    let exit_contract = stage_exit_contract(
        selected,
        machine,
        physical,
        &encoding,
        &baseline_layout,
        relaxation.as_ref(),
    )?;
    let manifest = expected_manifest(
        homes,
        machine,
        &encoding,
        &baseline_layout,
        relaxation.as_ref(),
        &exit_contract,
    )?;
    Ok((
        encoding,
        baseline_layout,
        relaxation,
        exit_contract,
        manifest,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_realization_artifacts<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
    exit_contract: &ValidatedWholeFunctionExitContract,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    validate_optimized_layout_independent_selected_form_encoding(
        selected, machine, physical, encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_selected_relaxation(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
        relaxation,
        selections,
    )?;
    validate_exit_contract(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
        relaxation,
        exit_contract,
    )
}
