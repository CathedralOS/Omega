use crate::{
    ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes, assign_register_homes,
    project_post_allocation_optimization_manifest,
    project_post_allocation_optimization_manifest_after_selected_lowering,
};

use crate::{
    LiteralFoldCustodyReceipt, StagedOptimizedLiteralFolds, StagedSelectedLoweringOptimizationRun,
    validate_optimized_literal_fold_custody, validate_selected_lowering_optimization_custody,
};

use super::custody::{literal_fold_home_custody_receipt, selected_lowering_home_custody_receipt};
use super::model::{
    OptimizedPostLiteralFoldHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};
use super::projection::{
    literal_fold_pre_physical, literal_fold_transformations, selected_lowering_final_analysis,
    selected_lowering_transformations,
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
    source: &LiteralFoldCustodyReceipt,
) -> Result<
    (
        ValidatedRegisterHomes,
        ValidatedPostAllocationOptimizationManifest,
    ),
    OptimizedPostLiteralFoldHomeCustodyError,
> {
    let final_step = folds.final_step();
    let environment = folds
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let homes = assign_register_homes(
        final_step.legality(),
        final_step.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
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

pub(super) fn construct_register_homes_after_selected_lowering(
    run: StagedSelectedLoweringOptimizationRun,
) -> Result<
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    OptimizedPostSelectedLoweringHomeCustodyError,
> {
    let source = validate_selected_lowering_optimization_custody(&run)
        .map_err(OptimizedPostSelectedLoweringHomeCustodyError::UpstreamSelectedLowering)?;
    let (ranges, legality) = selected_lowering_final_analysis(&run);
    let environment = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let homes = assign_register_homes(
        legality,
        ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
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
