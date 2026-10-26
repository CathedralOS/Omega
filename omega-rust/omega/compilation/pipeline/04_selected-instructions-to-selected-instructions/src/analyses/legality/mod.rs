//! Optimizer module role: executable entrance. Live ranges to allocation availability and physical-view legality.
//!
//! Each public route chooses one explicit availability policy. This entrance
//! then owns the shared analysis that records allocation-legality custody.

mod compute;
mod custody;
mod policies;

use crate::ValidatedAllocatorAvailability;

use crate::StagedOptimizedLiveRanges;
use crate::{
    AllocationLegalityError, AllocatorAvailabilityError, ValidatedAllocationLegality,
    ValidatedLiveRanges, ValidatedLiveness,
};
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationWorkBudget, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes::{AllocationLegalityIdentity, AllocatorAvailabilityIdentity};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target_operations_to_selected_instructions::{
    StagedOptimizedCustodyCore, ValidatedSelectedInstructions,
};
use terminal_psi::TerminalPsiIdentity;

pub fn stage_optimized_allocation_legality(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let availability = policies::all_environment_allocatable_views(&ranges)?;
    stage_optimized_allocation_legality_with_availability(ranges, availability)
}

/// Restrict unconstrained allocation to the selected convention's caller-saved
/// units while preserving authoritative fixed ABI and operand views.
pub(crate) fn stage_optimized_allocation_legality_for_frameless_leaf(
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

pub(crate) fn stage_optimized_allocation_legality_with_availability(
    ranges: StagedOptimizedLiveRanges,
    availability: ValidatedAllocatorAvailability,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    compute::compute_allocation_legality(ranges, availability)
}

/// Opt-in physical-view legality staging over complete live-range custody.
/// It records exact candidates and required fixed-view transitions, but grants
/// no splitting, copy insertion, home assignment, emission, or publication.
#[derive(Debug)]
pub struct StagedOptimizedAllocationLegality {
    ranges: StagedOptimizedLiveRanges,
    availability: ValidatedAllocatorAvailability,
    legality: ValidatedAllocationLegality,
    custody: StagedOptimizedAllocationLegalityCustodyReceipt,
}

impl StagedOptimizedAllocationLegality {
    /// The producer stage, which owns the optimized-target product later
    /// stages share; ordinary consumers read the current program and
    /// analyses directly.
    pub const fn live_range_stage(&self) -> &StagedOptimizedLiveRanges {
        &self.ranges
    }

    /// The live ranges over the current program.
    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        self.ranges.ranges()
    }

    /// The liveness facts over the current program.
    pub const fn liveness(&self) -> &ValidatedLiveness {
        self.ranges.liveness()
    }

    /// The current selected program this legality describes.
    pub const fn selected(&self) -> &ValidatedSelectedInstructions {
        self.ranges.selected()
    }

    /// The target register environment admitted with the current program.
    pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
        self.ranges.register_environment()
    }

    /// The governing optimizer selections for this admission.
    pub fn selections(&self) -> &OptimizationSelections {
        self.ranges.selections()
    }

    /// The per-pass work budget admitted beside the same evidence.
    pub fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.ranges.budget_per_pass()
    }

    pub const fn legality(&self) -> &ValidatedAllocationLegality {
        &self.legality
    }
    pub const fn allocator_availability(&self) -> &ValidatedAllocatorAvailability {
        &self.availability
    }
    receipt_accessor_rows!(ref StagedOptimizedAllocationLegality {
        custody: StagedOptimizedAllocationLegalityCustodyReceipt,
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedAllocationLegalityCustodyReceipt {
    core: StagedOptimizedCustodyCore,
    allocator_availability: AllocatorAvailabilityIdentity,
    selected: SelectedInstructionPlanIdentity,
    liveness: selected_instructions::LivenessIdentity,
    ranges: selected_instructions::LiveRangeIdentity,
    legality: AllocationLegalityIdentity,
}

impl StagedOptimizedAllocationLegalityCustodyReceipt {
    target_operations_to_selected_instructions::staging_custody_core_accessors!();

    receipt_accessor_rows!(StagedOptimizedAllocationLegalityCustodyReceipt {
        allocator_availability: AllocatorAvailabilityIdentity,
        ranges: selected_instructions::LiveRangeIdentity,
        legality: AllocationLegalityIdentity,
        selected: SelectedInstructionPlanIdentity,
        liveness: selected_instructions::LivenessIdentity,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedAllocationLegalityCustodyError {
    Availability(AllocatorAvailabilityError),
    Analysis(AllocationLegalityError),
    UnsupportedFramelessLeafConvention,
    MissingRequiredActiveResidentRematerializationView(&'static str),
}

failed_display!(
    OptimizedAllocationLegalityCustodyError,
    "optimized allocation-legality staging failed"
);

impl std::error::Error for OptimizedAllocationLegalityCustodyError {}
