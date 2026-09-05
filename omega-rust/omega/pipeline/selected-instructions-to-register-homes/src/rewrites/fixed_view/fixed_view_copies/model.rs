use crate::{FixedViewCopyError, ValidatedFixedViewCopies};

use crate::StagedOptimizedAllocationLegality;
use crate::{
    OptimizedFixedPrecoloredSegmentHomeCustodyError, StagedOptimizedFixedPrecoloredSegmentHomes,
};

/// Exact named fixed-view copy materialization over the complete source
/// legality chain. It mutates only its private selected-CFG realization and
/// grants no allocation, emission, or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedFixedViewCopies {
    pub(super) source: StagedOptimizedFixedPrecoloredSegmentHomes,
    pub(super) copies: ValidatedFixedViewCopies,
    pub(super) custody: FixedViewCopyCustodyReceipt,
}

impl StagedOptimizedFixedViewCopies {
    pub const fn source_segment_home_stage(&self) -> &StagedOptimizedFixedPrecoloredSegmentHomes {
        &self.source
    }
    pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        self.source.source_legality_stage()
    }
    pub const fn copies(&self) -> &ValidatedFixedViewCopies {
        &self.copies
    }
    pub const fn custody(&self) -> FixedViewCopyCustodyReceipt {
        self.custody
    }
}

pub use register_homes::FixedViewCopyCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedFixedViewCopyCustodyError {
    UpstreamSegmentHomes(OptimizedFixedPrecoloredSegmentHomeCustodyError),
    Materialization(FixedViewCopyError),
    Revalidation(FixedViewCopyError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedFixedViewCopyCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized fixed-view copy staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedFixedViewCopyCustodyError {}
