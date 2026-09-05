use std::collections::BTreeSet;

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};

use crate::{
    FixedPrecoloredIntervalPolicy, FixedPrecoloredSegmentHomeError, FixedPrecoloredSegmentHomePlan,
    FixedPrecoloredSegmentHomeValidationReceipt, FixedPrecoloredSplitRequirementPolicy,
    ValidatedAllocationLegality, ValidatedFixedPrecoloredIntervals,
    ValidatedFixedPrecoloredSegmentHomes, ValidatedFixedPrecoloredSplitRequirements,
    ValidatedLiveRanges, fixed_precolored_segment_home_plan_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_fixed_precolored_segment_homes(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    candidate: FixedPrecoloredSegmentHomePlan,
) -> Result<ValidatedFixedPrecoloredSegmentHomes, FixedPrecoloredSegmentHomeError> {
    validate_roots(
        ranges,
        legality,
        fixed,
        requirements,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        &candidate,
    )?;
    let expected = super::replay::replay(
        ranges,
        requirements,
        physical,
        candidate.policy,
        candidate.budget,
    )?;
    if candidate.usage != expected.usage {
        return Err(FixedPrecoloredSegmentHomeError::UsageMismatch);
    }
    if candidate.functions != expected.functions
        || candidate.structural_unit_functions != expected.structural_unit_functions
    {
        return Err(FixedPrecoloredSegmentHomeError::NonCanonicalFunctions);
    }
    let domain_count = candidate
        .functions
        .iter()
        .chain(&candidate.structural_unit_functions)
        .map(|function| {
            function
                .assignments
                .iter()
                .map(|assignment| assignment.allocation_domain)
                .collect::<BTreeSet<_>>()
                .len()
        })
        .sum();
    let assignment_count = candidate
        .functions
        .iter()
        .chain(&candidate.structural_unit_functions)
        .map(|function| function.assignments.len())
        .sum();
    let receipt = FixedPrecoloredSegmentHomeValidationReceipt {
        identity: fixed_precolored_segment_home_plan_identity(&candidate),
        split_requirements: candidate.split_requirements,
        fixed_intervals: candidate.fixed_intervals,
        ranges: candidate.ranges,
        legality: candidate.legality,
        register_environment: candidate.register_environment,
        allocator_availability: candidate.allocator_availability,
        optimization_unit: candidate.optimization_unit,
        fuel_schedule: candidate.fuel_schedule,
        target: candidate.target,
        policy: candidate.policy,
        usage: candidate.usage,
        function_count: candidate.functions.len(),
        structural_unit_function_count: candidate.structural_unit_functions.len(),
        domain_count,
        assignment_count,
    };
    Ok(ValidatedFixedPrecoloredSegmentHomes {
        plan: candidate,
        receipt,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_roots(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    candidate: &FixedPrecoloredSegmentHomePlan,
) -> Result<(), FixedPrecoloredSegmentHomeError> {
    if candidate.split_requirements != requirements.receipt().identity()
        || candidate.fixed_intervals != fixed.receipt().identity()
        || candidate.ranges != ranges.receipt().identity()
        || candidate.legality != legality.receipt().identity()
        || candidate.register_environment != register_environment
        || candidate.allocator_availability != legality.receipt().allocator_availability()
        || candidate.optimization_unit != ranges.receipt().optimization_unit()
        || candidate.fuel_schedule != ranges.receipt().fuel_schedule()
        || candidate.target != ranges.plan().target
        || requirements.receipt().fixed_intervals() != fixed.receipt().identity()
        || requirements.receipt().ranges() != ranges.receipt().identity()
        || requirements.receipt().legality() != legality.receipt().identity()
        || requirements.receipt().register_environment() != register_environment
        || requirements.receipt().allocator_availability()
            != legality.receipt().allocator_availability()
        || requirements.receipt().optimization_unit() != ranges.receipt().optimization_unit()
        || requirements.receipt().fuel_schedule() != ranges.receipt().fuel_schedule()
        || requirements.receipt().target() != ranges.plan().target
        || requirements.receipt().policy()
            != FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1
        || fixed.receipt().policy()
            != FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1
        || legality.receipt().ranges() != ranges.receipt().identity()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
        || target_register_environment_identity(
            ranges.plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        ) != register_environment
    {
        return Err(FixedPrecoloredSegmentHomeError::RootMismatch);
    }
    Ok(())
}
