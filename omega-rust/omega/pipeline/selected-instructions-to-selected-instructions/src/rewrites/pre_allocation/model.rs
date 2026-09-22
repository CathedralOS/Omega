//! Pre-allocation carriers, receipts, schedules, and errors.
use crate::AllocationLegalityError;
use crate::CopyRemovalError;
use crate::LiveRangeError;
use crate::LivenessError;
use crate::OptimizedAllocationLegalityCustodyError;
use crate::StagedOptimizedAllocationLegality;
use crate::StagedOptimizedAllocationLegalityCustodyReceipt;
use crate::ValidatedAllocationLegality;
use crate::ValidatedCopyRemoval;
use crate::ValidatedLiveRanges;
use crate::ValidatedLiveness;
use optimization_core::{
    Optimization, OptimizationSelectionIdentity, OptimizationSelections, OptimizationWorkBudget,
    OptimizationWorkUsage, PreAllocationOptimizationCompletionIdentity,
};
use selected_instructions::{
    CopyRemovalIdentity, LiveRangeIdentity, LivenessIdentity, SelectedInstructionId,
    SelectedInstructionPlanIdentity,
};

/// Narrow copy-removal admission set: which exact catalog payloads the
/// selected pre-allocation rules enable. This is not an opt level or a
/// generic instruction-combining grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CopyRemovalPolicy {
    enabled: u32,
}

impl CopyRemovalPolicy {
    const SAME_BLOCK_COPY_I64_V1_BIT: u32 = 1 << 0;
    const KNOWN_BITS: u32 = Self::SAME_BLOCK_COPY_I64_V1_BIT;

    /// Rebind every admissible same-block use of one `CopyI64` destination
    /// to the copied source register, dropping the copy and the
    /// destination's roster row.
    pub const SAME_BLOCK_COPY_I64_V1: Self = Self {
        enabled: Self::SAME_BLOCK_COPY_I64_V1_BIT,
    };

    pub const fn empty() -> Self {
        Self { enabled: 0 }
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            enabled: self.enabled | other.enabled,
        }
    }

    /// Whether `other`'s enabled rules are a subset of this policy's.
    pub const fn contains(self, other: Self) -> bool {
        self.enabled & other.enabled == other.enabled
    }

    pub const fn canonical_bits(self) -> u32 {
        self.enabled
    }

    pub const fn from_canonical_bits(bits: u32) -> Option<Self> {
        if bits & !Self::KNOWN_BITS != 0 {
            return None;
        }
        Some(Self { enabled: bits })
    }
}

/// One independently validated copy removal plus the rebuilt analyses over
/// its transformed program. No source analysis fact crosses the transformed
/// selected-CFG boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedCopyRemovalStep {
    pub(super) removal: ValidatedCopyRemoval,
    pub(super) liveness: ValidatedLiveness,
    pub(super) ranges: ValidatedLiveRanges,
    pub(super) legality: ValidatedAllocationLegality,
    /// Candidates the pass evaluated and declined before this removal
    /// committed, in the pass's deterministic scan order.
    pub(super) declined: usize,
    /// Candidates the pass evaluated in total, this commit included.
    pub(super) evaluated: usize,
}

impl StagedOptimizedCopyRemovalStep {
    pub const fn removal(&self) -> &ValidatedCopyRemoval {
        &self.removal
    }
    pub const fn liveness(&self) -> &ValidatedLiveness {
        &self.liveness
    }
    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        &self.ranges
    }
    pub const fn legality(&self) -> &ValidatedAllocationLegality {
        &self.legality
    }
    pub const fn declined(&self) -> usize {
        self.declined
    }
    pub const fn evaluated(&self) -> usize {
        self.evaluated
    }
}

/// The terminal no-change discovery pass: every remaining `CopyI64`
/// candidate was evaluated and declined, proving the run reached the
/// admission fixed point on the final program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedCopyRemovalAttempt {
    pub(super) source_selected: SelectedInstructionPlanIdentity,
    pub(super) candidates: usize,
    pub(super) declined: usize,
}

impl StagedOptimizedCopyRemovalAttempt {
    pub const fn source_selected(&self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn candidates(&self) -> usize {
        self.candidates
    }
    pub const fn declined(&self) -> usize {
        self.declined
    }
}

/// Completed execution of the pre-allocation projection of one exact
/// source-visible suite. Applied steps are followed by one clean discovery
/// pass, so an empty `steps` vector is still an evidenced successful run.
#[derive(Debug)]
pub struct StagedPreAllocationOptimizationRun {
    pub(super) source: StagedOptimizedAllocationLegality,
    pub(super) selections: OptimizationSelections,
    pub(super) pre_allocation_selections: OptimizationSelections,
    pub(super) policy: CopyRemovalPolicy,
    pub(super) steps: Vec<StagedOptimizedCopyRemovalStep>,
    pub(super) attempt: StagedOptimizedCopyRemovalAttempt,
    pub(super) custody: StagedPreAllocationOptimizationCustodyReceipt,
}

impl StagedPreAllocationOptimizationRun {
    pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        &self.source
    }
    /// The complete authored suite admitted with the run, never narrowed.
    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }
    /// The exact pre-allocation rules the suite selected.
    pub const fn pre_allocation_selections(&self) -> &OptimizationSelections {
        &self.pre_allocation_selections
    }
    pub const fn policy(&self) -> CopyRemovalPolicy {
        self.policy
    }
    pub fn steps(&self) -> &[StagedOptimizedCopyRemovalStep] {
        &self.steps
    }
    pub const fn attempt(&self) -> &StagedOptimizedCopyRemovalAttempt {
        &self.attempt
    }
    pub const fn custody(&self) -> &StagedPreAllocationOptimizationCustodyReceipt {
        &self.custody
    }
    /// The program this run publishes: the last step's transformed plan when
    /// a removal committed, otherwise the unchanged admitted source plan.
    pub fn current(&self) -> crate::SelectedProgramRef<'_> {
        match self.steps.last() {
            Some(step) => crate::SelectedProgramRef::new(&step.removal),
            None => crate::SelectedProgramRef::new(self.source.selected()),
        }
    }
    /// The facts over the current program: the last step's rebuilt analyses,
    /// or the run's admitted analyses when no copy was admissible.
    pub fn liveness(&self) -> &ValidatedLiveness {
        match self.steps.last() {
            Some(step) => step.liveness(),
            None => self.source.liveness(),
        }
    }
    pub fn ranges(&self) -> &ValidatedLiveRanges {
        match self.steps.last() {
            Some(step) => step.ranges(),
            None => self.source.ranges(),
        }
    }
    pub fn legality(&self) -> &ValidatedAllocationLegality {
        match self.steps.last() {
            Some(step) => step.legality(),
            None => self.source.legality(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedPreAllocationOptimizationCustodyReceipt {
    pub(super) identity: PreAllocationOptimizationCompletionIdentity,
    pub(super) source: StagedOptimizedAllocationLegalityCustodyReceipt,
    pub(super) selections: OptimizationSelectionIdentity,
    pub(super) pre_allocation_selections: OptimizationSelectionIdentity,
    pub(super) policy: CopyRemovalPolicy,
    pub(super) budget: OptimizationWorkBudget,
    pub(super) usage: OptimizationWorkUsage,
    pub(super) iteration_bound: usize,
    pub(super) removal_count: usize,
    pub(super) initial_virtual_register_count: usize,
    pub(super) iterations: Vec<StagedOptimizedCopyRemovalIterationReceipt>,
    pub(super) attempt: StagedOptimizedCopyRemovalAttemptReceipt,
    pub(super) final_selected: SelectedInstructionPlanIdentity,
    pub(super) final_liveness: LivenessIdentity,
    pub(super) final_ranges: LiveRangeIdentity,
    pub(super) final_legality: register_homes::AllocationLegalityIdentity,
    pub(super) final_virtual_register_count: usize,
}

impl StagedPreAllocationOptimizationCustodyReceipt {
    pub const fn identity(&self) -> PreAllocationOptimizationCompletionIdentity {
        self.identity
    }
    pub const fn source(&self) -> StagedOptimizedAllocationLegalityCustodyReceipt {
        self.source
    }
    pub const fn selections(&self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn pre_allocation_selections(&self) -> OptimizationSelectionIdentity {
        self.pre_allocation_selections
    }
    pub const fn policy(&self) -> CopyRemovalPolicy {
        self.policy
    }
    pub const fn budget(&self) -> OptimizationWorkBudget {
        self.budget
    }
    pub const fn usage(&self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn iteration_bound(&self) -> usize {
        self.iteration_bound
    }
    pub const fn removal_count(&self) -> usize {
        self.removal_count
    }
    pub const fn initial_virtual_register_count(&self) -> usize {
        self.initial_virtual_register_count
    }
    pub fn iterations(&self) -> &[StagedOptimizedCopyRemovalIterationReceipt] {
        &self.iterations
    }
    pub const fn attempt(&self) -> StagedOptimizedCopyRemovalAttemptReceipt {
        self.attempt
    }
    pub const fn final_selected(&self) -> SelectedInstructionPlanIdentity {
        self.final_selected
    }
    pub const fn final_liveness(&self) -> LivenessIdentity {
        self.final_liveness
    }
    pub const fn final_ranges(&self) -> LiveRangeIdentity {
        self.final_ranges
    }
    pub const fn final_legality(&self) -> register_homes::AllocationLegalityIdentity {
        self.final_legality
    }
    pub const fn final_virtual_register_count(&self) -> usize {
        self.final_virtual_register_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedCopyRemovalIterationReceipt {
    pub(super) source_selected: SelectedInstructionPlanIdentity,
    pub(super) copy_removal: CopyRemovalIdentity,
    pub(super) function_index: usize,
    pub(super) copy: SelectedInstructionId,
    pub(super) transformed_selected: SelectedInstructionPlanIdentity,
    pub(super) fresh_liveness: LivenessIdentity,
    pub(super) fresh_ranges: LiveRangeIdentity,
    pub(super) fresh_legality: register_homes::AllocationLegalityIdentity,
    pub(super) declined: usize,
    pub(super) evaluated: usize,
}

impl StagedOptimizedCopyRemovalIterationReceipt {
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn copy_removal(self) -> CopyRemovalIdentity {
        self.copy_removal
    }
    pub const fn function_index(self) -> usize {
        self.function_index
    }
    pub const fn copy(self) -> SelectedInstructionId {
        self.copy
    }
    pub const fn transformed_selected(self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn fresh_liveness(self) -> LivenessIdentity {
        self.fresh_liveness
    }
    pub const fn fresh_ranges(self) -> LiveRangeIdentity {
        self.fresh_ranges
    }
    pub const fn fresh_legality(self) -> register_homes::AllocationLegalityIdentity {
        self.fresh_legality
    }
    pub const fn declined(self) -> usize {
        self.declined
    }
    pub const fn evaluated(self) -> usize {
        self.evaluated
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedCopyRemovalAttemptReceipt {
    pub(super) source_selected: SelectedInstructionPlanIdentity,
    pub(super) candidates: usize,
    pub(super) declined: usize,
}

impl StagedOptimizedCopyRemovalAttemptReceipt {
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn candidates(self) -> usize {
        self.candidates
    }
    pub const fn declined(self) -> usize {
        self.declined
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedCopyRemovalCustodyError {
    UpstreamLegality(OptimizedAllocationLegalityCustodyError),
    CopyRemoval(CopyRemovalError),
    Liveness(LivenessError),
    LiveRanges(LiveRangeError),
    AllocationLegality(AllocationLegalityError),
    RemainingTransitions {
        count: usize,
    },
    StepMismatch {
        step: usize,
    },
    TerminalAttemptMismatch,
    ReceiptMismatch,
    MissingPreAllocationOptimization,
    UnsupportedPreAllocationOptimization(Optimization),
    /// Every applied step must drop the virtual-register measure by exactly
    /// one: the copy and its destination's roster row leave together.
    CopyRemovalMeasureMismatch {
        previous: usize,
        current: usize,
    },
    PreAllocationIterationBoundExceeded {
        bound: usize,
    },
    SelectionProjectionMismatch,
    WorkOverflow,
    PreAllocationBudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

impl std::fmt::Display for OptimizedCopyRemovalCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "optimized copy-removal staging failed: {self:?}")
    }
}

impl std::error::Error for OptimizedCopyRemovalCustodyError {}
