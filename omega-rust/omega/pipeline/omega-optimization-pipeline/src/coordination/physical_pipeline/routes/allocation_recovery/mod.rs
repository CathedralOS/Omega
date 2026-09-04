//! Optimizer module role: executable entrance. Single physical route for the closed allocation-recovery catalog.

mod active_resident;
mod fixed_view;

use omega_optimization_core::Optimization;

use crate::{StagedOptimizedLiveRanges, StagedOptimizedVerifiedPhysicalPipeline};

use super::super::OptimizedVerifiedPhysicalPipelineError;
use active_resident::stage_active_resident;
use fixed_view::stage_fixed_view;

pub(in crate::coordination::physical_pipeline) fn stage_allocation_recovery_pipeline(
    ranges: StagedOptimizedLiveRanges,
    rule: Optimization,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    match rule {
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 => {
            stage_fixed_view(ranges)
        }
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1 => {
            stage_active_resident(ranges)
        }
        _ => Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition),
    }
}
