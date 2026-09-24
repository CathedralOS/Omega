use crate::assignment::post_allocation_manifest::{
    project_post_allocation_optimization_manifest,
    project_post_allocation_optimization_manifest_after_pre_allocation,
    project_post_allocation_optimization_manifest_after_selected_lowering,
};
use crate::{
    ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes, assign_register_homes,
};

use selected_instructions_to_selected_instructions::{
    StagedOptimizedLiteralFoldCustodyReceipt, StagedOptimizedLiteralFolds,
    StagedPreAllocationOptimizationRun, StagedSelectedLoweringOptimizationRun,
    validate_optimized_literal_fold_custody, validate_pre_allocation_optimization_custody,
    validate_selected_lowering_optimization_custody,
};

use super::custody::{
    literal_fold_home_custody_receipt, pre_allocation_home_custody_receipt,
    selected_lowering_home_custody_receipt,
};
use super::projection::{
    literal_fold_pre_physical, literal_fold_transformations, pre_allocation_final_analysis,
    pre_allocation_transformations, selected_lowering_final_analysis,
    selected_lowering_transformations,
};
use super::{
    OptimizedPostLiteralFoldHomeCustodyError, OptimizedPostPreAllocationHomeCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyError, StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterPreAllocation,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

pub(super) fn construct_register_homes_after_literal_folds(
    folds: StagedOptimizedLiteralFolds,
) -> Result<StagedOptimizedRegisterHomesAfterLiteralFolds, OptimizedPostLiteralFoldHomeCustodyError>
{
    let source = validate_optimized_literal_fold_custody(&folds)
        .map_err(OptimizedPostLiteralFoldHomeCustodyError::UpstreamFolds)?;
    let (homes, manifest) = build_homes_and_manifest(&folds, &source)?;
    let custody = literal_fold_home_custody_receipt(source, &homes, &manifest);
    Ok(StagedOptimizedRegisterHomesAfterLiteralFolds {
        folds,
        homes,
        manifest,
        custody,
    })
}

fn build_homes_and_manifest(
    folds: &StagedOptimizedLiteralFolds,
    source: &StagedOptimizedLiteralFoldCustodyReceipt,
) -> Result<
    (
        ValidatedRegisterHomes,
        ValidatedPostAllocationOptimizationManifest,
    ),
    OptimizedPostLiteralFoldHomeCustodyError,
> {
    let final_step = folds.final_step();
    let environment = folds.source_legality_stage().register_environment();
    let homes = assign_register_homes(
        final_step.legality(),
        final_step.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedPostLiteralFoldHomeCustodyError::Assignment)?;
    let transformations = literal_fold_transformations(source);
    let manifest = project_post_allocation_optimization_manifest(
        literal_fold_pre_physical(source),
        &transformations,
        final_step.ranges(),
        final_step.legality(),
        &homes,
    )
    .map_err(OptimizedPostLiteralFoldHomeCustodyError::Manifest)?;
    Ok((homes, manifest))
}

/// Strict homes after a completed pre-allocation run. The run's retained
/// custody is independently replayed before its transformed program and
/// rebuilt analyses are trusted; the manifest binds the completion identity
/// and every committed copy-removal iteration.
pub(super) fn construct_register_homes_after_pre_allocation(
    run: StagedPreAllocationOptimizationRun,
) -> Result<
    StagedOptimizedRegisterHomesAfterPreAllocation,
    OptimizedPostPreAllocationHomeCustodyError,
> {
    let source = validate_pre_allocation_optimization_custody(&run)
        .map_err(OptimizedPostPreAllocationHomeCustodyError::UpstreamPreAllocation)?;
    let (ranges, legality) = pre_allocation_final_analysis(&run);
    let environment = run.source_legality_stage().register_environment();
    let homes = assign_register_homes(
        legality,
        ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedPostPreAllocationHomeCustodyError::Assignment)?;
    let transformations = pre_allocation_transformations(&source);
    let manifest = project_post_allocation_optimization_manifest_after_pre_allocation(
        source.source().manifest(),
        source.identity(),
        &transformations,
        ranges,
        legality,
        &homes,
    )
    .map_err(OptimizedPostPreAllocationHomeCustodyError::Manifest)?;
    let custody = pre_allocation_home_custody_receipt(source, &homes, &manifest);
    Ok(StagedOptimizedRegisterHomesAfterPreAllocation {
        run,
        homes,
        manifest,
        custody,
    })
}

pub(super) fn construct_register_homes_after_selected_lowering(
    run: StagedSelectedLoweringOptimizationRun,
) -> Result<
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    OptimizedPostSelectedLoweringHomeCustodyError,
> {
    let source = validate_selected_lowering_optimization_custody(&run)
        .map_err(OptimizedPostSelectedLoweringHomeCustodyError::UpstreamSelectedLowering)?;
    let (ranges, legality) = selected_lowering_final_analysis(&run);
    let environment = run.source_legality_stage().register_environment();
    let homes = assign_register_homes(
        legality,
        ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedPostSelectedLoweringHomeCustodyError::Assignment)?;
    let transformations = selected_lowering_transformations(&source);
    let manifest = project_post_allocation_optimization_manifest_after_selected_lowering(
        source.source().manifest(),
        source.identity(),
        &transformations,
        ranges,
        legality,
        &homes,
    )
    .map_err(OptimizedPostSelectedLoweringHomeCustodyError::Manifest)?;
    let custody = selected_lowering_home_custody_receipt(source, &homes, &manifest);
    Ok(StagedOptimizedRegisterHomesAfterSelectedLowering {
        run,
        homes,
        manifest,
        custody,
    })
}
