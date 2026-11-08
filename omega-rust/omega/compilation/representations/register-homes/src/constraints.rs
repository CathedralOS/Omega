/*
We define allocation candidate policies and exact precoloring/legality
constraints on physical register homes (`AllocatorAvailabilityPlan`,
`AllocationLegalityPlan`, `FixedPrecoloredIntervalPlan`, and
`FixedPrecoloredSplitRequirementPlan`).

These records capture analysis facts and candidate domains prior to home
assignment. Incompatible fixed views remain explicit transition or split
requirements rather than being silently reconciled, and allocator availability
policies restrict only unconstrained candidates (`OMGAVA\0\0` version 1).
*/

use std::sync::Arc;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::{
    LiveRangeEdgeConnector, LiveRangeIdentity, LiveRangePoint, LivenessPosition, SelectedBlockId,
    SelectedInstructionId, VirtualFixedConstraintSite, VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::NativeTarget;

use crate::codec_bytes::Envelope;
use crate::digest_encoder::content_identity;
use crate::encode_fixed_constraint_site_identity;

const ALLOCATOR_AVAILABILITY_ENVELOPE: Envelope = Envelope::new(b"OMGAVA\0\0", 1);

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

impl AllocatorAvailabilityPlan {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_allocator_availability_content(self);
        let identity = allocator_availability_identity(self);
        ALLOCATOR_AVAILABILITY_ENVELOPE.frame(identity.bytes(), &content)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, AllocatorAvailabilityDecodeError> {
        let (mut cursor, identity) = ALLOCATOR_AVAILABILITY_ENVELOPE.open(
            encoded,
            || AllocatorAvailabilityDecodeError::WrongMagic,
            AllocatorAvailabilityDecodeError::UnsupportedVersion,
        )?;
        let identity = AllocatorAvailabilityIdentity::from_bytes(identity);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let physical = PhysicalRegisterModelIdentity::from_bytes(cursor.array()?);
        let policy = match cursor.byte()? {
            0 => AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
            1 => {
                let views = crate::codec_bytes::decode_list::<_, AllocatorAvailabilityDecodeError>(
                    &mut cursor,
                    |cursor| Ok(RegisterViewId(cursor.u16()?)),
                )?;
                AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views }
            }
            tag => return Err(AllocatorAvailabilityDecodeError::UnknownPolicy(tag)),
        };
        let classes = crate::codec_bytes::decode_list::<_, AllocatorAvailabilityDecodeError>(
            &mut cursor,
            |cursor| {
                let class = RegisterClassId(cursor.u16()?);
                let unconstrained_views =
                    crate::codec_bytes::decode_list::<_, AllocatorAvailabilityDecodeError>(
                        &mut *cursor,
                        |cursor| Ok(RegisterViewId(cursor.u16()?)),
                    )?;
                Ok(RegisterClassAvailability {
                    class,
                    unconstrained_views,
                })
            },
        )?;
        if cursor.remaining() != 0 {
            return Err(AllocatorAvailabilityDecodeError::TrailingBytes);
        }
        let plan = Self {
            register_environment,
            physical,
            policy,
            classes,
        };
        if allocator_availability_identity(&plan) != identity {
            return Err(AllocatorAvailabilityDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterClassAvailability {
    pub class: RegisterClassId, // Physical register class identity
    pub unconstrained_views: Vec<RegisterViewId>, // Sorted allocatable views for this class
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocatorAvailabilityDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    LengthOverflow,
    TrailingBytes,
    IdentityMismatch,
}

crate::decode_error! { AllocatorAvailabilityDecodeError, "invalid terminal allocator availability" }

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

crate::identity_newtype! { pub struct AllocationLegalityIdentity(pub(crate) [u8; 32]); }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationLegalityPlan {
    pub ranges: LiveRangeIdentity, // Live-range digest
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

pub fn allocation_legality_identity_from_digests(
    plan: &AllocationLegalityPlan,
    digests: &[[u8; 32]],
) -> AllocationLegalityIdentity {
    assert_eq!(
        digests.len(),
        plan.functions.len(),
        "one digest per allocation-legality function"
    );
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-allocation-legality.v8\0");
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    crate::codec_bytes::length(&mut bytes, digests.len(), "allocation-legality identity");
    for digest in digests {
        bytes.extend_from_slice(digest);
    }
    AllocationLegalityIdentity::from_bytes(content_identity(&bytes))
}

pub fn allocation_legality_function_digest(function: &FunctionAllocationLegality) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(256 + function.virtual_registers.len() * 48);
    bytes.extend_from_slice(b"omega.terminal-allocation-legality-function.v2\0");
    crate::codec_bytes::push_encoded(&mut bytes, function.machine.get().to_le_bytes());
    crate::codec_bytes::length(
        &mut bytes,
        function.virtual_registers.len(),
        "allocation-legality identity",
    );
    for register in &function.virtual_registers {
        crate::codec_bytes::push_encoded(&mut bytes, register.virtual_register.0.to_le_bytes());
        crate::codec_bytes::push_encoded(&mut bytes, register.class.0.to_le_bytes());
        crate::codec_bytes::length(
            &mut bytes,
            register.spans.len(),
            "allocation-legality identity",
        );
        for point in &register.spans {
            crate::codec_bytes::push_encoded(&mut bytes, point.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.start.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.end.0.to_le_bytes());
            crate::codec_bytes::length(
                &mut bytes,
                point.candidates.len(),
                "allocation-legality identity",
            );
            for candidate in &point.candidates {
                crate::codec_bytes::push_encoded(&mut bytes, candidate.0.to_le_bytes());
            }
        }
        crate::codec_bytes::length(
            &mut bytes,
            register.early_clobber_points.len(),
            "allocation-legality identity",
        );
        for point in &register.early_clobber_points {
            crate::codec_bytes::push_encoded(&mut bytes, point.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.position.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.operand.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, point.point.0.to_le_bytes());
            crate::codec_bytes::length(
                &mut bytes,
                point.candidates.len(),
                "allocation-legality identity",
            );
            for candidate in &point.candidates {
                crate::codec_bytes::push_encoded(&mut bytes, candidate.0.to_le_bytes());
            }
        }
        crate::codec_bytes::length(
            &mut bytes,
            register.entry_transitions.len(),
            "allocation-legality identity",
        );
        for transition in &register.entry_transitions {
            crate::codec_bytes::push_encoded(&mut bytes, transition.from_view.0.to_le_bytes());
            encode_fixed_constraint_site_identity(&mut bytes, transition.to_site);
            crate::codec_bytes::push_encoded(&mut bytes, transition.to_view.0.to_le_bytes());
        }
    }
    content_identity(&bytes)
}

crate::identity_newtype! { pub struct FixedPrecoloredIntervalPlanIdentity(pub(crate) [u8; 32]); }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredIntervalPolicy {
    FixedConstraintPointIntervalsV1, // Fixed constraints occupy [point, point + 1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredIntervalPlan {
    pub ranges: LiveRangeIdentity,            // Live-range digest
    pub legality: AllocationLegalityIdentity, // Allocation-legality digest
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub allocator_availability: AllocatorAvailabilityIdentity, // Allocator availability digest
    pub optimization_unit: OptimizationUnitIdentity, // Optimization unit digest
    pub work_schedule: WorkScheduleIdentity,  // Work schedule identity
    pub policy: FixedPrecoloredIntervalPolicy, // Interval derivation policy
    pub budget: OptimizationWorkBudget,       // Optimization work budget
    pub usage: OptimizationWorkUsage,         // Optimization work usage
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

pub fn fixed_precolored_interval_plan_identity(
    plan: &FixedPrecoloredIntervalPlan,
) -> FixedPrecoloredIntervalPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.fixed-precolored-point-intervals.v2\0");
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_interval_family(&mut bytes, &plan.functions);
    FixedPrecoloredIntervalPlanIdentity(content_identity(&bytes))
}

fn encode_interval_family(bytes: &mut Vec<u8>, functions: &[FunctionFixedPrecoloredIntervals]) {
    crate::codec_bytes::length(bytes, functions.len(), "fixed/precolored interval");
    for function in functions {
        crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
        crate::codec_bytes::length(bytes, function.intervals.len(), "fixed/precolored interval");
        for interval in &function.intervals {
            crate::codec_bytes::push_encoded(bytes, interval.virtual_register.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, interval.class.0.to_le_bytes());
            encode_fixed_constraint_site_identity(bytes, interval.site);
            crate::codec_bytes::push_encoded(bytes, interval.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, interval.start.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, interval.end.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, interval.view.0.to_le_bytes());
        }
    }
}

crate::identity_newtype! { pub struct FixedPrecoloredSplitRequirementPlanIdentity(pub(crate) [u8; 32]); }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedPrecoloredSplitRequirementPolicy {
    FixedUseBoundaryRequirementsV1, // Partition acyclic fragment trees when a fixed use empties the domain
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPrecoloredSplitRequirementPlan {
    pub fixed_intervals: FixedPrecoloredIntervalPlanIdentity, // Fixed intervals digest
    pub ranges: LiveRangeIdentity,                            // Live-range digest
    pub legality: AllocationLegalityIdentity,                 // Allocation-legality digest
    pub register_environment: TargetRegisterEnvironmentIdentity, // Register environment digest
    pub allocator_availability: AllocatorAvailabilityIdentity, // Allocator availability digest
    pub optimization_unit: OptimizationUnitIdentity,          // Optimization unit digest
    pub work_schedule: WorkScheduleIdentity,                  // Work schedule identity
    pub target: NativeTarget,                                 // Target architecture and format
    pub policy: FixedPrecoloredSplitRequirementPolicy,        // Split requirement policy
    pub budget: OptimizationWorkBudget,                       // Optimization work budget
    pub usage: OptimizationWorkUsage,                         // Optimization work usage
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

pub fn fixed_precolored_split_requirement_plan_identity(
    plan: &FixedPrecoloredSplitRequirementPlan,
) -> FixedPrecoloredSplitRequirementPlanIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.fixed-precolored-split-requirements.v2\0");
    bytes.extend_from_slice(&plan.fixed_intervals.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    crate::codec_bytes::push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
    crate::codec_bytes::encode_target(&mut bytes, plan.target);
    bytes.push(match plan.policy {
        FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_split_requirement_family(&mut bytes, &plan.functions);
    FixedPrecoloredSplitRequirementPlanIdentity(content_identity(&bytes))
}

fn encode_split_requirement_family(
    bytes: &mut Vec<u8>,
    functions: &[FunctionFixedPrecoloredSplitRequirements],
) {
    crate::codec_bytes::length(bytes, functions.len(), "fixed/precolored split requirement");
    for function in functions {
        crate::codec_bytes::push_encoded(bytes, function.machine.get().to_le_bytes());
        crate::codec_bytes::length(
            bytes,
            function.registers.len(),
            "fixed/precolored split requirement",
        );
        for register in &function.registers {
            crate::codec_bytes::push_encoded(bytes, register.virtual_register.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, register.class.0.to_le_bytes());
            crate::codec_bytes::length(
                bytes,
                register.fragments.len(),
                "fixed/precolored split requirement",
            );
            for fragment in &register.fragments {
                crate::codec_bytes::push_encoded(bytes, fragment.block.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, fragment.source_start.0.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, fragment.source_end.0.to_le_bytes());
                crate::codec_bytes::length(
                    bytes,
                    fragment.segments.len(),
                    "fixed/precolored split requirement",
                );
                for segment in &fragment.segments {
                    crate::codec_bytes::push_encoded(bytes, segment.id.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, segment.start.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, segment.end.0.to_le_bytes());
                    crate::codec_bytes::length(
                        bytes,
                        segment.candidates.len(),
                        "fixed/precolored split requirement",
                    );
                    for view in &segment.candidates {
                        crate::codec_bytes::push_encoded(bytes, view.0.to_le_bytes());
                    }
                    match segment.opening {
                        FixedPrecoloredSourceSegmentOpening::SourceRangeStartV1 => bytes.push(0),
                        FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 {
                            connector,
                        } => {
                            bytes.push(1);
                            encode_connector(bytes, connector);
                        }
                        FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 {
                            incoming,
                            site,
                            destination_view,
                        } => {
                            bytes.push(2);
                            bytes.push(u8::from(incoming.is_some()));
                            if let Some(connector) = incoming {
                                encode_connector(bytes, connector);
                            }
                            encode_fixed_constraint_site_identity(bytes, site);
                            crate::codec_bytes::push_encoded(
                                bytes,
                                destination_view.0.to_le_bytes(),
                            );
                        }
                    }
                }
            }
        }
    }
}

fn encode_connector(bytes: &mut Vec<u8>, connector: LiveRangeEdgeConnector) {
    crate::codec_bytes::push_encoded(bytes, connector.source.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, connector.terminator.0.to_le_bytes());
    bytes.push(connector.polarity_ordinal);
    crate::codec_bytes::push_encoded(
        bytes,
        connector
            .psi_edge
            .map_or(0, |edge| edge.get())
            .to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(bytes, connector.target.0.to_le_bytes());
}

pub mod allocator_availability {
    pub use super::{
        AllocatorAvailabilityDecodeError, AllocatorAvailabilityIdentity, AllocatorAvailabilityPlan,
        AllocatorAvailabilityPolicy, RegisterClassAvailability, allocator_availability_identity,
    };
}

pub mod allocation_legality {
    pub use super::{
        AllocationLegalityIdentity, AllocationLegalityPlan, EntryFixedViewTransition,
        FunctionAllocationLegality, VirtualEarlyClobberPointLegality, VirtualLegalitySpan,
        VirtualRegisterAllocationLegality, allocation_legality_function_digest,
        allocation_legality_identity_from_digests,
    };
}

pub mod fixed_precolored_intervals {
    pub use super::{
        FixedPrecoloredInterval, FixedPrecoloredIntervalPlan, FixedPrecoloredIntervalPlanIdentity,
        FixedPrecoloredIntervalPolicy, FunctionFixedPrecoloredIntervals,
        fixed_precolored_interval_plan_identity,
    };
}

pub mod fixed_precolored_split_requirements {
    pub use super::{
        FixedPrecoloredRegisterSplitRequirements, FixedPrecoloredSourceFragmentRequirements,
        FixedPrecoloredSourceSegment, FixedPrecoloredSourceSegmentId,
        FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPlan,
        FixedPrecoloredSplitRequirementPlanIdentity, FixedPrecoloredSplitRequirementPolicy,
        FunctionFixedPrecoloredSplitRequirements, fixed_precolored_split_requirement_plan_identity,
    };
}

#[cfg(test)]
mod tests {
    use super::{
        AllocatorAvailabilityDecodeError, AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy,
        RegisterClassAvailability, allocator_availability_identity,
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

    #[test]
    fn codec_round_trips_plain_plan_and_rejects_framing_or_tamper() {
        let source = plan();
        let encoded = source.encode();
        assert_eq!(AllocatorAvailabilityPlan::decode(&encoded).unwrap(), source);

        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            AllocatorAvailabilityPlan::decode(&wrong_magic),
            Err(AllocatorAvailabilityDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        crate::codec_bytes::patch_encoded(&mut wrong_version, 8, 2_u32.to_le_bytes());
        assert_eq!(
            AllocatorAvailabilityPlan::decode(&wrong_version),
            Err(AllocatorAvailabilityDecodeError::UnsupportedVersion(2))
        );
        assert_eq!(
            AllocatorAvailabilityPlan::decode(&encoded[..encoded.len() - 1]),
            Err(AllocatorAvailabilityDecodeError::Truncated)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            AllocatorAvailabilityPlan::decode(&trailing),
            Err(AllocatorAvailabilityDecodeError::TrailingBytes)
        );
        let mut tampered = encoded;
        *tampered.last_mut().unwrap() ^= 1;
        assert_eq!(
            AllocatorAvailabilityPlan::decode(&tampered),
            Err(AllocatorAvailabilityDecodeError::IdentityMismatch)
        );
    }
}
