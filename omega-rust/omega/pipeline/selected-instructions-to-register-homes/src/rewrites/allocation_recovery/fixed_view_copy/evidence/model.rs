use optimization_core::OptimizationWorkUsage;
use register_model::{RegisterClassId, RegisterViewId};
use selected_instructions::{SelectedBlockId, VirtualRegisterId};
use semantic_vocabulary::MachineId;

use crate::{
    FixedPrecoloredHomeDomainId, FixedPrecoloredSourceSegmentId, LiveRangeEdgeConnector,
    VirtualFixedConstraintSite,
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
