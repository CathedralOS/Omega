use omega_isa_aarch64::aarch64_preservation_convention_for_target;
use omega_isa_x86_64::x86_64_preservation_convention_for_target;
use omega_regalloc::{
    AllocatorAvailabilityPolicy, ValidatedAllocatorAvailability, materialize_allocator_availability,
};

use crate::StagedOptimizedLiveRanges;

use super::model::OptimizedAllocationLegalityCustodyError;

pub(super) fn all_environment_allocatable_views(
    ranges: &StagedOptimizedLiveRanges,
) -> Result<ValidatedAllocatorAvailability, OptimizedAllocationLegalityCustodyError> {
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
    Ok(availability)
}

/// Restrict unconstrained allocation to the exact selected convention's
/// caller-saved units. This is the required search policy for the current
/// frameless leaf lane; fixed ABI/operand views remain authoritative.
pub(super) fn frameless_leaf_caller_saved_views(
    ranges: &StagedOptimizedLiveRanges,
) -> Result<ValidatedAllocatorAvailability, OptimizedAllocationLegalityCustodyError> {
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
    Ok(availability)
}

/// Restrict unconstrained allocation to the exact two caller-saved views used
/// by the v1 active-resident immediate-u64 multi-use rematerialization policy.
/// This deliberately creates the closed pressure case the policy is defined
/// to recover; it is not a general allocator cost or availability policy.
pub(super) fn active_resident_immediate_u64_multi_use_rematerialization_v1(
    ranges: &StagedOptimizedLiveRanges,
) -> Result<ValidatedAllocatorAvailability, OptimizedAllocationLegalityCustodyError> {
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
    Ok(availability)
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

#[cfg(test)]
#[path = "policies_tests.rs"]
mod tests;
