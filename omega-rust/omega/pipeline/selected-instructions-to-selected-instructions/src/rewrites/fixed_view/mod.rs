//! Optimizer module role: stage group. Allocation legality to exact fixed-view copies.
//!
//! Fixed-interval and segment-home analysis are internal prerequisites of the
//! selected-CFG transformation. Their facts never bypass the mandatory
//! post-transformation reanalysis stage.

mod fixed_precolored_segment_homes;
mod fixed_view_copies;

pub use fixed_precolored_segment_homes::*;
pub use fixed_view_copies::*;
