//! Optimizer module role: stage group. Allocation legality to exact fixed-view copies.
//!
//! Fixed-interval and segment-home analysis are internal prerequisites of the
//! selected-CFG transformation. Their facts never bypass the mandatory
//! post-transformation reanalysis stage.

mod fixed_precolored_segment_homes;
mod fixed_view_copies;

#[cfg(any(test, feature = "test-support"))]
pub use fixed_precolored_segment_homes::OptimizedFixedPrecoloredSegmentHomeCustodyFieldForTest;
pub use fixed_precolored_segment_homes::{
    FixedPrecoloredSegmentHomeDecline, OptimizedFixedPrecoloredSegmentHomeCustodyError,
    StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
    StagedOptimizedFixedPrecoloredSegmentHomes, probe_optimized_fixed_precolored_segment_homes,
    stage_optimized_fixed_precolored_segment_homes,
    validate_optimized_fixed_precolored_segment_home_custody,
};
#[cfg(any(test, feature = "test-support"))]
pub use fixed_view_copies::OptimizedFixedViewCopyCustodyFieldForTest;
pub use fixed_view_copies::{
    OptimizedFixedViewCopyCustodyError, StagedOptimizedFixedViewCopies,
    StagedOptimizedFixedViewCopyCustodyReceipt, stage_optimized_fixed_view_copies,
    validate_optimized_fixed_view_copy_custody,
};
