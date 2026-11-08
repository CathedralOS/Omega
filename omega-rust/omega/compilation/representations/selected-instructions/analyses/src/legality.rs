/*
  Staged allocation-legality custody entrances (`stage_optimized_allocation_legality`,
  `StagedOptimizedAllocationLegality`,
  `StagedOptimizedAllocationLegalityCustodyReceipt`).

  Chains `StagedOptimizedLiveRanges` through explicit allocator-availability
  policies (`FullEnvironment`, `FramelessLeaf`, and active-resident immediate
  rematerialization policies) to produce sealed allocation-legality stages.
*/

mod compute {
    use crate::{ValidatedAllocatorAvailability, analyze_allocation_legality};

    use crate::StagedOptimizedLiveRanges;

    use super::custody::custody_receipt;
    use super::{OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality};

    pub(super) fn compute_allocation_legality(
        ranges: StagedOptimizedLiveRanges,
        availability: ValidatedAllocatorAvailability,
    ) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
        let environment = ranges.register_environment();
        let legality = analyze_allocation_legality(
            ranges.ranges(),
            &availability,
            environment.identity(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
        )
        .map_err(OptimizedAllocationLegalityCustodyError::Analysis)?;
        let custody = custody_receipt(
            ranges.custody(),
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
}
mod custody {
    use crate::AllocationLegalityValidationReceipt;
    use register_homes::AllocatorAvailabilityIdentity;

    use crate::StagedOptimizedLiveRangeCustodyReceipt;

    use super::StagedOptimizedAllocationLegalityCustodyReceipt;

    pub(super) fn custody_receipt(
        upstream: StagedOptimizedLiveRangeCustodyReceipt,
        allocator_availability: AllocatorAvailabilityIdentity,
        legality: AllocationLegalityValidationReceipt,
    ) -> StagedOptimizedAllocationLegalityCustodyReceipt {
        StagedOptimizedAllocationLegalityCustodyReceipt {
            core: upstream.core(),
            allocator_availability,
            selected: upstream.selected(),
            liveness: upstream.liveness(),
            ranges: upstream.ranges(),
            legality: legality.identity(),
        }
    }
}
mod policies {
    use crate::{ValidatedAllocatorAvailability, materialize_allocator_availability};
    use isa_aarch64::aarch64_preservation_convention_for_target;
    use isa_x86_64::x86_64_preservation_convention_for_target;
    use register_homes::AllocatorAvailabilityPolicy;

    use crate::StagedOptimizedLiveRanges;

    use super::OptimizedAllocationLegalityCustodyError;

    pub(super) fn all_environment_allocatable_views(
        ranges: &StagedOptimizedLiveRanges,
    ) -> Result<ValidatedAllocatorAvailability, OptimizedAllocationLegalityCustodyError> {
        let environment = ranges.register_environment();
        materialize_allocator_availability(
            environment.identity(),
            environment.target(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
        )
        .map_err(OptimizedAllocationLegalityCustodyError::Availability)
    }

    pub(super) fn frameless_leaf_caller_saved_views(
        ranges: &StagedOptimizedLiveRanges,
    ) -> Result<ValidatedAllocatorAvailability, OptimizedAllocationLegalityCustodyError> {
        let environment = ranges.register_environment();
        let convention = match environment.target().architecture {
            target::Architecture::X86_64 => x86_64_preservation_convention_for_target(
                environment.physical(),
                environment.target(),
            ),
            target::Architecture::Aarch64 => aarch64_preservation_convention_for_target(
                environment.physical(),
                environment.target(),
            ),
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
        materialize_allocator_availability(
            environment.identity(),
            environment.target(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views },
        )
        .map_err(OptimizedAllocationLegalityCustodyError::Availability)
    }

    pub(super) fn active_resident_immediate_u64_multi_use_rematerialization_v1(
        ranges: &StagedOptimizedLiveRanges,
    ) -> Result<ValidatedAllocatorAvailability, OptimizedAllocationLegalityCustodyError> {
        let environment = ranges.register_environment();
        let views = active_resident_immediate_u64_multi_use_rematerialization_v1_views(
            environment.target().architecture,
            environment.physical().model(),
        )?;
        materialize_allocator_availability(
            environment.identity(),
            environment.target(),
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &environment.allocation_constraint_keys(),
            AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views },
        )
        .map_err(OptimizedAllocationLegalityCustodyError::Availability)
    }

    fn active_resident_immediate_u64_multi_use_rematerialization_v1_views(
        architecture: target::Architecture,
        model: &register_model::PhysicalRegisterModel,
    ) -> Result<Vec<register_model::RegisterViewId>, OptimizedAllocationLegalityCustodyError> {
        let names = match architecture {
            target::Architecture::X86_64 => ["rax", "rcx"],
            target::Architecture::Aarch64 => ["x0", "x1"],
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
}

use crate::ValidatedAllocatorAvailability;

use crate::{
    AllocationLegalityError, AllocatorAvailabilityError, StagedOptimizedLiveRanges,
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
use selected_instructions::SelectedProgram;
use selected_instructions::StagedOptimizedCustodyCore;
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use terminal_psi::TerminalPsiIdentity;

pub fn stage_optimized_allocation_legality(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let availability = policies::all_environment_allocatable_views(&ranges)?;
    stage_optimized_allocation_legality_with_availability(ranges, availability)
}

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

pub(crate) fn stage_optimized_allocation_legality_with_availability(
    ranges: StagedOptimizedLiveRanges,
    availability: ValidatedAllocatorAvailability,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    compute::compute_allocation_legality(ranges, availability)
}

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

    staged_producer_accessors!(
        ranges; ranges, liveness, selected, register_environment, selections, budget_per_pass
    );

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
    selected_instructions::staging_custody_core_accessors!();

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
