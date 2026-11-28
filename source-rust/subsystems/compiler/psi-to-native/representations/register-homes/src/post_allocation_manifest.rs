/*
We define the in-process post-allocation optimization manifest
(`PostAllocationOptimizationManifest`) and its append-only selected-transformation
ledger (`PostAllocationSelectedTransformation`). Later stages share the manifest
and compare it structurally; nothing hashes it.
*/

use optimization_core::{
    OptimizationSelectionIdentity, OptimizationUnitIdentity, OptimizationWorkBudget,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::SelectedInstructionPlan;
use std::sync::Arc;
use target::NativeTarget;

use crate::{AllocatorAvailabilityIdentity, RegisterHomePlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationManifestStage {
    RegisterHomes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationSpillStatus {
    NotRequiredForValidatedHomePlan,
    RealizedInSelectedProgram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PostAllocationStatistics {
    pub functions: u64,               // Ordinary selected functions
    pub assignments: u64,             // Virtual-register home assignments
    pub distinct_physical_views: u64, // Distinct physical views assigned
    pub virtual_interferences: u64,   // Virtual-register interferences
    pub fixed_view_transitions: u64,  // Remaining fixed-view transitions
}

/// One rewrite in the ordered ledger, recorded by kind. Each rewrite's plan
/// holds the programs it read and wrote; keeping those here would hold one
/// program copy per rewrite until the image is written. The manifest's
/// `selected` program and the phase outputs it names are what later checks
/// compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostAllocationSelectedTransformation {
    RuntimeSpill,
    RuntimeRematerialization,
    RuntimeSplit,
    FixedOccurrences,
    FixedViewCopy,
    LiteralFold,
    PressureRematerialization,
    CopyRemoval,
    RedundantExtension,
    AddressFold,
    ConstantBoolean,
    ConstantBranch,
    DeadCompare,
    RedundantCompare,
    EquivalentCompare,
    CopiedCallOperand,
    StoredLoadForwarding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationOptimizationManifest {
    pub stage: PostAllocationManifestStage, // Manifest pipeline stage
    pub optimization_unit: OptimizationUnitIdentity, // Unit the selected program was lowered from
    pub selection: OptimizationSelectionIdentity, // Optimization selections it was lowered under
    pub budget_per_pass: OptimizationWorkBudget, // Per-pass work budget it was lowered under
    pub target: NativeTarget,               // Target architecture and format
    pub selected: Arc<SelectedInstructionPlan>, // Final selected plan
    pub selected_lowering_output: Option<Arc<SelectedInstructionPlan>>, // Program the selected-lowering phase ended with, if it ran
    pub pre_allocation_output: Option<Arc<SelectedInstructionPlan>>, // Program the pre-allocation phase ended with, if it ran
    pub selected_transformations: Vec<PostAllocationSelectedTransformation>, // Ordered physical-form rewrites
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub allocator_availability: AllocatorAvailabilityIdentity,   // Allocator availability digest
    pub homes: Arc<RegisterHomePlan>,                            // Validated register homes
    pub spills: PostAllocationSpillStatus,                       // Spill realization status
    pub frame: PostAllocationUnavailableData, // Frame status (unavailable at this stage)
    pub emission: PostAllocationUnavailableData, // Emission status (unavailable at this stage)
    pub publication: PostAllocationUnavailableData, // Publication status (unavailable at this stage)
    pub statistics: PostAllocationStatistics,       // Summary statistics
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationOptimizationManifestError {
    RootMismatch,
    UnresolvedFixedViewTransitions,
    StatisticsOverflow,
    ContentMismatch,
}

crate::decode_error_display! { PostAllocationOptimizationManifestError, "invalid post-allocation optimization manifest" }

#[cfg(test)]
mod tests {
    use super::{
        PostAllocationManifestStage, PostAllocationOptimizationManifest,
        PostAllocationSelectedTransformation, PostAllocationSpillStatus, PostAllocationStatistics,
        PostAllocationUnavailableData,
    };
    use crate::{AllocatorAvailabilityIdentity, RegisterHomePlan};
    use optimization_core::{
        OptimizationSelectionIdentity, OptimizationUnitIdentity, OptimizationWorkBudget,
    };
    use register_model::TargetRegisterEnvironmentIdentity;
    use selected_instructions::test_support::placeholder_plan;
    use std::sync::Arc;
    use target::NativeTarget;

    fn record() -> PostAllocationOptimizationManifest {
        let selected = placeholder_plan(1);
        PostAllocationOptimizationManifest {
            stage: PostAllocationManifestStage::RegisterHomes,
            optimization_unit: OptimizationUnitIdentity::from_canonical_bytes(b"unit"),
            selection: OptimizationSelectionIdentity::from_bytes([3; 32]),
            budget_per_pass: OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
            target: NativeTarget::linux_x64(),
            selected: Arc::clone(&selected),
            selected_lowering_output: None,
            pre_allocation_output: None,
            selected_transformations: Vec::new(),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
            allocator_availability: AllocatorAvailabilityIdentity::from_bytes([6; 32]),
            homes: Arc::new(RegisterHomePlan {
                selected,
                register_environment: TargetRegisterEnvironmentIdentity::from_bytes([4; 32]),
                allocator_availability: AllocatorAvailabilityIdentity::from_bytes([6; 32]),
                functions: Vec::new(),
            }),
            spills: PostAllocationSpillStatus::NotRequiredForValidatedHomePlan,
            frame: PostAllocationUnavailableData::Unavailable,
            emission: PostAllocationUnavailableData::Unavailable,
            publication: PostAllocationUnavailableData::Unavailable,
            statistics: PostAllocationStatistics {
                functions: 1,
                assignments: 2,
                distinct_physical_views: 2,
                virtual_interferences: 1,
                fixed_view_transitions: 0,
            },
        }
    }

    /// The manifest is compared, not hashed: a change to any recorded
    /// decision, its order or a shared plan makes two manifests differ.
    #[test]
    fn manifest_comparison_sees_every_recorded_decision() {
        let baseline = record();
        assert_eq!(baseline, record());
        let mutations: Vec<fn(&mut PostAllocationOptimizationManifest)> = vec![
            |record| {
                record.optimization_unit = OptimizationUnitIdentity::from_canonical_bytes(b"other")
            },
            |record| record.selection = OptimizationSelectionIdentity::from_bytes([8; 32]),
            |record| record.budget_per_pass = OptimizationWorkBudget::new(2, 1, 1, 1, 1).unwrap(),
            |record| record.target = NativeTarget::linux_arm64(),
            |record| record.selected = placeholder_plan(2),
            |record| record.selected_lowering_output = Some(placeholder_plan(1)),
            |record| record.pre_allocation_output = Some(placeholder_plan(1)),
            |record| {
                record.selected_transformations = vec![
                    PostAllocationSelectedTransformation::RuntimeSpill,
                    PostAllocationSelectedTransformation::LiteralFold,
                ]
            },
            |record| {
                record.register_environment =
                    TargetRegisterEnvironmentIdentity::from_bytes([10; 32])
            },
            |record| {
                record.allocator_availability = AllocatorAvailabilityIdentity::from_bytes([12; 32])
            },
            |record| {
                Arc::make_mut(&mut record.homes).register_environment =
                    TargetRegisterEnvironmentIdentity::from_bytes([11; 32])
            },
            |record| record.spills = PostAllocationSpillStatus::RealizedInSelectedProgram,
            |record| record.statistics.functions += 1,
            |record| record.statistics.assignments += 1,
            |record| record.statistics.distinct_physical_views += 1,
            |record| record.statistics.virtual_interferences += 1,
            |record| record.statistics.fixed_view_transitions += 1,
        ];
        for mutate in mutations {
            let mut changed = record();
            mutate(&mut changed);
            assert_ne!(baseline, changed);
        }
        let mut ordered = record();
        ordered.selected_transformations = vec![
            PostAllocationSelectedTransformation::RuntimeSpill,
            PostAllocationSelectedTransformation::LiteralFold,
        ];
        let mut reordered = ordered.clone();
        reordered.selected_transformations.reverse();
        assert_ne!(ordered, reordered);
    }
}
