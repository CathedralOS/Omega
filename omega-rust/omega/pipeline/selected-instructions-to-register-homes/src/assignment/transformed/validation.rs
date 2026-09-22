use crate::assignment::post_allocation_manifest::validate_post_allocation_optimization_manifest_after_selected_lowering;
use crate::{validate_post_allocation_optimization_manifest, validate_register_homes};

use crate::{
    validate_optimized_literal_fold_custody, validate_selected_lowering_optimization_custody,
};

use super::custody::{literal_fold_home_custody_receipt, selected_lowering_home_custody_receipt};
use super::projection::{
    literal_fold_pre_physical, literal_fold_transformations, selected_lowering_final_analysis,
    selected_lowering_transformations,
};
use super::{
    OptimizedPostLiteralFoldHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

pub fn validate_optimized_register_home_after_literal_fold_custody(
    staged: &StagedOptimizedRegisterHomesAfterLiteralFolds,
) -> Result<
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    OptimizedPostLiteralFoldHomeCustodyError,
> {
    let source = validate_optimized_literal_fold_custody(&staged.folds)
        .map_err(OptimizedPostLiteralFoldHomeCustodyError::UpstreamFolds)?;
    let environment = staged.register_environment();
    let homes = validate_register_homes(
        staged.legality(),
        staged.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        staged.homes.plan().clone(),
    )
    .map_err(OptimizedPostLiteralFoldHomeCustodyError::Assignment)?;
    if homes.receipt() != staged.homes.receipt() {
        return Err(OptimizedPostLiteralFoldHomeCustodyError::ReceiptMismatch);
    }
    let transformations = literal_fold_transformations(&source);
    let manifest = validate_post_allocation_optimization_manifest(
        staged.manifest.record(),
        literal_fold_pre_physical(&source),
        &transformations,
        staged.ranges(),
        staged.legality(),
        &homes,
    )
    .map_err(OptimizedPostLiteralFoldHomeCustodyError::Manifest)?;
    let custody = literal_fold_home_custody_receipt(source, &homes, &manifest);
    if custody != staged.custody {
        return Err(OptimizedPostLiteralFoldHomeCustodyError::ReceiptMismatch);
    }
    Ok(custody)
}

pub fn validate_optimized_register_home_after_selected_lowering_custody(
    staged: &StagedOptimizedRegisterHomesAfterSelectedLowering,
) -> Result<
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    OptimizedPostSelectedLoweringHomeCustodyError,
> {
    let source = validate_selected_lowering_optimization_custody(&staged.run)
        .map_err(OptimizedPostSelectedLoweringHomeCustodyError::UpstreamSelectedLowering)?;
    let (ranges, legality) = selected_lowering_final_analysis(&staged.run);
    let environment = staged.register_environment();
    let homes = validate_register_homes(
        legality,
        ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        staged.homes.plan().clone(),
    )
    .map_err(OptimizedPostSelectedLoweringHomeCustodyError::Assignment)?;
    if homes.receipt() != staged.homes.receipt() {
        return Err(OptimizedPostSelectedLoweringHomeCustodyError::ReceiptMismatch);
    }
    let transformations = selected_lowering_transformations(&source);
    let manifest = validate_post_allocation_optimization_manifest_after_selected_lowering(
        staged.manifest.record(),
        source.source().manifest(),
        source.identity(),
        &transformations,
        ranges,
        legality,
        &homes,
    )
    .map_err(OptimizedPostSelectedLoweringHomeCustodyError::Manifest)?;
    let custody = selected_lowering_home_custody_receipt(source, &homes, &manifest);
    if custody != staged.custody {
        return Err(OptimizedPostSelectedLoweringHomeCustodyError::ReceiptMismatch);
    }
    Ok(custody)
}
