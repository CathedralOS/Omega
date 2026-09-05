//! Fixed-view allocation and copy evidence, separate from admitted plans.
//!
//! These records do not grant validation or publication authority. The owning
//! transform independently reconstructs and compares them before admission.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedReanalysisCustodyReceipt {
    pub source: FixedViewCopyCustodyReceipt,
    pub transformed_liveness: crate::LivenessIdentity,
    pub transformed_ranges: crate::LiveRangeIdentity,
    pub transformed_legality: crate::AllocationLegalityIdentity,
    pub allocator_availability: crate::AllocatorAvailabilityIdentity,
    pub function_count: usize,
    pub virtual_register_count: usize,
    pub entry_transition_count: usize,
}

impl SelectedReanalysisCustodyReceipt {
    pub const fn source(self) -> FixedViewCopyCustodyReceipt {
        self.source
    }
    pub const fn transformed_liveness(self) -> crate::LivenessIdentity {
        self.transformed_liveness
    }
    pub const fn transformed_ranges(self) -> crate::LiveRangeIdentity {
        self.transformed_ranges
    }
    pub const fn transformed_legality(self) -> crate::AllocationLegalityIdentity {
        self.transformed_legality
    }
    pub const fn allocator_availability(self) -> crate::AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }
    pub const fn entry_transition_count(self) -> usize {
        self.entry_transition_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredSegmentHomeCustodyReceipt {
    pub upstream: AllocationLegalityCustodyReceipt,
    pub fixed: FixedPrecoloredIntervalValidationReceipt,
    pub requirements: FixedPrecoloredSplitRequirementValidationReceipt,
    pub homes: FixedPrecoloredSegmentHomeValidationReceipt,
}

impl FixedPrecoloredSegmentHomeCustodyReceipt {
    pub const fn upstream(self) -> AllocationLegalityCustodyReceipt {
        self.upstream
    }
    pub const fn fixed(self) -> FixedPrecoloredIntervalValidationReceipt {
        self.fixed
    }
    pub const fn requirements(self) -> FixedPrecoloredSplitRequirementValidationReceipt {
        self.requirements
    }
    pub const fn homes(self) -> FixedPrecoloredSegmentHomeValidationReceipt {
        self.homes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedViewCopyCustodyReceipt {
    pub psi: TerminalPsiIdentity,
    pub target: target::NativeTarget,
    pub entry: MachineId,
    pub optimization: OptimizationIdentityBundleIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    pub manifest: PrePhysicalOptimizationManifestIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub allocator_availability: crate::AllocatorAvailabilityIdentity,
    pub source_selected: SelectedInstructionPlanIdentity,
    pub source_liveness: crate::LivenessIdentity,
    pub source_ranges: crate::LiveRangeIdentity,
    pub source_legality: crate::AllocationLegalityIdentity,
    pub fixed_intervals: crate::FixedPrecoloredIntervalPlanIdentity,
    pub split_requirements: crate::FixedPrecoloredSplitRequirementPlanIdentity,
    pub segment_homes: crate::FixedPrecoloredSegmentHomePlanIdentity,
    pub transformation: FixedViewCopyIdentity,
    pub transformed_selected: SelectedInstructionPlanIdentity,
    pub policy: FixedViewCopyPolicy,
    pub usage: OptimizationWorkUsage,
    pub function_count: usize,
    pub copy_count: usize,
}

impl FixedViewCopyCustodyReceipt {
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
    pub const fn allocator_availability(self) -> crate::AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_liveness(self) -> crate::LivenessIdentity {
        self.source_liveness
    }
    pub const fn source_ranges(self) -> crate::LiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> crate::AllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn fixed_intervals(self) -> crate::FixedPrecoloredIntervalPlanIdentity {
        self.fixed_intervals
    }
    pub const fn split_requirements(self) -> crate::FixedPrecoloredSplitRequirementPlanIdentity {
        self.split_requirements
    }
    pub const fn segment_homes(self) -> crate::FixedPrecoloredSegmentHomePlanIdentity {
        self.segment_homes
    }
    pub const fn transformation(self) -> FixedViewCopyIdentity {
        self.transformation
    }
    pub const fn transformed_selected(self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn policy(self) -> FixedViewCopyPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn copy_count(self) -> usize {
        self.copy_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredIntervalValidationReceipt {
    pub identity: FixedPrecoloredIntervalPlanIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: FixedPrecoloredIntervalPolicy,
    pub usage: OptimizationWorkUsage,
    pub function_count: usize,
    pub structural_unit_function_count: usize,
    pub inspected_register_count: usize,
    pub interval_count: usize,
    pub entry_interval_count: usize,
    pub operand_interval_count: usize,
}

impl FixedPrecoloredIntervalValidationReceipt {
    pub const fn identity(self) -> FixedPrecoloredIntervalPlanIdentity {
        self.identity
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
    pub const fn policy(self) -> FixedPrecoloredIntervalPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
    }
    pub const fn inspected_register_count(self) -> usize {
        self.inspected_register_count
    }
    pub const fn interval_count(self) -> usize {
        self.interval_count
    }
    pub const fn entry_interval_count(self) -> usize {
        self.entry_interval_count
    }
    pub const fn operand_interval_count(self) -> usize {
        self.operand_interval_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredSplitRequirementValidationReceipt {
    pub identity: FixedPrecoloredSplitRequirementPlanIdentity,
    pub fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub policy: FixedPrecoloredSplitRequirementPolicy,
    pub usage: OptimizationWorkUsage,
    pub function_count: usize,
    pub structural_unit_function_count: usize,
    pub register_count: usize,
    pub fragment_count: usize,
    pub source_point_count: usize,
    pub segment_count: usize,
    pub incompatible_fixed_use_boundary_count: usize,
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
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredSegmentHomeValidationReceipt {
    pub identity: FixedPrecoloredSegmentHomePlanIdentity,
    pub split_requirements: FixedPrecoloredSplitRequirementPlanIdentity,
    pub fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub policy: FixedPrecoloredSegmentHomePolicy,
    pub usage: OptimizationWorkUsage,
    pub function_count: usize,
    pub structural_unit_function_count: usize,
    pub domain_count: usize,
    pub assignment_count: usize,
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
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
    }
    pub const fn domain_count(self) -> usize {
        self.domain_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
}

/// Named source evidence used to discover the exact fixed-view boundaries
/// consumed by this transformation. Legacy wire generations remain decodable,
/// but current production and validation require segment-home evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedViewCopySourceEvidence {
    LegacyLegalityTransitionsV1,
    FixedPrecoloredSegmentHomesV1 {
        fixed_intervals: FixedPrecoloredIntervalPlanIdentity,
        split_requirements: FixedPrecoloredSplitRequirementPlanIdentity,
        segment_homes: FixedPrecoloredSegmentHomePlanIdentity,
    },
}
