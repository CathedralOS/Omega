//! Optimizer module role: executable entrance. Allocation-availability and physical-view legality stage.
//!
//! Each public route chooses one explicit availability policy. This entrance
//! then owns the shared analysis-to-independent-replay join that grants
//! allocation-legality custody.

mod compute;
mod custody;
mod model;
mod policies;
mod validation;

pub use model::*;
pub use validation::validate_optimized_allocation_legality_custody;

use omega_regalloc::ValidatedAllocatorAvailability;

use crate::StagedOptimizedLiveRanges;

pub fn stage_optimized_allocation_legality(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let availability = policies::all_environment_allocatable_views(&ranges)?;
    stage_optimized_allocation_legality_with_availability(ranges, availability)
}

/// Restrict unconstrained allocation to the selected convention's caller-saved
/// units while preserving authoritative fixed ABI and operand views.
pub fn stage_optimized_allocation_legality_for_frameless_leaf(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let availability = policies::frameless_leaf_caller_saved_views(&ranges)?;
    stage_optimized_allocation_legality_with_availability(ranges, availability)
}

pub fn stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let availability =
        policies::active_resident_immediate_u64_multi_use_rematerialization_v1(&ranges)?;
    stage_optimized_allocation_legality_with_availability(ranges, availability)
}

pub fn stage_optimized_allocation_legality_with_availability(
    ranges: StagedOptimizedLiveRanges,
    availability: ValidatedAllocatorAvailability,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let staged = compute::compute_allocation_legality(ranges, availability)?;
    let custody = validate_optimized_allocation_legality_custody(
        staged.live_range_stage(),
        staged.allocator_availability(),
        staged.legality(),
    )?;
    if custody != staged.custody() {
        return Err(OptimizedAllocationLegalityCustodyError::ReceiptMismatch);
    }
    Ok(staged)
}
