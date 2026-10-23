//! Optimizer module role: executable entrance. Fixed-driven split requirements.
//!
//! This boundary partitions authenticated source live points and records where
//! an incoming and fixed-use exact-view domain are incompatible. It creates no
//! home, recovery strategy, transformed interval, or physical program.

mod compute;
mod error;
mod validation;

use crate::WorkDefect;
pub(crate) use error::FixedPrecoloredSplitRequirementError;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkUsage};
use register_homes::FixedPrecoloredSplitRequirementPolicy;
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredSplitRequirementPlan, FixedPrecoloredSplitRequirementPlanIdentity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::LiveRangeIdentity;
use semantic_vocabulary::FuelScheduleIdentity;
use target::NativeTarget;

pub(crate) fn analyze_fixed_precolored_split_requirements(
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
    fixed: &crate::ValidatedFixedPrecoloredIntervals,
    policy: FixedPrecoloredSplitRequirementPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedFixedPrecoloredSplitRequirements, FixedPrecoloredSplitRequirementError> {
    let plan = compute::compute(ranges, legality, fixed, policy, budget)?;
    validation::seal_fixed_precolored_split_requirements(legality, plan)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredSplitRequirementValidationReceipt {
    pub(crate) identity: FixedPrecoloredSplitRequirementPlanIdentity,
    pub(crate) fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) target: NativeTarget,
    pub(crate) policy: FixedPrecoloredSplitRequirementPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) register_count: usize,
    pub(crate) fragment_count: usize,
    pub(crate) source_point_count: usize,
    pub(crate) segment_count: usize,
    pub(crate) incompatible_fixed_use_boundary_count: usize,
}

impl FixedPrecoloredSplitRequirementValidationReceipt {
    pub const fn identity(self) -> FixedPrecoloredSplitRequirementPlanIdentity {
        self.identity
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
    pub const fn policy(self) -> FixedPrecoloredSplitRequirementPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn register_count(self) -> usize {
        self.register_count
    }
    pub const fn fragment_count(self) -> usize {
        self.fragment_count
    }
    pub const fn source_point_count(self) -> usize {
        self.source_point_count
    }
    pub const fn segment_count(self) -> usize {
        self.segment_count
    }
    pub const fn incompatible_fixed_use_boundary_count(self) -> usize {
        self.incompatible_fixed_use_boundary_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFixedPrecoloredSplitRequirements {
    pub(crate) plan: FixedPrecoloredSplitRequirementPlan,
    pub(crate) receipt: FixedPrecoloredSplitRequirementValidationReceipt,
}

impl ValidatedFixedPrecoloredSplitRequirements {
    pub const fn plan(&self) -> &FixedPrecoloredSplitRequirementPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> FixedPrecoloredSplitRequirementValidationReceipt {
        self.receipt
    }
}

/// Sort key for a virtual fixed-constraint site — the total order both the
/// compute pass and the replay checker build their cut tables on.
pub(super) type SiteKey = (u8, u32, u32, u32, u16, u8);

pub(super) fn site_key(site: selected_instructions::VirtualFixedConstraintSite) -> SiteKey {
    match site {
        selected_instructions::VirtualFixedConstraintSite::Entry => (0, 0, 0, 0, 0, 0),
        selected_instructions::VirtualFixedConstraintSite::Operand {
            position,
            point,
            instruction,
            operand,
            access,
        } => (
            1,
            position.0,
            point.0,
            instruction.0,
            operand,
            match access {
                register_model::RegisterOperandAccess::Use => 0,
                register_model::RegisterOperandAccess::Def => 1,
                register_model::RegisterOperandAccess::UseDef => 2,
            },
        ),
    }
}

/// The live point a virtual fixed-constraint site names.
pub(super) fn site_point(site: selected_instructions::VirtualFixedConstraintSite) -> u32 {
    match site {
        selected_instructions::VirtualFixedConstraintSite::Entry => 0,
        selected_instructions::VirtualFixedConstraintSite::Operand { point, .. } => point.0,
    }
}

/// Total early-clobber uses across a function's live ranges — the work
/// accounting both passes charge identically. The count stays exact: an
/// overflow is a refusal through the shared work defect, not a wrapped total.
pub(super) fn early_clobber_uses(
    ranges: &selected_instructions::FunctionLiveRanges,
) -> Result<usize, FixedPrecoloredSplitRequirementError> {
    let mut use_count = 0usize;
    for row in &ranges.early_clobbers {
        use_count = use_count
            .checked_add(row.uses.len())
            .ok_or(WorkDefect::Overflow)?;
    }
    Ok(use_count)
}
