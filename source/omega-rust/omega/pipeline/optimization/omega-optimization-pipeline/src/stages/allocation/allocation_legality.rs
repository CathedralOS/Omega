use omega_isa_aarch64::aarch64_preservation_convention_for_target;
use omega_isa_x86_64::x86_64_preservation_convention_for_target;
use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use omega_regalloc::{
    AllocationLegalityError, AllocationLegalityIdentity, AllocatorAvailabilityError,
    AllocatorAvailabilityIdentity, AllocatorAvailabilityPolicy, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, analyze_allocation_legality,
    materialize_allocator_availability, validate_allocation_legality,
    validate_allocator_availability,
};
use omega_selected_instructions::SelectedInstructionPlanIdentity;
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
    availability: ValidatedAllocatorAvailability,
    legality: ValidatedAllocationLegality,
    custody: StagedOptimizedAllocationLegalityCustodyReceipt,
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
    psi: TerminalPsiIdentity,
    target: omega_target::NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    manifest: PrePhysicalOptimizationManifestIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    allocator_availability: AllocatorAvailabilityIdentity,
    selected: SelectedInstructionPlanIdentity,
    liveness: omega_regalloc::LivenessIdentity,
    ranges: omega_regalloc::LiveRangeIdentity,
    legality: AllocationLegalityIdentity,
    function_count: usize,
    structural_unit_function_count: usize,
    virtual_register_count: usize,
    point_count: usize,
    candidate_count: usize,
    entry_transition_count: usize,
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

pub fn stage_optimized_allocation_legality(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let availability = materialize_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Availability)?;
    stage_optimized_allocation_legality_with_availability(ranges, availability)
}

/// Restrict unconstrained allocation to the exact selected convention's
/// caller-saved units. This is the required search policy for the current
/// frameless leaf lane; fixed ABI/operand views remain authoritative.
pub fn stage_optimized_allocation_legality_for_frameless_leaf(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let convention = match environment.target().architecture {
        omega_target::Architecture::X86_64 => {
            x86_64_preservation_convention_for_target(environment.physical(), environment.target())
        }
        omega_target::Architecture::Aarch64 => {
            aarch64_preservation_convention_for_target(environment.physical(), environment.target())
        }
    }
    .ok_or(OptimizedAllocationLegalityCustodyError::UnsupportedFramelessLeafConvention)?;
    let caller_saved = convention
        .caller_saved
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let views = environment
        .physical()
        .model()
        .views
        .iter()
        .filter(|view| {
            view.allocatable
                && view
                    .units
                    .iter()
                    .chain(&view.write_units)
                    .all(|unit| caller_saved.contains(unit))
        })
        .map(|view| view.id)
        .collect::<Vec<_>>();
    let availability = materialize_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views },
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Availability)?;
    stage_optimized_allocation_legality_with_availability(ranges, availability)
}

/// Restrict unconstrained allocation to the exact two caller-saved views used
/// by the v1 active-resident immediate-u64 multi-use rematerialization policy.
/// This deliberately creates the closed pressure case the policy is defined
/// to recover; it is not a general allocator cost or availability policy.
pub fn stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let views = active_resident_immediate_u64_multi_use_rematerialization_v1_views(
        environment.target().architecture,
        environment.physical().model(),
    )?;
    let availability = materialize_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views },
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Availability)?;
    stage_optimized_allocation_legality_with_availability(ranges, availability)
}

fn active_resident_immediate_u64_multi_use_rematerialization_v1_views(
    architecture: omega_target::Architecture,
    model: &omega_register_model::PhysicalRegisterModel,
) -> Result<Vec<omega_register_model::RegisterViewId>, OptimizedAllocationLegalityCustodyError> {
    let names = match architecture {
        omega_target::Architecture::X86_64 => ["rax", "rcx"],
        omega_target::Architecture::Aarch64 => ["x0", "x1"],
    };
    names
        .into_iter()
        .map(|name| {
            model
                .view_named(name)
                .map(|view| view.id)
                .ok_or(
                    OptimizedAllocationLegalityCustodyError::MissingRequiredActiveResidentRematerializationView(
                        name,
                    ),
                )
        })
        .collect()
}

pub fn stage_optimized_allocation_legality_with_availability(
    ranges: StagedOptimizedLiveRanges,
    availability: ValidatedAllocatorAvailability,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let upstream = validate_optimized_live_range_custody(ranges.liveness_stage(), ranges.ranges())
        .map_err(OptimizedAllocationLegalityCustodyError::UpstreamLiveRanges)?;
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let replayed_availability = validate_allocator_availability(
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
    let legality = analyze_allocation_legality(
        ranges.ranges(),
        &availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Analysis)?;
    let replayed = validate_allocation_legality(
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
    availability: &ValidatedAllocatorAvailability,
    legality: &ValidatedAllocationLegality,
) -> Result<StagedOptimizedAllocationLegalityCustodyReceipt, OptimizedAllocationLegalityCustodyError>
{
    let upstream = validate_optimized_live_range_custody(ranges.liveness_stage(), ranges.ranges())
        .map_err(OptimizedAllocationLegalityCustodyError::UpstreamLiveRanges)?;
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let replayed_availability = validate_allocator_availability(
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
    let replayed = validate_allocation_legality(
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
    allocator_availability: AllocatorAvailabilityIdentity,
    legality: omega_regalloc::AllocationLegalityValidationReceipt,
) -> StagedOptimizedAllocationLegalityCustodyReceipt {
    StagedOptimizedAllocationLegalityCustodyReceipt {
        psi: upstream.psi(),
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
        structural_unit_function_count: legality.structural_unit_function_count(),
        virtual_register_count: legality.virtual_register_count(),
        point_count: legality.point_count(),
        candidate_count: legality.candidate_count(),
        entry_transition_count: legality.entry_transition_count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_resident_two_view_policy_reports_the_exact_missing_required_view() {
        let mut model = omega_isa_x86_64::x86_64_physical_register_model();
        model.views.retain(|view| view.name != "rcx");

        assert_eq!(
            active_resident_immediate_u64_multi_use_rematerialization_v1_views(
                omega_target::Architecture::X86_64,
                &model,
            ),
            Err(
                OptimizedAllocationLegalityCustodyError::MissingRequiredActiveResidentRematerializationView(
                    "rcx",
                ),
            )
        );
    }
}
