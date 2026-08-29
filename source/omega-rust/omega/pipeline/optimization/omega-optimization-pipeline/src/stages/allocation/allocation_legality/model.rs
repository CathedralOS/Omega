use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use omega_regalloc::{
    AllocationLegalityError, AllocationLegalityIdentity, AllocatorAvailabilityError,
    AllocatorAvailabilityIdentity, ValidatedAllocationLegality, ValidatedAllocatorAvailability,
};
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::{OptimizedLiveRangeCustodyError, StagedOptimizedLiveRanges};

/// Opt-in physical-view legality staging over complete live-range custody.
/// It records exact candidates and required fixed-view transitions, but grants
/// no splitting, copy insertion, home assignment, emission, or publication.
#[derive(Debug)]
pub struct StagedOptimizedAllocationLegality {
    pub(super) ranges: StagedOptimizedLiveRanges,
    pub(super) availability: ValidatedAllocatorAvailability,
    pub(super) legality: ValidatedAllocationLegality,
    pub(super) custody: StagedOptimizedAllocationLegalityCustodyReceipt,
}

impl StagedOptimizedAllocationLegality {
    pub const fn live_range_stage(&self) -> &StagedOptimizedLiveRanges {
        &self.ranges
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
    pub(super) psi: TerminalPsiIdentity,
    pub(super) target: omega_target::NativeTarget,
    pub(super) entry: MachineId,
    pub(super) optimization: OptimizationIdentityBundleIdentity,
    pub(super) projection: OptimizedAbstractPlanProjectionIdentity,
    pub(super) manifest: PrePhysicalOptimizationManifestIdentity,
    pub(super) optimization_unit: OptimizationUnitIdentity,
    pub(super) fuel_schedule: FuelScheduleIdentity,
    pub(super) register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    pub(super) allocator_availability: AllocatorAvailabilityIdentity,
    pub(super) selected: SelectedInstructionPlanIdentity,
    pub(super) liveness: omega_regalloc::LivenessIdentity,
    pub(super) ranges: omega_regalloc::LiveRangeIdentity,
    pub(super) legality: AllocationLegalityIdentity,
    pub(super) function_count: usize,
    pub(super) structural_unit_function_count: usize,
    pub(super) virtual_register_count: usize,
    pub(super) point_count: usize,
    pub(super) candidate_count: usize,
    pub(super) entry_transition_count: usize,
}

impl StagedOptimizedAllocationLegalityCustodyReceipt {
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
    }
    pub const fn target(self) -> omega_target::NativeTarget {
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
    pub const fn register_environment(
        self,
    ) -> omega_register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn liveness(self) -> omega_regalloc::LivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> omega_regalloc::LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
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
