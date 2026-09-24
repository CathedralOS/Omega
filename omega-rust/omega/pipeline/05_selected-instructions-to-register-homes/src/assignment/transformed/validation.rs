use crate::assignment::post_allocation_manifest::{
    validate_post_allocation_optimization_manifest_after_pre_allocation,
    validate_post_allocation_optimization_manifest_after_selected_lowering,
};
use crate::{validate_post_allocation_optimization_manifest, validate_register_homes};

use selected_instructions_to_selected_instructions::{
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
    OptimizedPostSelectedLoweringHomeCustodyError,
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    StagedOptimizedPostPreAllocationHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomesAfterLiteralFolds, StagedOptimizedRegisterHomesAfterPreAllocation,
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

/// Independent replay of post-pre-allocation home staging: re-run the run's
/// custody validation, re-assign homes over the final ranges and legality,
/// and rebuild the manifest — including its pre-allocation completion — from
/// the replayed iteration receipts.
pub fn validate_optimized_register_home_after_pre_allocation_custody(
    staged: &StagedOptimizedRegisterHomesAfterPreAllocation,
) -> Result<
    StagedOptimizedPostPreAllocationHomeCustodyReceipt,
    OptimizedPostPreAllocationHomeCustodyError,
> {
    let source = validate_pre_allocation_optimization_custody(&staged.run)
        .map_err(OptimizedPostPreAllocationHomeCustodyError::UpstreamPreAllocation)?;
    let (ranges, legality) = pre_allocation_final_analysis(&staged.run);
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
    .map_err(OptimizedPostPreAllocationHomeCustodyError::Assignment)?;
    if homes.receipt() != staged.homes.receipt() {
        return Err(OptimizedPostPreAllocationHomeCustodyError::ReceiptMismatch);
    }
    let transformations = pre_allocation_transformations(&source);
    let manifest = validate_post_allocation_optimization_manifest_after_pre_allocation(
        staged.manifest.record(),
        source.source().manifest(),
        source.identity(),
        &transformations,
        ranges,
        legality,
        &homes,
    )
    .map_err(OptimizedPostPreAllocationHomeCustodyError::Manifest)?;
    let custody = pre_allocation_home_custody_receipt(source, &homes, &manifest);
    if custody != staged.custody {
        return Err(OptimizedPostPreAllocationHomeCustodyError::ReceiptMismatch);
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
