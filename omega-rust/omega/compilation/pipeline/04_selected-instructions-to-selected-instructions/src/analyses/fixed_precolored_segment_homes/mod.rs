//! Optimizer module role: executable entrance. Fixed/precolored segment-home assignment.
//!
//! This boundary assigns one deterministic physical view to each authenticated
//! source-segment domain. It creates no copy, VReg, instruction, spill, or
//! transformed liveness, and distinct assigned views do not imply movement.

mod compute;
mod domains;
mod error;
use domains::Domain;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    CopyAffinity, FunctionLiveRanges, LiveRangePoint, SelectedBlockId, VirtualLiveRange,
    VirtualRegisterId,
};
use std::collections::BTreeMap;

use crate::work_accounting::Work;
mod validation;

pub(crate) use error::FixedPrecoloredSegmentHomeError;
use register_homes::FixedPrecoloredSegmentHomePolicy;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkUsage};
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredSegmentHomePlan, FixedPrecoloredSegmentHomePlanIdentity,
    FixedPrecoloredSplitRequirementPlanIdentity,
};
use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use selected_instructions::LiveRangeIdentity;
use semantic_vocabulary::FuelScheduleIdentity;
use target::NativeTarget;

#[allow(clippy::too_many_arguments)]
pub(crate) fn assign_fixed_precolored_segment_homes(
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
    fixed: &crate::ValidatedFixedPrecoloredIntervals,
    requirements: &crate::ValidatedFixedPrecoloredSplitRequirements,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: FixedPrecoloredSegmentHomePolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedFixedPrecoloredSegmentHomes, FixedPrecoloredSegmentHomeError> {
    let plan = compute::compute(
        ranges,
        legality,
        fixed,
        requirements,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validation::seal_fixed_precolored_segment_homes(plan)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredSegmentHomeValidationReceipt {
    pub(crate) identity: FixedPrecoloredSegmentHomePlanIdentity,
    pub(crate) split_requirements: FixedPrecoloredSplitRequirementPlanIdentity,
    pub(crate) fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) target: NativeTarget,
    pub(crate) policy: FixedPrecoloredSegmentHomePolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) domain_count: usize,
    pub(crate) assignment_count: usize,
}

impl FixedPrecoloredSegmentHomeValidationReceipt {
    pub const fn identity(self) -> FixedPrecoloredSegmentHomePlanIdentity {
        self.identity
    }
    pub const fn split_requirements(self) -> FixedPrecoloredSplitRequirementPlanIdentity {
        self.split_requirements
    }
    pub const fn fixed_intervals(self) -> FixedPrecoloredIntervalPlanIdentity {
        self.fixed_intervals
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn target(self) -> NativeTarget {
        self.target
    }
    pub const fn policy(self) -> FixedPrecoloredSegmentHomePolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn domain_count(self) -> usize {
        self.domain_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFixedPrecoloredSegmentHomes {
    pub(crate) plan: FixedPrecoloredSegmentHomePlan,
    pub(crate) receipt: FixedPrecoloredSegmentHomeValidationReceipt,
}

impl ValidatedFixedPrecoloredSegmentHomes {
    pub const fn plan(&self) -> &FixedPrecoloredSegmentHomePlan {
        &self.plan
    }
    pub const fn receipt(&self) -> FixedPrecoloredSegmentHomeValidationReceipt {
        self.receipt
    }
}

fn affinity_edges(
    domains: &[Domain],
    ranges: &FunctionLiveRanges,
    work: &mut Work,
) -> Result<BTreeMap<usize, Vec<(usize, usize)>>, FixedPrecoloredSegmentHomeError> {
    let mut members = BTreeMap::<VirtualRegisterId, Vec<usize>>::new();
    for (index, domain) in domains.iter().enumerate() {
        members
            .entry(domain.virtual_register)
            .or_default()
            .push(index);
    }
    let mut by_register = BTreeMap::<VirtualRegisterId, &VirtualLiveRange>::new();
    for range in &ranges.virtual_registers {
        by_register.insert(range.virtual_register, range);
    }
    let mut edges = Vec::new();
    for affinity in &ranges.copy_affinities {
        work.validation_steps(1)?;
        work.iterations(1)?;
        let (Some(source), Some(destination)) = (
            by_register.get(&affinity.source).copied(),
            by_register.get(&affinity.destination).copied(),
        ) else {
            continue;
        };
        let (Some(source_point), Some(destination_point)) = (
            occurrence_point(source, affinity, RegisterOperandAccess::Use),
            occurrence_point(destination, affinity, RegisterOperandAccess::Def),
        ) else {
            continue;
        };
        let (Some(source_domain), Some(destination_domain)) = (
            members.get(&affinity.source).and_then(|member| {
                containing_domain(member, domains, affinity.block, source_point)
            }),
            members.get(&affinity.destination).and_then(|member| {
                containing_domain(member, domains, affinity.block, destination_point)
            }),
        ) else {
            continue;
        };
        if source_domain != destination_domain {
            edges.push((
                source_domain.min(destination_domain),
                source_domain.max(destination_domain),
            ));
        }
    }
    edges.sort_unstable();
    // Every ranking scan only needs the edges touching the domains it asks
    // about, so index them by member once rather than rescanning the full
    // edge list for each question.
    let mut incident = BTreeMap::<usize, Vec<(usize, usize)>>::new();
    for &edge in &edges {
        incident.entry(edge.0).or_default().push(edge);
        incident.entry(edge.1).or_default().push(edge);
    }
    Ok(incident)
}

fn occurrence_point(
    range: &VirtualLiveRange,
    affinity: &CopyAffinity,
    access: RegisterOperandAccess,
) -> Option<LiveRangePoint> {
    range
        .occurrences
        .iter()
        .find(|occurrence| {
            occurrence.instruction == affinity.instruction && occurrence.access == access
        })
        .map(|occurrence| occurrence.point)
}

fn containing_domain(
    members: &[usize],
    domains: &[Domain],
    block: SelectedBlockId,
    point: LiveRangePoint,
) -> Option<usize> {
    members.iter().copied().find(|&index| {
        domains[index]
            .segments
            .iter()
            .any(|segment| segment.block == block && segment.start <= point && point < segment.end)
    })
}
