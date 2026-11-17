/*
  Staged allocation-legality custody entrances (`stage_optimized_allocation_legality`,
  `admit_optimized_allocation_legality`, `StagedOptimizedAllocationLegality`,
  `StagedOptimizedAllocationLegalityCustodyReceipt`).

  `stage_optimized_allocation_legality` chains `StagedOptimizedLiveRanges`
  through one explicit `AllocatorAvailabilityRequirement` and computes the
  legality analysis. `admit_optimized_allocation_legality` stages a legality a
  rewrite already validated for the same program instead of computing it
  again; it accepts the analysis only when its receipt names those exact live
  ranges, register environment and allocator availability.
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

/// Which physical register views a legality analysis lets allocation use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocatorAvailabilityRequirement {
    /// Every view the register environment marks allocatable.
    AllEnvironmentAllocatableViews,
    /// Only views whose units the target convention leaves caller-saved, so a
    /// leaf needs no callee-saved preservation.
    FramelessLeafCallerSavedViews,
    /// The two fixed views the active-resident rematerialization rule reasons about.
    ActiveResidentImmediateU64MultiUseRematerializationV1,
}

fn materialize_availability(
    ranges: &StagedOptimizedLiveRanges,
    requirement: AllocatorAvailabilityRequirement,
) -> Result<ValidatedAllocatorAvailability, OptimizedAllocationLegalityCustodyError> {
    match requirement {
        AllocatorAvailabilityRequirement::AllEnvironmentAllocatableViews => {
            policies::all_environment_allocatable_views(ranges)
        }
        AllocatorAvailabilityRequirement::FramelessLeafCallerSavedViews => {
            policies::frameless_leaf_caller_saved_views(ranges)
        }
        AllocatorAvailabilityRequirement::ActiveResidentImmediateU64MultiUseRematerializationV1 => {
            policies::active_resident_immediate_u64_multi_use_rematerialization_v1(ranges)
        }
    }
}

pub fn stage_optimized_allocation_legality(
    ranges: StagedOptimizedLiveRanges,
    requirement: AllocatorAvailabilityRequirement,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let availability = materialize_availability(&ranges, requirement)?;
    stage_optimized_allocation_legality_with_availability(ranges, availability)
}

/// Stage a legality analysis a rewrite already validated for `ranges`' program.
/// Nothing is recomputed; the analysis must name these exact live ranges, this
/// register environment and this allocator availability.
pub fn admit_optimized_allocation_legality(
    ranges: StagedOptimizedLiveRanges,
    availability: ValidatedAllocatorAvailability,
    legality: ValidatedAllocationLegality,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let receipt = legality.receipt();
    if receipt.ranges() != ranges.ranges().receipt().identity()
        || receipt.register_environment() != ranges.register_environment().identity()
        || receipt.allocator_availability() != availability.receipt().identity()
        || availability.receipt().register_environment() != ranges.register_environment().identity()
    {
        return Err(OptimizedAllocationLegalityCustodyError::AnalysisMismatch);
    }
    let custody =
        custody::custody_receipt(ranges.custody(), availability.receipt().identity(), receipt);
    Ok(StagedOptimizedAllocationLegality {
        ranges,
        availability,
        legality,
        custody,
    })
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
    staged_producer_accessors!(
        ranges; ranges, liveness, selected, record, register_environment, selections, budget_per_pass
    );

    /// Keep this analysis for a rule that requires `requirement`, refusing
    /// when the analysis was staged under different allocator availability.
    pub(crate) fn require_availability(
        self,
        requirement: AllocatorAvailabilityRequirement,
    ) -> Result<Self, OptimizedAllocationLegalityCustodyError> {
        let required = materialize_availability(&self.ranges, requirement)?;
        if required.receipt().identity() != self.availability.receipt().identity() {
            return Err(OptimizedAllocationLegalityCustodyError::AvailabilityConflict);
        }
        Ok(self)
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
    /// An admitted analysis names other live ranges, registers or availability.
    AnalysisMismatch,
    /// A rule requires other allocator availability than the analysis it was handed.
    AvailabilityConflict,
}

failed_display!(
    OptimizedAllocationLegalityCustodyError,
    "optimized allocation-legality staging failed"
);
