use omega_optimization_core::{
    PostAllocationOptimizationManifestIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
use omega_register_model::TargetRegisterEnvironmentIdentity;
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use omega_target::NativeTarget;

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedViewCopyIdentity,
    LiteralFoldIdentity, LiveRangeIdentity, LivenessIdentity, PressureRematerializationIdentity,
    RegisterHomeIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationManifestStage {
    ValidatedRegisterHomes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationSpillStatus {
    NotRequiredForValidatedHomePlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PostAllocationStatistics {
    /// Ordinary selected functions. Structural-signature Unit functions are
    /// counted separately and never folded into this established statistic.
    pub functions: u64,
    pub structural_unit_functions: u64,
    pub assignments: u64,
    pub distinct_physical_views: u64,
    pub virtual_interferences: u64,
    pub fixed_view_transitions: u64,
}

/// Ordered physical-form rewrites applied to the selected CFG before the
/// rooted liveness/range/legality/home chain. Order is semantic custody; this
/// is not an unordered feature or optimization-level set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostAllocationSelectedTransformation {
    FixedViewCopy(FixedViewCopyIdentity),
    LiteralFold(LiteralFoldIdentity),
    PressureRematerialization(PressureRematerializationIdentity),
}

/// Structured report at the first independently validated physical-home
/// boundary. This record is not machine-emission or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationOptimizationManifest {
    pub identity: PostAllocationOptimizationManifestIdentity,
    pub stage: PostAllocationManifestStage,
    pub pre_physical: PrePhysicalOptimizationManifestIdentity,
    pub target: NativeTarget,
    pub selected: SelectedInstructionPlanIdentity,
    pub selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pub selected_transformations: Vec<PostAllocationSelectedTransformation>,
    pub liveness: LivenessIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub homes: RegisterHomeIdentity,
    pub spills: PostAllocationSpillStatus,
    pub frame: PostAllocationUnavailableData,
    pub emission: PostAllocationUnavailableData,
    pub publication: PostAllocationUnavailableData,
    pub statistics: PostAllocationStatistics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPostAllocationOptimizationManifest {
    pub(super) record: PostAllocationOptimizationManifest,
}

impl ValidatedPostAllocationOptimizationManifest {
    pub const fn record(&self) -> &PostAllocationOptimizationManifest {
        &self.record
    }
}
