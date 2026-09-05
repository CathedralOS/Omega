use crate::{LivenessError, ValidatedLiveness};

use target_operations_to_selected_instructions::{
    OptimizedSelectionCustodyError, StagedOptimizedSelectedInstructions,
};

/// Opt-in liveness staging over the complete selected-instruction custody
/// carrier. This grants no interval, allocation, emission, or publication
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedLiveness {
    pub(super) selected: StagedOptimizedSelectedInstructions,
    pub(super) liveness: ValidatedLiveness,
    pub(super) custody: LivenessCustodyReceipt,
}

impl StagedOptimizedLiveness {
    pub const fn selected_stage(&self) -> &StagedOptimizedSelectedInstructions {
        &self.selected
    }

    pub const fn liveness(&self) -> &ValidatedLiveness {
        &self.liveness
    }

    pub const fn custody(&self) -> LivenessCustodyReceipt {
        self.custody
    }
}

pub use register_homes::LivenessCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedLivenessCustodyError {
    UpstreamSelection(OptimizedSelectionCustodyError),
    Analysis(LivenessError),
    Revalidation(LivenessError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedLivenessCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "optimized liveness staging failed: {self:?}")
    }
}

impl std::error::Error for OptimizedLivenessCustodyError {}
