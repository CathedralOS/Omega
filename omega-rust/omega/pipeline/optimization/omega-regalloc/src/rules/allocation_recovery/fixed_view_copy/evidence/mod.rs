//! Optimizer module role: stage group. Segment-home authority for fixed-view-copy policy.

mod compute;
mod model;
mod replay;

pub(crate) use model::{AuthenticatedFixedViewBoundary, FixedViewBoundaryEvidence};

pub(crate) use compute::derive as derive_positionally;
pub(crate) use replay::reconstruct as reconstruct_by_key;
