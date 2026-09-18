use crate::{
    FixedPrecoloredIntervalError, FixedPrecoloredIntervalValidationReceipt,
    FixedPrecoloredSegmentHomeError, FixedPrecoloredSegmentHomeValidationReceipt,
    FixedPrecoloredSplitRequirementError, FixedPrecoloredSplitRequirementValidationReceipt,
    ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSegmentHomes,
    ValidatedFixedPrecoloredSplitRequirements,
};

use crate::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, ValidatedLiveRanges, ValidatedLiveness,
};
use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

#[derive(Debug)]
pub struct StagedOptimizedFixedPrecoloredSegmentHomes {
    pub(super) source: StagedOptimizedAllocationLegality,
    pub(super) fixed: ValidatedFixedPrecoloredIntervals,
    pub(super) requirements: ValidatedFixedPrecoloredSplitRequirements,
    pub(super) homes: ValidatedFixedPrecoloredSegmentHomes,
    pub(super) custody: StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
}

impl StagedOptimizedFixedPrecoloredSegmentHomes {
    /// The retained producer stage. Replay and custody validation inspect it;
    /// ordinary consumers read the current program and analyses directly.
    pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        &self.source
    }

    /// The current selected program this probe describes. Segment homes do
    /// not transform the program, so the source program remains current.
    pub const fn selected(&self) -> &ValidatedSelectedInstructions {
        self.source.selected()
    }

    /// The liveness facts over the current program.
    pub const fn liveness(&self) -> &ValidatedLiveness {
        self.source.liveness()
    }

    /// The live ranges over the current program.
    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        self.source.ranges()
    }

    /// The allocation legality over the current program.
    pub const fn legality(&self) -> &ValidatedAllocationLegality {
        self.source.legality()
    }

    /// The environment-derived allocator availability for the program.
    pub const fn allocator_availability(&self) -> &ValidatedAllocatorAvailability {
        self.source.allocator_availability()
    }

    /// The target register environment admitted with the current program.
    pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
        self.source.register_environment()
    }

    /// The governing optimizer selections for this admission.
    pub fn selections(&self) -> &OptimizationSelections {
        self.source.selections()
    }

    /// The per-pass work budget admitted beside the same evidence.
    pub fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.source.budget_per_pass()
    }

    pub const fn fixed_intervals(&self) -> &ValidatedFixedPrecoloredIntervals {
        &self.fixed
    }
    pub const fn split_requirements(&self) -> &ValidatedFixedPrecoloredSplitRequirements {
        &self.requirements
    }
    pub const fn segment_homes(&self) -> &ValidatedFixedPrecoloredSegmentHomes {
        &self.homes
    }
    pub const fn custody(&self) -> StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt {
    pub(super) upstream: StagedOptimizedAllocationLegalityCustodyReceipt,
    pub(super) fixed: FixedPrecoloredIntervalValidationReceipt,
    pub(super) requirements: FixedPrecoloredSplitRequirementValidationReceipt,
    pub(super) homes: FixedPrecoloredSegmentHomeValidationReceipt,
}

impl StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt {
    pub const fn upstream(self) -> StagedOptimizedAllocationLegalityCustodyReceipt {
        self.upstream
    }
    pub const fn fixed(self) -> FixedPrecoloredIntervalValidationReceipt {
        self.fixed
    }
    pub const fn requirements(self) -> FixedPrecoloredSplitRequirementValidationReceipt {
        self.requirements
    }
    pub const fn homes(self) -> FixedPrecoloredSegmentHomeValidationReceipt {
        self.homes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedFixedPrecoloredSegmentHomeCustodyError {
    UpstreamLegality(OptimizedAllocationLegalityCustodyError),
    FixedIntervals(FixedPrecoloredIntervalError),
    SplitRequirements(FixedPrecoloredSplitRequirementError),
    SegmentHomes(FixedPrecoloredSegmentHomeError),
    ReceiptMismatch,
}

/// The capacity verdict a borrow-only segment-home probe can report before
/// the sequence commits: placement reached a domain with no viable physical
/// view (`SegmentPressure`), or a front-end derivation exceeded its per-pass
/// work budget (`BudgetExceeded`) before homes could be assigned. Both mean
/// the sequence cannot serve the program while runtime-spill recovery still
/// can; shape and custody rejections are not declines and keep the staged
/// error surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedPrecoloredSegmentHomeDecline {
    SegmentPressure,
    BudgetExceeded,
}

impl OptimizedFixedPrecoloredSegmentHomeCustodyError {
    /// The capacity verdict this failure reports, when it is one: segment
    /// placement pressure and front-end work-budget exhaustion are the only
    /// outcomes a composing route may answer with a different recovery.
    pub fn capacity_decline(&self) -> Option<FixedPrecoloredSegmentHomeDecline> {
        match self {
            Self::SegmentHomes(FixedPrecoloredSegmentHomeError::SegmentPressure { .. }) => {
                Some(FixedPrecoloredSegmentHomeDecline::SegmentPressure)
            }
            Self::FixedIntervals(FixedPrecoloredIntervalError::BudgetExceeded { .. })
            | Self::SplitRequirements(FixedPrecoloredSplitRequirementError::BudgetExceeded {
                ..
            })
            | Self::SegmentHomes(FixedPrecoloredSegmentHomeError::BudgetExceeded { .. }) => {
                Some(FixedPrecoloredSegmentHomeDecline::BudgetExceeded)
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for OptimizedFixedPrecoloredSegmentHomeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "fixed/precolored segment-home staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedFixedPrecoloredSegmentHomeCustodyError {}
