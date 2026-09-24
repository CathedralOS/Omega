//! Optimizer module role: stage group. Shared manifest construction for canonical fixed-frame realization.
mod fixed_frame;
mod rel8;
mod statistics;
pub(super) use fixed_frame::{expected_fixed_frame_manifest, fixed_frame_custody};
