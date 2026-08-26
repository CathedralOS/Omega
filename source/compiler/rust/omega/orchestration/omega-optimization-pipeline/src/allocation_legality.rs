use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use omega_regalloc::{
    TerminalAllocationLegalityError, TerminalAllocationLegalityIdentity,
    TerminalAllocatorAvailabilityError, TerminalAllocatorAvailabilityIdentity,
    TerminalAllocatorAvailabilityPolicy, ValidatedTerminalAllocationLegality,
    ValidatedTerminalAllocatorAvailability, analyze_terminal_allocation_legality,
    materialize_terminal_allocator_availability, validate_terminal_allocation_legality,
    validate_terminal_allocator_availability,
};
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::{
    OptimizedLiveRangeCustodyError, StagedOptimizedLiveRanges,
    validate_optimized_live_range_custody,
};

/// Opt-in physical-view legality staging over complete live-range custody.
/// It records exact candidates and required fixed-view transitions, but grants
/// no splitting, copy insertion, home assignment, emission, or publication.
#[derive(Debug)]
pub struct StagedOptimizedAllocationLegality {
    ranges: StagedOptimizedLiveRanges,
    availability: ValidatedTerminalAllocatorAvailability,
    legality: ValidatedTerminalAllocationLegality,
    custody: StagedOptimizedAllocationLegalityCustodyReceipt,
}

impl StagedOptimizedAllocationLegality {
    pub const fn live_range_stage(&self) -> &StagedOptimizedLiveRanges {
        &self.ranges
    }
    pub const fn legality(&self) -> &ValidatedTerminalAllocationLegality {
        &self.legality
    }
    pub const fn allocator_availability(&self) -> &ValidatedTerminalAllocatorAvailability {
        &self.availability
    }
    pub const fn custody(&self) -> StagedOptimizedAllocationLegalityCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedAllocationLegalityCustodyReceipt {
    terminal_psi: TerminalPsiIdentity,
    target: omega_target::NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    manifest: PrePhysicalOptimizationManifestIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    allocator_availability: TerminalAllocatorAvailabilityIdentity,
    selected: TerminalSelectedInstructionPlanIdentity,
    liveness: omega_regalloc::TerminalLivenessIdentity,
    ranges: omega_regalloc::TerminalLiveRangeIdentity,
    legality: TerminalAllocationLegalityIdentity,
    function_count: usize,
    virtual_register_count: usize,
    point_count: usize,
    candidate_count: usize,
    entry_transition_count: usize,
}

impl StagedOptimizedAllocationLegalityCustodyReceipt {
    pub const fn terminal_psi(self) -> TerminalPsiIdentity {
        self.terminal_psi
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
    pub const fn allocator_availability(self) -> TerminalAllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn liveness(self) -> omega_regalloc::TerminalLivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> omega_regalloc::TerminalLiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> TerminalAllocationLegalityIdentity {
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
    Availability(TerminalAllocatorAvailabilityError),
    Analysis(TerminalAllocationLegalityError),
    Revalidation(TerminalAllocationLegalityError),
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

pub fn stage_optimized_allocation_legality(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let availability = materialize_terminal_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        TerminalAllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Availability)?;
    stage_optimized_allocation_legality_with_availability(ranges, availability)
}

pub fn stage_optimized_allocation_legality_with_availability(
    ranges: StagedOptimizedLiveRanges,
    availability: ValidatedTerminalAllocatorAvailability,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let upstream = validate_optimized_live_range_custody(ranges.liveness_stage(), ranges.ranges())
        .map_err(OptimizedAllocationLegalityCustodyError::UpstreamLiveRanges)?;
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let replayed_availability = validate_terminal_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        availability.plan().clone(),
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Availability)?;
    if replayed_availability.receipt() != availability.receipt() {
        return Err(OptimizedAllocationLegalityCustodyError::ReceiptMismatch);
    }
    let legality = analyze_terminal_allocation_legality(
        ranges.ranges(),
        &availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Analysis)?;
    let replayed = validate_terminal_allocation_legality(
        ranges.ranges(),
        &availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        legality.plan().clone(),
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Revalidation)?;
    if replayed.receipt() != legality.receipt() {
        return Err(OptimizedAllocationLegalityCustodyError::ReceiptMismatch);
    }
    let custody = custody_receipt(
        upstream,
        availability.receipt().identity(),
        legality.receipt(),
    );
    Ok(StagedOptimizedAllocationLegality {
        ranges,
        availability,
        legality,
        custody,
    })
}

pub fn validate_optimized_allocation_legality_custody(
    ranges: &StagedOptimizedLiveRanges,
    availability: &ValidatedTerminalAllocatorAvailability,
    legality: &ValidatedTerminalAllocationLegality,
) -> Result<StagedOptimizedAllocationLegalityCustodyReceipt, OptimizedAllocationLegalityCustodyError>
{
    let upstream = validate_optimized_live_range_custody(ranges.liveness_stage(), ranges.ranges())
        .map_err(OptimizedAllocationLegalityCustodyError::UpstreamLiveRanges)?;
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let replayed_availability = validate_terminal_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        availability.plan().clone(),
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Availability)?;
    if replayed_availability.receipt() != availability.receipt() {
        return Err(OptimizedAllocationLegalityCustodyError::ReceiptMismatch);
    }
    let replayed = validate_terminal_allocation_legality(
        ranges.ranges(),
        availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        legality.plan().clone(),
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Revalidation)?;
    if replayed.receipt() != legality.receipt() {
        return Err(OptimizedAllocationLegalityCustodyError::ReceiptMismatch);
    }
    Ok(custody_receipt(
        upstream,
        availability.receipt().identity(),
        replayed.receipt(),
    ))
}

fn custody_receipt(
    upstream: crate::StagedOptimizedLiveRangeCustodyReceipt,
    allocator_availability: TerminalAllocatorAvailabilityIdentity,
    legality: omega_regalloc::TerminalAllocationLegalityValidationReceipt,
) -> StagedOptimizedAllocationLegalityCustodyReceipt {
    StagedOptimizedAllocationLegalityCustodyReceipt {
        terminal_psi: upstream.terminal_psi(),
        target: upstream.target(),
        entry: upstream.entry(),
        optimization: upstream.optimization(),
        projection: upstream.projection(),
        manifest: upstream.manifest(),
        optimization_unit: upstream.optimization_unit(),
        fuel_schedule: upstream.fuel_schedule(),
        register_environment: upstream.register_environment(),
        allocator_availability,
        selected: upstream.selected(),
        liveness: upstream.liveness(),
        ranges: upstream.ranges(),
        legality: legality.identity(),
        function_count: legality.function_count(),
        virtual_register_count: legality.virtual_register_count(),
        point_count: legality.point_count(),
        candidate_count: legality.candidate_count(),
        entry_transition_count: legality.entry_transition_count(),
    }
}
