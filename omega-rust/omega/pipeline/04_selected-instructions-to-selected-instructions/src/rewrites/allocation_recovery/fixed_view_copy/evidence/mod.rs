//! Optimizer module role: stage group. Segment-home authority for fixed-view-copy policy.

mod compute;
mod replay;

use crate::register_homes::{FixedPrecoloredHomeDomainId, FixedPrecoloredSourceSegmentId};
pub(crate) use compute::derive as derive_positionally;
use optimization_core::OptimizationWorkUsage;
pub(crate) use replay::reconstruct as reconstruct_by_key;
use semantic_vocabulary::MachineId;
use target_operations_to_selected_instructions::register_model::{RegisterClassId, RegisterViewId};
use target_operations_to_selected_instructions::{
    LiveRangeEdgeConnector, SelectedBlockId, VirtualFixedConstraintSite, VirtualRegisterId,
};

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
