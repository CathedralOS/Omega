//! Optimizer module role: stage group. Executable spill recovery.

mod recovery;

use crate::{ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes};
use optimization_core::{
    PreAllocationOptimizationCompletionIdentity, PrePhysicalOptimizationManifestIdentity,
    SelectedLoweringOptimizationCompletionIdentity,
};
pub(crate) use recovery::{
    assign_source, recover, recover_after_active_resident_rematerialization,
    recover_after_declined_fixed_view_probe, recover_after_fixed_view_copies,
    recover_after_pre_allocation, recover_after_selected_lowering,
};
use register_homes::{
    PostAllocationOptimizationManifestError, PostAllocationSelectedTransformation,
};
use selected_instructions::SelectedInstructionPlan;
use selected_instructions_to_selected_instructions::{
    AllocationLegalityError, FixedViewCopyPolicy, LiveRangeError, LivenessError,
    RuntimeRematerializationError, RuntimeSpillError, RuntimeSplitError, SelectedProgramRef,
    StagedOptimizedAllocationLegality, StagedOptimizedSelectedReanalysis,
    StagedPreAllocationOptimizationRun, StagedSelectedLoweringOptimizationRun,
    ValidatedAllocationLegality, ValidatedAllocatorAvailability, ValidatedLiveRanges,
    ValidatedLiveness, ValidatedRuntimeRematerialization, ValidatedRuntimeSpill,
    ValidatedRuntimeSplit,
};

/// A completed recovery: its source custody, the recorded steps and the final
/// facts, homes and manifest. Every recovery caller transfers it directly to
/// `RetainedAllocation::try_from`, which publishes the final step's program
/// and facts.
#[derive(Debug)]
pub(crate) struct RuntimeSpillAllocation {
    pub(crate) source: RuntimeSpillSource,
    pub(crate) steps: Vec<RuntimeSpillStep>,
    pub(crate) facts: RuntimeSpillFacts,
    pub(crate) homes: ValidatedRegisterHomes,
    pub(crate) manifest: ValidatedPostAllocationOptimizationManifest,
}

impl RuntimeSpillAllocation {
    /// Replayed custody evidence for retained selection validation: the
    /// declared allocation-recovery selection this recovery's recorded
    /// prefix ran under, when the prefix came from a declared rule.
    pub(crate) fn recovery_prefix_selection(&self) -> Option<optimization_core::Optimization> {
        self.source.recovery_prefix_selection()
    }
}

/// The staged custody a spill recovery began from. The direct route enters
/// with raw legality; a fixed-view sequence that still faces pressure enters
/// with its complete post-copy reanalysis, and the copy transformation stays
/// recorded in the manifest prefix. A declared sequence whose segment-home
/// probe reported a capacity decline before custody was consumed enters with
/// the same legality plus the declined policy, so the produced allocation
/// still binds the declared selection. An active-resident
/// rematerialization sweep whose rebuilt facts still report
/// `NoCompatibleHome` enters with that proven prefix: the rematerialization
/// stays recorded ahead of every spill step. Completed rewrite suites enter
/// the same way: a selected-lowering or pre-allocation run whose final facts
/// still report pressure keeps its committed ledger and suite completion
/// identity as the recorded prefix.
#[derive(Debug)]
pub(crate) enum RuntimeSpillSource {
    Legality(StagedOptimizedAllocationLegality),
    DeclinedFixedView {
        legality: StagedOptimizedAllocationLegality,
        policy: FixedViewCopyPolicy,
    },
    FixedViewCopies(StagedOptimizedSelectedReanalysis),
    ActiveResidentRematerialization(crate::StagedOptimizedActiveResidentRematerializationPressure),
    SelectedLowering(StagedSelectedLoweringOptimizationRun),
    PreAllocation(StagedPreAllocationOptimizationRun),
}

/// The declared manifest base: the pre-physical root, completion identities
/// of executed rewrite suites, and prefix transformations ahead of every
/// spill.
#[derive(Debug)]
pub(crate) struct RuntimeSpillManifestBase {
    pub(crate) pre_physical: PrePhysicalOptimizationManifestIdentity,
    pub(crate) selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pub(crate) pre_allocation_completion: Option<PreAllocationOptimizationCompletionIdentity>,
    pub(crate) prefix: Vec<PostAllocationSelectedTransformation>,
}

impl RuntimeSpillSource {
    /// Every arm shares one underlying legality stage: the fixed-view entries
    /// reach it through the copy transformation's source custody or carry it
    /// directly when the sequence was declined before committing, the
    /// active-resident entry reaches it as the rematerialization's own
    /// staged source, and the completed-suite entries reach it as the run's
    /// admitted source legality — the suites preserve register environment,
    /// selections, budget, and target owner, so the source stage's shared
    /// facts stay current.
    fn legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        match self {
            Self::Legality(source) => source,
            Self::DeclinedFixedView { legality, .. } => legality,
            Self::FixedViewCopies(reanalysis) => {
                reanalysis.transformation_stage().source_legality_stage()
            }
            Self::ActiveResidentRematerialization(pressure) => pressure.source(),
            Self::SelectedLowering(run) => run.source_legality_stage(),
            Self::PreAllocation(run) => run.source_legality_stage(),
        }
    }

    /// The register environment is an upstream fact both arms share through
    /// the underlying legality stage.
    pub(crate) fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.legality_stage().register_environment()
    }

    /// The governing optimizer selections admitted with the shared legality
    /// stage.
    pub(crate) fn selections(&self) -> &optimization_core::OptimizationSelections {
        self.legality_stage().selections()
    }

    /// The per-pass work budget admitted beside the same evidence.
    pub(crate) fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
        self.legality_stage().budget_per_pass()
    }

    /// The optimized-target product the emission stages read, shared by
    /// handle.
    pub(crate) fn optimized_target_owner(
        &self,
    ) -> &std::sync::Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>
    {
        self.legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target_owner()
    }

    pub(crate) fn allocator_availability(&self) -> &ValidatedAllocatorAvailability {
        self.legality_stage().allocator_availability()
    }

    /// The fixed-view policy this recovery's recorded prefix ran under. The
    /// post-copy arm reads it from the reanalysis custody; the declined arm
    /// carries the policy whose probe declined.
    /// `None` on the direct legality, active-resident, and completed-suite
    /// paths.
    pub(crate) fn fixed_view_copy_policy(&self) -> Option<FixedViewCopyPolicy> {
        match self {
            Self::Legality(_)
            | Self::ActiveResidentRematerialization(_)
            | Self::SelectedLowering(_)
            | Self::PreAllocation(_) => None,
            Self::DeclinedFixedView { policy, .. } => Some(*policy),
            Self::FixedViewCopies(reanalysis) => Some(reanalysis.custody().source().policy()),
        }
    }

    /// The declared allocation-recovery selection this recovery's recorded
    /// prefix ran under: a shared-entry prefix binds the shared-entry
    /// selection and an active-resident sweep binds the rematerialization
    /// selection. Every other prefix is default-path recovery and binds
    /// none. Derived from the source custody, never asserted.
    pub(crate) fn recovery_prefix_selection(&self) -> Option<optimization_core::Optimization> {
        match self {
            Self::ActiveResidentRematerialization(_) => {
                Some(optimization_core::Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1)
            }
            _ => match self.fixed_view_copy_policy() {
                Some(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1) => {
                    Some(optimization_core::Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1)
                }
                _ => None,
            },
        }
    }

    /// The program the first recovery step consumes: the selected program on
    /// the direct and pre-copy declined paths, the fixed-view
    /// transformation's copy output, the active-resident
    /// rematerialization's transformed program, or the completed run's
    /// current program — its last committed step's output, or the unchanged
    /// admitted program when the terminal pass already proved the fixed
    /// point.
    pub(crate) fn base(&self) -> SelectedProgramRef<'_> {
        match self {
            Self::Legality(source) => SelectedProgramRef::new(source.selected()),
            Self::DeclinedFixedView { legality, .. } => {
                SelectedProgramRef::new(legality.selected())
            }
            Self::FixedViewCopies(reanalysis) => {
                SelectedProgramRef::new(reanalysis.transformation_stage().copies())
            }
            Self::ActiveResidentRematerialization(pressure) => {
                SelectedProgramRef::new(pressure.rematerialization())
            }
            Self::SelectedLowering(run) => match run.steps().last() {
                Some(step) => SelectedProgramRef::new(step.fold()),
                None => SelectedProgramRef::new(run.source_legality_stage().selected()),
            },
            Self::PreAllocation(run) => run.current(),
        }
    }

    /// Facts valid when recovery begins: the original legality facts on the
    /// direct and pre-copy declined paths, the completely reanalyzed facts
    /// after fixed-view copies, the rebuilt facts after the
    /// rematerialization sweep, or the completed run's final facts. No
    /// analysis result is carried across the rewrite boundary.
    pub(crate) fn liveness(&self) -> &ValidatedLiveness {
        match self {
            Self::Legality(source) => source.liveness(),
            Self::DeclinedFixedView { legality, .. } => legality.liveness(),
            Self::FixedViewCopies(reanalysis) => reanalysis.liveness(),
            Self::ActiveResidentRematerialization(pressure) => pressure.liveness(),
            Self::SelectedLowering(run) => match run.steps().last() {
                Some(step) => step.liveness(),
                None => run.source_legality_stage().liveness(),
            },
            Self::PreAllocation(run) => run.liveness(),
        }
    }

    pub(crate) fn ranges(&self) -> &ValidatedLiveRanges {
        match self {
            Self::Legality(source) => source.ranges(),
            Self::DeclinedFixedView { legality, .. } => legality.ranges(),
            Self::FixedViewCopies(reanalysis) => reanalysis.ranges(),
            Self::ActiveResidentRematerialization(pressure) => pressure.ranges(),
            Self::SelectedLowering(run) => match run.steps().last() {
                Some(step) => step.ranges(),
                None => run.source_legality_stage().ranges(),
            },
            Self::PreAllocation(run) => run.ranges(),
        }
    }

    pub(crate) fn legality(&self) -> &ValidatedAllocationLegality {
        match self {
            Self::Legality(source) => source.legality(),
            Self::DeclinedFixedView { legality, .. } => legality.legality(),
            Self::FixedViewCopies(reanalysis) => reanalysis.legality(),
            Self::ActiveResidentRematerialization(pressure) => pressure.legality(),
            Self::SelectedLowering(run) => match run.steps().last() {
                Some(step) => step.legality(),
                None => run.source_legality_stage().legality(),
            },
            Self::PreAllocation(run) => run.legality(),
        }
    }

    /// Project the declared prefix from the source's custody: completed suites
    /// contribute their receipts' ledgers and completion identities, and a
    /// declined probe its recorded policy.
    fn declared_manifest(&self) -> RuntimeSpillManifestBase {
        match self {
            Self::Legality(source)
            | Self::DeclinedFixedView {
                legality: source, ..
            } => RuntimeSpillManifestBase {
                pre_physical: source.custody().manifest(),
                selected_lowering_completion: None,
                pre_allocation_completion: None,
                prefix: Vec::new(),
            },
            Self::FixedViewCopies(reanalysis) => {
                let upstream = reanalysis.custody();
                RuntimeSpillManifestBase {
                    pre_physical: upstream.source().manifest(),
                    selected_lowering_completion: None,
                    pre_allocation_completion: None,
                    prefix: vec![PostAllocationSelectedTransformation::FixedViewCopy(
                        upstream.source().transformation(),
                    )],
                }
            }
            Self::ActiveResidentRematerialization(pressure) => {
                let upstream = pressure.custody();
                RuntimeSpillManifestBase {
                    pre_physical: upstream.source().manifest(),
                    selected_lowering_completion: None,
                    pre_allocation_completion: None,
                    prefix: vec![
                        PostAllocationSelectedTransformation::PressureRematerialization(
                            upstream.rematerialization(),
                        ),
                    ],
                }
            }
            Self::SelectedLowering(run) => {
                let upstream = run.custody();
                RuntimeSpillManifestBase {
                    pre_physical: upstream.source().manifest(),
                    selected_lowering_completion: Some(upstream.identity()),
                    pre_allocation_completion: None,
                    prefix: upstream
                        .iterations()
                        .iter()
                        .map(|iteration| {
                            PostAllocationSelectedTransformation::LiteralFold(iteration.fold())
                        })
                        .collect(),
                }
            }
            Self::PreAllocation(run) => {
                let upstream = run.custody();
                RuntimeSpillManifestBase {
                    pre_physical: upstream.source().manifest(),
                    selected_lowering_completion: None,
                    pre_allocation_completion: Some(upstream.identity()),
                    prefix: upstream
                        .iterations()
                        .iter()
                        .map(|iteration| iteration.transformation().into())
                        .collect(),
                }
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct RuntimeSpillStep {
    pub(crate) rewrite: RuntimeSpillStepRewrite,
}

/// The recorded recovery decision for one pressured value, cheapest first.
/// A split ends a fixed-view definition's pinned segment with one copy and
/// realizes no storage, so it is chosen whenever an overlapping candidate
/// admits it; rematerialization is chosen whenever its admission accepts the
/// victim's pure immediate definition; private storage is the fallback.
#[derive(Debug)]
pub(crate) enum RuntimeSpillStepRewrite {
    Split(ValidatedRuntimeSplit),
    Spill(ValidatedRuntimeSpill),
    Rematerialization(ValidatedRuntimeRematerialization),
}

impl RuntimeSpillStepRewrite {
    pub(crate) fn transformed(&self) -> &SelectedInstructionPlan {
        match self {
            Self::Split(rewrite) => rewrite.transformed(),
            Self::Spill(rewrite) => rewrite.transformed(),
            Self::Rematerialization(rewrite) => rewrite.transformed(),
        }
    }

    pub(crate) fn selected(&self) -> SelectedProgramRef<'_> {
        match self {
            Self::Split(rewrite) => SelectedProgramRef::new(rewrite),
            Self::Spill(rewrite) => SelectedProgramRef::new(rewrite),
            Self::Rematerialization(rewrite) => SelectedProgramRef::new(rewrite),
        }
    }
}

#[derive(Debug)]
pub(crate) struct RuntimeSpillFacts {
    pub(crate) liveness: ValidatedLiveness,
    pub(crate) ranges: ValidatedLiveRanges,
    pub(crate) legality: ValidatedAllocationLegality,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSpillAllocationError {
    Rewrite(RuntimeSpillError),
    Rematerialization(RuntimeRematerializationError),
    Split(RuntimeSplitError),
    Liveness(LivenessError),
    Ranges(LiveRangeError),
    Legality(AllocationLegalityError),
    Homes(crate::RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    RecoveryNotRequired,
}

impl std::fmt::Display for RuntimeSpillAllocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "runtime spill allocation failed: {self:?}")
    }
}

impl std::error::Error for RuntimeSpillAllocationError {}
