use omega_optimization_core::{
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
};
use omega_regalloc::{
    PostAllocationOptimizationManifestError, PostAllocationSelectedTransformation,
    TerminalRegisterHomeError, TerminalRegisterHomeIdentity,
    ValidatedPostAllocationOptimizationManifest, ValidatedTerminalRegisterHomes,
    assign_terminal_register_homes, project_post_allocation_optimization_manifest,
    validate_post_allocation_optimization_manifest, validate_terminal_register_homes,
};

use crate::{
    OptimizedLiteralFoldCustodyError, StagedOptimizedLiteralFoldCustodyReceipt,
    StagedOptimizedLiteralFolds, validate_optimized_literal_fold_custody,
};

/// Strict homes after one or more separately requested literal folds. The
/// complete append-only fold chain remains owned and the manifest ledger is
/// derived from it rather than accepted from a caller.
#[derive(Debug)]
pub struct StagedOptimizedRegisterHomesAfterLiteralFolds {
    folds: StagedOptimizedLiteralFolds,
    homes: ValidatedTerminalRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterLiteralFolds {
    pub const fn fold_stage(&self) -> &StagedOptimizedLiteralFolds {
        &self.folds
    }
    pub const fn homes(&self) -> &ValidatedTerminalRegisterHomes {
        &self.homes
    }
    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> &StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
    source: StagedOptimizedLiteralFoldCustodyReceipt,
    homes: TerminalRegisterHomeIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    function_count: usize,
    assignment_count: usize,
}

impl StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
    pub const fn source(&self) -> &StagedOptimizedLiteralFoldCustodyReceipt {
        &self.source
    }
    pub const fn homes(&self) -> TerminalRegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(&self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn function_count(&self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(&self) -> usize {
        self.assignment_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostLiteralFoldHomeCustodyError {
    UpstreamFolds(OptimizedLiteralFoldCustodyError),
    Assignment(TerminalRegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedPostLiteralFoldHomeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized post-literal-fold home staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPostLiteralFoldHomeCustodyError {}

pub fn stage_optimized_register_homes_after_literal_folds(
    folds: StagedOptimizedLiteralFolds,
) -> Result<StagedOptimizedRegisterHomesAfterLiteralFolds, OptimizedPostLiteralFoldHomeCustodyError>
{
    let source = validate_optimized_literal_fold_custody(&folds)
        .map_err(OptimizedPostLiteralFoldHomeCustodyError::UpstreamFolds)?;
    let (homes, manifest) = build_homes_and_manifest(&folds, &source)?;
    let custody = custody_receipt(source, &homes, &manifest);
    Ok(StagedOptimizedRegisterHomesAfterLiteralFolds {
        folds,
        homes,
        manifest,
        custody,
    })
}

pub fn validate_optimized_register_home_after_literal_fold_custody(
    staged: &StagedOptimizedRegisterHomesAfterLiteralFolds,
) -> Result<
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    OptimizedPostLiteralFoldHomeCustodyError,
> {
    let source = validate_optimized_literal_fold_custody(&staged.folds)
        .map_err(OptimizedPostLiteralFoldHomeCustodyError::UpstreamFolds)?;
    let final_step = staged.folds.final_step();
    let environment = staged
        .folds
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let homes = validate_terminal_register_homes(
        final_step.legality(),
        final_step.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        staged.homes.plan().clone(),
    )
    .map_err(OptimizedPostLiteralFoldHomeCustodyError::Assignment)?;
    if homes.receipt() != staged.homes.receipt() {
        return Err(OptimizedPostLiteralFoldHomeCustodyError::ReceiptMismatch);
    }
    let transformations = transformations(&source);
    let manifest = validate_post_allocation_optimization_manifest(
        staged.manifest.record(),
        pre_physical(&source),
        &transformations,
        final_step.ranges(),
        final_step.legality(),
        &homes,
    )
    .map_err(OptimizedPostLiteralFoldHomeCustodyError::Manifest)?;
    let custody = custody_receipt(source, &homes, &manifest);
    if custody != staged.custody {
        return Err(OptimizedPostLiteralFoldHomeCustodyError::ReceiptMismatch);
    }
    Ok(custody)
}

fn build_homes_and_manifest(
    folds: &StagedOptimizedLiteralFolds,
    source: &StagedOptimizedLiteralFoldCustodyReceipt,
) -> Result<
    (
        ValidatedTerminalRegisterHomes,
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
    let homes = assign_terminal_register_homes(
        final_step.legality(),
        final_step.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedPostLiteralFoldHomeCustodyError::Assignment)?;
    let transformations = transformations(source);
    let manifest = project_post_allocation_optimization_manifest(
        pre_physical(source),
        &transformations,
        final_step.ranges(),
        final_step.legality(),
        &homes,
    )
    .map_err(OptimizedPostLiteralFoldHomeCustodyError::Manifest)?;
    Ok((homes, manifest))
}

fn transformations(
    source: &StagedOptimizedLiteralFoldCustodyReceipt,
) -> Vec<PostAllocationSelectedTransformation> {
    source
        .transformations()
        .iter()
        .copied()
        .map(PostAllocationSelectedTransformation::LiteralFold)
        .collect()
}

fn pre_physical(
    source: &StagedOptimizedLiteralFoldCustodyReceipt,
) -> PrePhysicalOptimizationManifestIdentity {
    source.source().manifest()
}

fn custody_receipt(
    source: StagedOptimizedLiteralFoldCustodyReceipt,
    homes: &ValidatedTerminalRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
) -> StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt {
        source,
        homes: homes.receipt().identity(),
        post_allocation_manifest: manifest.record().identity,
        function_count: homes.receipt().function_count(),
        assignment_count: homes.receipt().assignment_count(),
    }
}
