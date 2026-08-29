//! Closed source taxonomy for allocation recovery.

mod active_resident;
mod fixed_view;
mod projection;

use crate::{
    StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
};

pub(super) use active_resident::*;
pub(super) use fixed_view::*;

#[derive(Debug)]
pub enum StagedAllocationRecoveryFunctionRelativeSource {
    FixedViewCopies(Box<StagedOptimizedRegisterHomesAfterFixedViewCopies>),
    ActiveResidentRematerialization(Box<StagedOptimizedActiveResidentRematerialization>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagedAllocationRecoverySourceCustodyReceipt {
    FixedViewCopies(StagedOptimizedPostCopyRegisterHomeCustodyReceipt),
    ActiveResidentRematerialization(StagedOptimizedActiveResidentRematerializationCustodyReceipt),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationRecoverySourceKind {
    FixedViewCopiesV1,
    ActiveResidentRematerializationV1,
}
