/*
We define allocation candidate policies and exact precoloring/legality
constraints on physical register homes (`AllocatorAvailabilityPlan`,
`AllocationLegalityPlan`, `FixedPrecoloredIntervalPlan`, and
`FixedPrecoloredSplitRequirementPlan`).

These records capture analysis facts and candidate domains prior to home
assignment. Incompatible fixed views remain explicit transition or split
requirements rather than being silently reconciled, and allocator availability
policies restrict only unconstrained candidates.
*/

use std::sync::Arc;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{
    LiveRangeEdgeConnector, LiveRangePoint, LivenessPosition, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionPlan, VirtualFixedConstraintSite, VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::NativeTarget;

use crate::digest_encoder::content_identity;

crate::identity_newtype! { pub struct AllocatorAvailabilityIdentity(pub(crate) [u8; 32]); }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocatorAvailabilityPolicy {
    AllEnvironmentAllocatableViewsV1, // Every environment-allocatable view is available
    ExplicitUnconstrainedViewAllowlistV1 {
        views: Vec<RegisterViewId>, // Allowed unconstrained physical register views
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatorAvailabilityPlan {
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub physical: PhysicalRegisterModelIdentity,                 // Physical register model digest
    pub policy: AllocatorAvailabilityPolicy,                     // Availability selection policy
    pub classes: Vec<RegisterClassAvailability>, // Class-ID-sorted availability rows
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterClassAvailability {
    pub class: RegisterClassId, // Physical register class identity
    pub unconstrained_views: Vec<RegisterViewId>, // Sorted allocatable views for this class
}

pub fn allocator_availability_identity(
    plan: &AllocatorAvailabilityPlan,
) -> AllocatorAvailabilityIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-allocator-availability.v1\0");
    bytes.extend_from_slice(&encode_terminal_allocator_availability_content(plan));
    AllocatorAvailabilityIdentity::from_bytes(content_identity(&bytes))
}

pub(crate) fn encode_terminal_allocator_availability_content(
    plan: &AllocatorAvailabilityPlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.physical.bytes());
    match &plan.policy {
        AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1 => bytes.push(0),
        AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views } => {
            bytes.push(1);
            crate::codec_bytes::length(&mut bytes, views.len(), "allocator-availability identity");
            for view in views {
                crate::codec_bytes::push_encoded(&mut bytes, view.0.to_le_bytes());
            }
        }
    }
    crate::codec_bytes::length(
        &mut bytes,
        plan.classes.len(),
        "allocator-availability identity",
    );
    for row in &plan.classes {
        crate::codec_bytes::push_encoded(&mut bytes, row.class.0.to_le_bytes());
        crate::codec_bytes::length(
            &mut bytes,
            row.unconstrained_views.len(),
            "allocator-availability identity",
        );
        for view in &row.unconstrained_views {
            crate::codec_bytes::push_encoded(&mut bytes, view.0.to_le_bytes());
        }
    }
    bytes
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationLegalityPlan {
    pub selected: Arc<SelectedInstructionPlan>, // Program whose live ranges this legality covers
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub allocator_availability: AllocatorAvailabilityIdentity, // Allocator availability digest
    pub functions: Vec<Arc<FunctionAllocationLegality>>, // Per-function legality rows
}

impl AllocationLegalityPlan {
    pub fn virtual_registers(
        &self,
    ) -> impl Iterator<Item = &VirtualRegisterAllocationLegality> + '_ {
        self.functions
            .iter()
            .flat_map(|function| &function.virtual_registers)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAllocationLegality {
    pub machine: MachineId, // Owning machine identity
    pub virtual_registers: Vec<VirtualRegisterAllocationLegality>, // Virtual-register legality rows
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualRegisterAllocationLegality {
    pub virtual_register: VirtualRegisterId, // Virtual register identity
    pub class: RegisterClassId,              // Physical register class
    pub spans: Vec<VirtualLegalitySpan>,     // Legal candidate spans across source fragments
    pub early_clobber_points: Vec<VirtualEarlyClobberPointLegality>, // Early-clobber legality points
    pub entry_transitions: Vec<EntryFixedViewTransition>, // Incompatible fixed-view transitions
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualLegalitySpan {
    pub block: SelectedBlockId,          // Containing block identity
    pub start: LiveRangePoint,           // Span start point (inclusive)
    pub end: LiveRangePoint,             // Span end point (exclusive)
    pub candidates: Vec<RegisterViewId>, // View-ID-sorted legal candidate views
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualEarlyClobberPointLegality {
    pub block: SelectedBlockId,             // Containing block identity
    pub position: LivenessPosition,         // Liveness instruction position
    pub instruction: SelectedInstructionId, // Instruction identity
    pub operand: u16,                       // Operand ordinal
    pub point: LiveRangePoint,              // Early-clobber live-range point
    pub candidates: Vec<RegisterViewId>,    // View-ID-sorted legal early-phase views
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryFixedViewTransition {
    pub from_view: RegisterViewId,           // Entry fixed view
    pub to_site: VirtualFixedConstraintSite, // Conflicting fixed constraint site
    pub to_view: RegisterViewId,             // Conflicting fixed view
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredIntervalPolicy {
    FixedConstraintPointIntervalsV1, // Fixed constraints occupy [point, point + 1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredIntervalPlan {
    pub selected: Arc<SelectedInstructionPlan>, // Program the ranges and legality describe
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub allocator_availability: AllocatorAvailabilityIdentity, // Allocator availability digest
    pub optimization_unit: OptimizationUnitIdentity, // Optimization unit digest
    pub work_schedule: WorkScheduleIdentity,    // Work schedule identity
    pub policy: FixedPrecoloredIntervalPolicy,  // Interval derivation policy
    pub budget: OptimizationWorkBudget,         // Optimization work budget
    pub usage: OptimizationWorkUsage,           // Optimization work usage
    pub functions: Vec<FunctionFixedPrecoloredIntervals>, // Per-function intervals
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFixedPrecoloredIntervals {
    pub machine: MachineId,                      // Owning machine identity
    pub intervals: Vec<FixedPrecoloredInterval>, // Fixed precolored point intervals
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedPrecoloredInterval {
    pub virtual_register: VirtualRegisterId, // Virtual register identity
    pub class: RegisterClassId,              // Physical register class
    pub site: VirtualFixedConstraintSite,    // Fixed constraint site
    pub block: SelectedBlockId,              // Containing block identity
    pub start: LiveRangePoint,               // Interval start point (inclusive)
    pub end: LiveRangePoint,                 // Interval end point (exclusive)
    pub view: RegisterViewId,                // Required physical register view
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredSplitRequirementPolicy {
    FixedUseBoundaryRequirementsV1, // Partition acyclic fragment trees when a fixed use empties the domain
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredSplitRequirementPlan {
    pub selected: Arc<SelectedInstructionPlan>, // Program the ranges and legality describe
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub allocator_availability: AllocatorAvailabilityIdentity, // Allocator availability digest
    pub optimization_unit: OptimizationUnitIdentity, // Optimization unit digest
    pub work_schedule: WorkScheduleIdentity,    // Work schedule identity
    pub target: NativeTarget,                   // Target architecture and format
    pub policy: FixedPrecoloredSplitRequirementPolicy, // Split requirement policy
    pub budget: OptimizationWorkBudget,         // Optimization work budget
    pub usage: OptimizationWorkUsage,           // Optimization work usage
    pub functions: Vec<FunctionFixedPrecoloredSplitRequirements>, // Per-function split requirements
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFixedPrecoloredSplitRequirements {
    pub machine: MachineId, // Owning machine identity
    pub registers: Vec<FixedPrecoloredRegisterSplitRequirements>, // Per-register split partitions
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredRegisterSplitRequirements {
    pub virtual_register: VirtualRegisterId, // Virtual register identity
    pub class: RegisterClassId,              // Physical register class
    pub fragments: Vec<FixedPrecoloredSourceFragmentRequirements>, // Partitioned source fragments
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredSourceFragmentRequirements {
    pub block: SelectedBlockId,       // Containing block identity
    pub source_start: LiveRangePoint, // Original fragment start point
    pub source_end: LiveRangePoint,   // Original fragment end point
    pub segments: Vec<FixedPrecoloredSourceSegment>, // Contiguous compatible segments
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedPrecoloredSourceSegmentId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredSourceSegment {
    pub id: FixedPrecoloredSourceSegmentId, // Dense segment identity
    pub start: LiveRangePoint,              // Segment start point (inclusive)
    pub end: LiveRangePoint,                // Segment end point (exclusive)
    pub candidates: Vec<RegisterViewId>,    // Sorted compatible physical views
    pub opening: FixedPrecoloredSourceSegmentOpening, // Segment boundary opening cause
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedPrecoloredSourceSegmentOpening {
    SourceRangeStartV1,
    IncomingSourceEdgeV1 {
        connector: LiveRangeEdgeConnector, // Incoming CFG edge connector
    },
    IncompatibleFixedUseDomainBoundaryV1 {
        incoming: Option<LiveRangeEdgeConnector>, // Optional incoming CFG edge connector
        site: VirtualFixedConstraintSite,         // Fixed use constraint site
        destination_view: RegisterViewId,         // Required destination physical view
    },
}

pub mod allocator_availability {
    pub use super::{
        AllocatorAvailabilityIdentity, AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy,
        RegisterClassAvailability, allocator_availability_identity,
    };
}

pub mod allocation_legality {
    pub use super::{
        AllocationLegalityPlan, EntryFixedViewTransition, FunctionAllocationLegality,
        VirtualEarlyClobberPointLegality, VirtualLegalitySpan, VirtualRegisterAllocationLegality,
    };
}

pub mod fixed_precolored_intervals {
    pub use super::{
        FixedPrecoloredInterval, FixedPrecoloredIntervalPlan, FixedPrecoloredIntervalPolicy,
        FunctionFixedPrecoloredIntervals,
    };
}

pub mod fixed_precolored_split_requirements {
    pub use super::{
        FixedPrecoloredRegisterSplitRequirements, FixedPrecoloredSourceFragmentRequirements,
        FixedPrecoloredSourceSegment, FixedPrecoloredSourceSegmentId,
        FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPlan,
        FixedPrecoloredSplitRequirementPolicy, FunctionFixedPrecoloredSplitRequirements,
    };
}

#[cfg(test)]
mod tests {
    use super::{
        AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy, RegisterClassAvailability,
        allocator_availability_identity,
    };
    use register_model::{
        PhysicalRegisterModelIdentity, RegisterClassId, RegisterViewId,
        TargetRegisterEnvironmentIdentity,
    };

    fn plan() -> AllocatorAvailabilityPlan {
        AllocatorAvailabilityPlan {
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([1; 32]),
            physical: PhysicalRegisterModelIdentity::from_bytes([2; 32]),
            policy: AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                views: vec![RegisterViewId(1)],
            },
            classes: vec![RegisterClassAvailability {
                class: RegisterClassId(0),
                unconstrained_views: vec![RegisterViewId(1)],
            }],
        }
    }

    #[test]
    fn identity_binds_every_availability_domain() {
        let baseline = allocator_availability_identity(&plan());
        let mut changes = Vec::new();

        let mut changed = plan();
        changed.register_environment = TargetRegisterEnvironmentIdentity::from_bytes([3; 32]);
        changes.push(changed);
        let mut changed = plan();
        changed.physical = PhysicalRegisterModelIdentity::from_bytes([4; 32]);
        changes.push(changed);
        let mut changed = plan();
        changed.policy = AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1;
        changes.push(changed);
        let mut changed = plan();
        changed.classes[0].class = RegisterClassId(1);
        changes.push(changed);
        let mut changed = plan();
        changed.classes[0].unconstrained_views[0] = RegisterViewId(2);
        changes.push(changed);

        assert_eq!(baseline, allocator_availability_identity(&plan()));
        for changed in changes {
            assert_ne!(baseline, allocator_availability_identity(&changed));
        }
    }
}
