//! Optimizer module role: executable entrance. Live ranges to allocation availability and physical-view legality.
//!
//! Each public route chooses one explicit availability policy. This entrance
//! then owns the shared analysis-to-independent-replay join that grants
//! allocation-legality custody.

mod compute;
mod custody;
mod policies;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

#[cfg(any(test, feature = "test-support"))]
pub use test_support::*;
pub use validation::validate_optimized_allocation_legality_custody;

use crate::ValidatedAllocatorAvailability;

use crate::StagedOptimizedLiveRanges;
use crate::{
    AllocationLegalityError, AllocatorAvailabilityError, OptimizedLiveRangeCustodyError,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
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
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;
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
    /// The retained producer stage. Replay and custody validation inspect it;
    /// ordinary consumers read the current program and analyses directly.
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
    pub const fn custody(&self) -> StagedOptimizedAllocationLegalityCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedAllocationLegalityCustodyReceipt {
    psi: TerminalPsiIdentity,
    target: target::NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    manifest: PrePhysicalOptimizationManifestIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    register_environment: register_model::TargetRegisterEnvironmentIdentity,
    allocator_availability: AllocatorAvailabilityIdentity,
    selected: SelectedInstructionPlanIdentity,
    liveness: selected_instructions::LivenessIdentity,
    ranges: selected_instructions::LiveRangeIdentity,
    legality: AllocationLegalityIdentity,
    function_count: usize,
    virtual_register_count: usize,
    point_count: usize,
    candidate_count: usize,
    entry_transition_count: usize,
}

impl StagedOptimizedAllocationLegalityCustodyReceipt {
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
    }
    pub const fn target(self) -> target::NativeTarget {
        self.target
    }
    pub const fn entry(self) -> MachineId {
        self.entry
    }
    pub const fn optimization(self) -> OptimizationIdentityBundleIdentity {
        self.optimization
    }
    pub const fn projection(self) -> OptimizedAbstractPlanProjectionIdentity {
        self.projection
    }
    pub const fn manifest(self) -> PrePhysicalOptimizationManifestIdentity {
        self.manifest
    }
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn register_environment(self) -> register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn liveness(self) -> selected_instructions::LivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> selected_instructions::LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn point_count(self) -> usize {
        self.point_count
    }
    pub const fn candidate_count(self) -> usize {
        self.candidate_count
    }
    pub const fn entry_transition_count(self) -> usize {
        self.entry_transition_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedAllocationLegalityCustodyError {
    UpstreamLiveRanges(OptimizedLiveRangeCustodyError),
    Availability(AllocatorAvailabilityError),
    Analysis(AllocationLegalityError),
    Revalidation(AllocationLegalityError),
    UnsupportedFramelessLeafConvention,
    MissingRequiredActiveResidentRematerializationView(&'static str),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedAllocationLegalityCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized allocation-legality staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedAllocationLegalityCustodyError {}
