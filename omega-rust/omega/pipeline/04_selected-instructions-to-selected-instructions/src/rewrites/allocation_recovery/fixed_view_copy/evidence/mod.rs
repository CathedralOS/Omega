//! Optimizer module role: stage group. Segment-home authority for fixed-view-copy policy.

mod compute;
mod replay;

pub(crate) use compute::derive as derive_positionally;
use optimization_core::OptimizationWorkUsage;
use register_homes::{FixedPrecoloredHomeDomainId, FixedPrecoloredSourceSegmentId};
use register_model::{RegisterClassId, RegisterViewId};
pub(crate) use replay::reconstruct as reconstruct_by_key;
use selected_instructions::{
    LiveRangeEdgeConnector, SelectedBlockId, VirtualFixedConstraintSite, VirtualRegisterId,
};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuthenticatedFixedViewBoundary {
    pub(crate) function: usize,
    pub(crate) machine: MachineId,
    pub(crate) virtual_register: VirtualRegisterId,
    pub(crate) class: RegisterClassId,
    pub(crate) source_segment: FixedPrecoloredSourceSegmentId,
    pub(crate) source_domain: FixedPrecoloredHomeDomainId,
    pub(crate) from_view: RegisterViewId,
    pub(crate) destination_segment: FixedPrecoloredSourceSegmentId,
    pub(crate) destination_domain: FixedPrecoloredHomeDomainId,
    pub(crate) site: VirtualFixedConstraintSite,
    pub(crate) block: SelectedBlockId,
    pub(crate) to_view: RegisterViewId,
    pub(crate) incoming: Option<LiveRangeEdgeConnector>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixedViewBoundaryEvidence {
    pub(crate) boundaries: Vec<AuthenticatedFixedViewBoundary>,
    pub(crate) usage: OptimizationWorkUsage,
}
