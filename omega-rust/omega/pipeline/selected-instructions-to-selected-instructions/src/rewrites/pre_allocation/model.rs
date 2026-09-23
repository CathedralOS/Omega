//! Pre-allocation carriers, receipts, schedules, and errors.
use std::sync::Arc;

use crate::AllocationLegalityError;
use crate::CopyRemovalError;
use crate::LiveRangeError;
use crate::LivenessError;
use crate::OptimizedAllocationLegalityCustodyError;
use crate::RedundantExtensionError;
use crate::StagedOptimizedAllocationLegality;
use crate::StagedOptimizedAllocationLegalityCustodyReceipt;
use crate::ValidatedAllocationLegality;
use crate::ValidatedCopyRemoval;
use crate::ValidatedLiveRanges;
use crate::ValidatedLiveness;
use crate::ValidatedRedundantExtension;
use optimization_core::{
    Optimization, OptimizationSelectionIdentity, OptimizationSelections, OptimizationWorkBudget,
    OptimizationWorkUsage, PreAllocationOptimizationCompletionIdentity,
};
use selected_instructions::{
    CopyRemovalIdentity, LiveRangeIdentity, LivenessIdentity, RedundantExtensionIdentity,
    SelectedInstructionId, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
};

/// Narrow pre-allocation admission set: which exact catalog payloads the
/// selected pre-allocation rules enable. This is not an opt level or a
/// generic instruction-combining grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreAllocationPolicy {
    enabled: u32,
}

impl PreAllocationPolicy {
    const SAME_BLOCK_COPY_I64_V1_BIT: u32 = 1 << 0;
    const REDUNDANT_EXTENSION_V1_BIT: u32 = 1 << 1;
    const KNOWN_BITS: u32 = Self::SAME_BLOCK_COPY_I64_V1_BIT | Self::REDUNDANT_EXTENSION_V1_BIT;

    /// Rebind every admissible same-block use of one `CopyI64` destination
    /// to the copied source register, dropping the copy and the
    /// destination's roster row.
    pub const SAME_BLOCK_COPY_I64_V1: Self = Self {
        enabled: Self::SAME_BLOCK_COPY_I64_V1_BIT,
    };

    /// Replace an admitted `ZeroExtend*`/`SignExtend*` whose input's producer
    /// already guarantees the same normalization with a `CopyI64` allocation
    /// can coalesce, keeping the extension's result register and instruction
    /// identity.
    pub const REDUNDANT_EXTENSION_V1: Self = Self {
        enabled: Self::REDUNDANT_EXTENSION_V1_BIT,
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

/// One committed pre-allocation step's validated transformation, by exact
/// rewrite family. Both variants implement `ValidatedSelectedAnalysis`, so
/// the step's transformed program re-enters discovery as the next sweep's
/// source — an extension removal publishes a `CopyI64` the copy-removal
/// pass then owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatedPreAllocationTransformation {
    CopyRemoval(ValidatedCopyRemoval),
    RedundantExtension(ValidatedRedundantExtension),
}

impl ValidatedPreAllocationTransformation {
    pub fn transformed(&self) -> &SelectedInstructionPlan {
        match self {
            Self::CopyRemoval(removal) => removal.transformed(),
            Self::RedundantExtension(removal) => removal.transformed(),
        }
    }

    pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
        match self {
            Self::CopyRemoval(removal) => removal.shared_transformed(),
            Self::RedundantExtension(removal) => removal.shared_transformed(),
        }
    }

    pub fn source_selected(&self) -> SelectedInstructionPlanIdentity {
        match self {
            Self::CopyRemoval(removal) => removal.receipt().source_selected(),
            Self::RedundantExtension(removal) => removal.receipt().source_selected(),
        }
    }

    pub fn transformed_selected(&self) -> SelectedInstructionPlanIdentity {
        match self {
            Self::CopyRemoval(removal) => removal.receipt().transformed_selected(),
            Self::RedundantExtension(removal) => removal.receipt().transformed_selected(),
        }
    }

    pub fn optimization_unit(&self) -> optimization_core::OptimizationUnitIdentity {
        match self {
            Self::CopyRemoval(removal) => removal.receipt().optimization_unit(),
            Self::RedundantExtension(removal) => removal.receipt().optimization_unit(),
        }
    }

    pub fn fuel_schedule(&self) -> semantic_vocabulary::FuelScheduleIdentity {
        match self {
            Self::CopyRemoval(removal) => removal.receipt().fuel_schedule(),
            Self::RedundantExtension(removal) => removal.receipt().fuel_schedule(),
        }
    }

    /// The function the committed transformation rewrote.
    pub fn function_index(&self) -> usize {
        match self {
            Self::CopyRemoval(removal) => removal.receipt().function_index(),
            Self::RedundantExtension(removal) => removal.receipt().function_index(),
        }
    }

    /// The source-side identity of the instruction the transformation
    /// rewrote: the removed `CopyI64`, or the extension that became one.
    pub fn instruction(&self) -> SelectedInstructionId {
        match self {
            Self::CopyRemoval(removal) => removal.receipt().copy(),
            Self::RedundantExtension(removal) => removal.receipt().extension(),
        }
    }

    /// The durable transformation identity the iteration receipt and the
    /// post-allocation manifest ledger record.
    pub fn identity(&self) -> PreAllocationTransformationIdentity {
        match self {
            Self::CopyRemoval(removal) => {
                PreAllocationTransformationIdentity::CopyRemoval(removal.receipt().identity())
            }
            Self::RedundantExtension(removal) => {
                PreAllocationTransformationIdentity::RedundantExtension(
                    removal.receipt().identity(),
                )
            }
        }
    }
}

/// The committed transformation a pre-allocation iteration receipt records,
/// by exact rewrite family. The manifest ledger cannot reorder or reforge a
/// step without changing this field's replayed value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreAllocationTransformationIdentity {
    CopyRemoval(CopyRemovalIdentity),
    RedundantExtension(RedundantExtensionIdentity),
}

/// One independently validated pre-allocation transformation plus the
/// rebuilt analyses over its transformed program. No source analysis fact
/// crosses the transformed selected-CFG boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPreAllocationStep {
    pub(super) transformation: ValidatedPreAllocationTransformation,
    pub(super) liveness: ValidatedLiveness,
    pub(super) ranges: ValidatedLiveRanges,
    pub(super) legality: ValidatedAllocationLegality,
    /// Candidates the owning pass evaluated and declined before this
    /// transformation committed, in the pass's deterministic scan order.
    pub(super) declined: usize,
    /// Candidates the pass evaluated in total, this commit included.
    pub(super) evaluated: usize,
}

impl StagedOptimizedPreAllocationStep {
    pub const fn transformation(&self) -> &ValidatedPreAllocationTransformation {
        &self.transformation
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

/// The terminal no-change discovery sweep: every enabled family pass
/// evaluated its remaining candidates and declined them, proving the run
/// reached the joint admission fixed point on the final program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedPreAllocationAttempt {
    pub(super) source_selected: SelectedInstructionPlanIdentity,
    pub(super) candidates: usize,
    pub(super) declined: usize,
}

impl StagedOptimizedPreAllocationAttempt {
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
/// sweep, so an empty `steps` vector is still an evidenced successful run.
#[derive(Debug)]
pub struct StagedPreAllocationOptimizationRun {
    pub(super) source: StagedOptimizedAllocationLegality,
    pub(super) selections: OptimizationSelections,
    pub(super) pre_allocation_selections: OptimizationSelections,
    pub(super) policy: PreAllocationPolicy,
    pub(super) steps: Vec<StagedOptimizedPreAllocationStep>,
    pub(super) attempt: StagedOptimizedPreAllocationAttempt,
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
    pub const fn policy(&self) -> PreAllocationPolicy {
        self.policy
    }
    pub fn steps(&self) -> &[StagedOptimizedPreAllocationStep] {
        &self.steps
    }
    pub const fn attempt(&self) -> &StagedOptimizedPreAllocationAttempt {
        &self.attempt
    }
    pub const fn custody(&self) -> &StagedPreAllocationOptimizationCustodyReceipt {
        &self.custody
    }
    /// The program this run publishes: the last step's transformed plan when
    /// a transformation committed, otherwise the unchanged admitted source
    /// plan.
    pub fn current(&self) -> crate::SelectedProgramRef<'_> {
        match self.steps.last() {
            Some(step) => crate::SelectedProgramRef::new(&step.transformation),
            None => crate::SelectedProgramRef::new(self.source.selected()),
        }
    }
    /// The facts over the current program: the last step's rebuilt analyses,
    /// or the run's admitted analyses when no candidate was admissible.
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
    pub(super) policy: PreAllocationPolicy,
    pub(super) budget: OptimizationWorkBudget,
    pub(super) usage: OptimizationWorkUsage,
    pub(super) iteration_bound: usize,
    pub(super) removal_count: usize,
    pub(super) initial_virtual_register_count: usize,
    pub(super) iterations: Vec<StagedOptimizedPreAllocationIterationReceipt>,
    pub(super) attempt: StagedOptimizedPreAllocationAttemptReceipt,
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
    pub const fn policy(&self) -> PreAllocationPolicy {
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
    pub fn iterations(&self) -> &[StagedOptimizedPreAllocationIterationReceipt] {
        &self.iterations
    }
    pub const fn attempt(&self) -> StagedOptimizedPreAllocationAttemptReceipt {
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
pub struct StagedOptimizedPreAllocationIterationReceipt {
    pub(super) source_selected: SelectedInstructionPlanIdentity,
    pub(super) transformation: PreAllocationTransformationIdentity,
    pub(super) function_index: usize,
    pub(super) instruction: SelectedInstructionId,
    pub(super) transformed_selected: SelectedInstructionPlanIdentity,
    pub(super) fresh_liveness: LivenessIdentity,
    pub(super) fresh_ranges: LiveRangeIdentity,
    pub(super) fresh_legality: register_homes::AllocationLegalityIdentity,
    pub(super) declined: usize,
    pub(super) evaluated: usize,
}

impl StagedOptimizedPreAllocationIterationReceipt {
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn transformation(self) -> PreAllocationTransformationIdentity {
        self.transformation
    }
    pub const fn function_index(self) -> usize {
        self.function_index
    }
    /// The source-side identity of the instruction the step rewrote.
    pub const fn instruction(self) -> SelectedInstructionId {
        self.instruction
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
pub struct StagedOptimizedPreAllocationAttemptReceipt {
    pub(super) source_selected: SelectedInstructionPlanIdentity,
    pub(super) candidates: usize,
    pub(super) declined: usize,
}

impl StagedOptimizedPreAllocationAttemptReceipt {
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
pub enum OptimizedPreAllocationCustodyError {
    UpstreamLegality(OptimizedAllocationLegalityCustodyError),
    CopyRemoval(CopyRemovalError),
    RedundantExtension(RedundantExtensionError),
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
    /// Every applied step must drop the joint measure — the plan's virtual
    /// registers plus its remaining extension instructions — by exactly
    /// one: a copy removal drops the copy and its destination's roster row;
    /// an extension removal turns the extension into a copy and removes no
    /// register.
    PreAllocationMeasureMismatch {
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

impl std::fmt::Display for OptimizedPreAllocationCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized pre-allocation staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPreAllocationCustodyError {}
