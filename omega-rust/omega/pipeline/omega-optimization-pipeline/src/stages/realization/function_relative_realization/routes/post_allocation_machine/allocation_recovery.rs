//! Allocation-recovery lineage into the rule-independent post-allocation join.

use omega_optimization_core::{OptimizationExecutionPhase, OptimizationSelections};
use omega_regalloc::PostAllocationSelectedTransformation;

use super::{build_artifacts, validate_artifacts};
use crate::stages::realization::function_relative_realization::assembly::expected_post_allocation_machine_manifest;
use crate::{
    FunctionRelativeOptimizationRealizationError,
    PostAllocationMachineFunctionRelativeSourceCustody,
    StagedAllocationRecoveryFunctionRelativeSource, StagedAllocationRecoverySourceCustodyReceipt,
    StagedOptimizedPostAllocationMachineCustodyReceipt,
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedResolvedSelectedFormLayout, StagedOptimizedSelectedFormEncoding,
    StagedPostAllocationMachineFunctionRelativeRealization,
    StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt,
    StagedPostAllocationMachineFunctionRelativeSource,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
    validate_optimized_active_resident_rematerialization,
    validate_optimized_post_allocation_machine_optimization_custody,
    validate_optimized_post_allocation_machine_plan_custody,
    validate_optimized_register_home_after_fixed_view_copy_custody,
};

pub fn stage_post_allocation_machine_function_relative_realization_after_allocation_recovery(
    source: StagedAllocationRecoveryFunctionRelativeSource,
    machine: StagedOptimizedPostAllocationMachinePlan,
    optimization: StagedOptimizedPostAllocationMachineOptimization,
) -> Result<
    StagedPostAllocationMachineFunctionRelativeRealization,
    FunctionRelativeOptimizationRealizationError,
> {
    let source_custody = validate_source(&source)?;
    let machine_custody = validate_machine(&source, &machine)?;
    validate_optimization(&source, &machine, &optimization)?;
    let (baseline_encoding, encoding, baseline_layout, layout, exit_contract) =
        build_source_artifacts(&source, &machine, &optimization)?;
    let manifest = expected_manifest(
        &source,
        &machine,
        &optimization,
        &baseline_encoding,
        &encoding,
        &baseline_layout,
        &layout,
        &exit_contract,
    )?;
    let normalized = optimization
        .custody()
        .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)?;
    let custody = StagedPostAllocationMachineFunctionRelativeRealizationCustodyReceipt {
        source: PostAllocationMachineFunctionRelativeSourceCustody::AfterAllocationRecovery(
            source_custody,
        ),
        machine: machine_custody,
        optimization: normalized,
        exit_contract: exit_contract.identity(),
        realization: manifest.record().identity,
    };
    Ok(StagedPostAllocationMachineFunctionRelativeRealization {
        source: StagedPostAllocationMachineFunctionRelativeSource::AfterAllocationRecovery(source),
        machine,
        optimization,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
        manifest,
        custody,
    })
}

pub(super) fn validate_after_allocation_recovery(
    source: &StagedAllocationRecoveryFunctionRelativeSource,
    staged: &StagedPostAllocationMachineFunctionRelativeRealization,
) -> Result<
    (
        PostAllocationMachineFunctionRelativeSourceCustody,
        StagedOptimizedPostAllocationMachineCustodyReceipt,
        ValidatedFunctionRelativeOptimizationRealizationManifest,
    ),
    FunctionRelativeOptimizationRealizationError,
> {
    let source_custody = validate_source(source)?;
    let machine_custody = validate_machine(source, &staged.machine)?;
    validate_optimization(source, &staged.machine, &staged.optimization)?;
    validate_source_artifacts(source, staged)?;
    let manifest = expected_manifest(
        source,
        &staged.machine,
        &staged.optimization,
        &staged.baseline_encoding,
        &staged.encoding,
        &staged.baseline_layout,
        &staged.layout,
        &staged.exit_contract,
    )?;
    Ok((
        PostAllocationMachineFunctionRelativeSourceCustody::AfterAllocationRecovery(source_custody),
        machine_custody,
        manifest,
    ))
}

fn validate_source(
    source: &StagedAllocationRecoveryFunctionRelativeSource,
) -> Result<
    StagedAllocationRecoverySourceCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    match source {
        StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(homes) => {
            validate_optimized_register_home_after_fixed_view_copy_custody(
                homes.reanalysis_stage(),
                homes.homes(),
                homes.post_allocation_manifest(),
            )
            .map(StagedAllocationRecoverySourceCustodyReceipt::FixedViewCopies)
            .map_err(FunctionRelativeOptimizationRealizationError::FixedViewSource)
        }
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(source) => {
            validate_optimized_active_resident_rematerialization(source)
                .map(StagedAllocationRecoverySourceCustodyReceipt::ActiveResidentRematerialization)
                .map_err(FunctionRelativeOptimizationRealizationError::ActiveResidentSource)
        }
    }
}

fn validate_machine(
    source: &StagedAllocationRecoveryFunctionRelativeSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineCustodyReceipt,
    FunctionRelativeOptimizationRealizationError,
> {
    match source {
        StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(homes) => {
            validate_optimized_post_allocation_machine_plan_custody(homes, machine)
        }
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(source) => {
            validate_optimized_post_allocation_machine_plan_custody(source, machine)
        }
    }
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachine)
}

fn validate_optimization(
    source: &StagedAllocationRecoveryFunctionRelativeSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    match source {
        StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(_) => Err(
            crate::OptimizedPostAllocationMachineOptimizationError::UnsupportedPostAllocationMachineOptimization(
                optimization.optimization(),
            ),
        ),
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(source) => {
            validate_optimized_post_allocation_machine_optimization_custody(
                source,
                machine,
                optimization,
            )
        }
    }
    .map_err(FunctionRelativeOptimizationRealizationError::PostAllocationMachineOptimization)
}

fn build_source_artifacts(
    source: &StagedAllocationRecoveryFunctionRelativeSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
) -> Result<
    (
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedResolvedSelectedFormLayout,
        StagedOptimizedResolvedSelectedFormLayout,
        ValidatedWholeFunctionExitContract,
    ),
    FunctionRelativeOptimizationRealizationError,
> {
    let physical = source.register_environment().physical();
    match source {
        StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(homes) => build_artifacts(
            homes.reanalysis_stage().transformation_stage().copies(),
            machine,
            physical,
            optimization,
        ),
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(source) => {
            build_artifacts(source.rematerialization(), machine, physical, optimization)
        }
    }
}

fn validate_source_artifacts(
    source: &StagedAllocationRecoveryFunctionRelativeSource,
    staged: &StagedPostAllocationMachineFunctionRelativeRealization,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    let physical = source.register_environment().physical();
    match source {
        StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(homes) => {
            validate_artifacts(
                homes.reanalysis_stage().transformation_stage().copies(),
                &staged.machine,
                physical,
                &staged.optimization,
                staged,
            )
        }
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(source) => {
            validate_artifacts(
                source.rematerialization(),
                &staged.machine,
                physical,
                &staged.optimization,
                staged,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn expected_manifest(
    source: &StagedAllocationRecoveryFunctionRelativeSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let optimized = source.optimized_target().optimized();
    let selections = optimized.selections();
    let post = source.post_allocation_manifest().record();
    let expected_transformation = match source {
        StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(homes) => {
            PostAllocationSelectedTransformation::FixedViewCopy(
                homes
                    .reanalysis_stage()
                    .transformation_stage()
                    .copies()
                    .receipt()
                    .identity(),
            )
        }
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(source) => {
            PostAllocationSelectedTransformation::PressureRematerialization(
                source.rematerialization().receipt().identity(),
            )
        }
    };
    if selections.for_phase(OptimizationExecutionPhase::AllocationRecovery)
        != source.expected_allocation_recovery_selections()
        || !selections
            .for_phase(OptimizationExecutionPhase::SelectedLowering)
            .is_empty()
        || !selections
            .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
            .is_empty()
        || post.pre_physical != optimized.pre_physical_manifest().record().identity
        || post.selected_lowering_completion.is_some()
        || post.selected_transformations.as_slice() != [expected_transformation]
        || post.selected != source.selected_identity()
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    expected_post_allocation_machine_manifest(
        selections,
        OptimizationSelections::default().identity(),
        None,
        post.pre_physical,
        post.identity,
        post.selected,
        post.target,
        machine,
        optimization,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
    )
}
