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
    AllocationLegalityError, FixedOccurrencesError, FixedViewCopyPolicy, LiveRangeError,
    LivenessError, RuntimeRematerializationError, RuntimeSpillError, RuntimeSplitError,
    SelectedProgramRef, StagedOptimizedAllocationLegality, StagedOptimizedSelectedReanalysis,
    StagedPreAllocationOptimizationRun, StagedSelectedLoweringOptimizationRun,
    ValidatedAllocationLegality, ValidatedFixedOccurrences, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedRuntimeRematerialization, ValidatedRuntimeSpill, ValidatedRuntimeSplit,
};

/// A completed recovery: its source custody, the final step and the final
/// facts, homes and manifest. Every recovery caller transfers it directly to
/// `RetainedAllocation::try_from`, which publishes the final step's program
/// and facts. Earlier steps survive only as the manifest's transformation
/// records: each step's program replaces the one before it, so keeping every
/// intermediate program held one copy of the rewritten function per step.
#[derive(Debug)]
pub(crate) struct RuntimeSpillAllocation {
    pub(crate) source: RuntimeSpillSource,
    pub(crate) last: RuntimeSpillStep,
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
/// `NoCompatibleHome` or value-local `NoCommonCandidate` enters with that proven prefix:
/// the rematerialization
/// stays recorded ahead of every spill step. Completed rewrite suites enter
/// the same way: a selected-lowering or pre-allocation run whose final facts
/// still report pressure keeps its committed ledger and suite completion
/// identity as the recorded prefix.
///
/// Every variant is one answer to the same question — which custody arrived —
/// and [`RuntimeSpillSource::resolve`] is the one place that reads it.
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

/// The prefix's current facts, resolved once from the staged custody recovery
/// was handed: the shared legality stage every source reaches, the program and
/// the validated analyses the first step reuses, and the manifest prefix the
/// produced manifest records ahead of every spill step. One resolution per
/// recovery, so no consumer re-derives which source variant arrived.
pub(crate) struct ResolvedSpillSource<'source> {
    pub(crate) legality_stage: &'source StagedOptimizedAllocationLegality,
    /// The program the first recovery step consumes.
    pub(crate) base: SelectedProgramRef<'source>,
    /// The validated liveness, ranges, and legality over `base`.
    pub(crate) liveness: &'source ValidatedLiveness,
    pub(crate) ranges: &'source ValidatedLiveRanges,
    pub(crate) legality: &'source ValidatedAllocationLegality,
    pub(crate) manifest: RuntimeSpillManifestBase,
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
            Self::DeclinedFixedView { policy, .. } => declared_shared_entry_selection(*policy),
            Self::FixedViewCopies(reanalysis) => {
                declared_shared_entry_selection(reanalysis.custody().source().policy())
            }
            Self::Legality(_) | Self::SelectedLowering(_) | Self::PreAllocation(_) => None,
        }
    }

    /// Resolve the prefix's facts once, in one pass over the source family:
    /// the underlying legality stage, the program the first recovery step
    /// consumes with its validated analyses, and the declared manifest base.
    /// The direct and pre-copy declined paths carry the admitted legality
    /// itself; a post-copy reanalysis carries the copy transformation's
    /// source custody and its own reanalyzed facts; the active-resident
    /// prefix carries the sweep's rebuilt program and facts; a completed
    /// suite carries its last committed step's program and facts, or the
    /// unchanged admitted program when the terminal pass already proved the
    /// fixed point.
    pub(crate) fn resolve(&self) -> ResolvedSpillSource<'_> {
        match self {
            Self::Legality(legality) | Self::DeclinedFixedView { legality, .. } => {
                ResolvedSpillSource {
                    legality_stage: legality,
                    base: SelectedProgramRef::new(legality.selected()),
                    liveness: legality.liveness(),
                    ranges: legality.ranges(),
                    legality: legality.legality(),
                    manifest: RuntimeSpillManifestBase {
                        pre_physical: legality.custody().manifest(),
                        selected_lowering_completion: None,
                        pre_allocation_completion: None,
                        prefix: Vec::new(),
                    },
                }
            }
            Self::FixedViewCopies(reanalysis) => {
                let upstream = reanalysis.custody();
                ResolvedSpillSource {
                    legality_stage: reanalysis.transformation_stage().source_legality_stage(),
                    base: SelectedProgramRef::new(reanalysis.transformation_stage().copies()),
                    liveness: reanalysis.liveness(),
                    ranges: reanalysis.ranges(),
                    legality: reanalysis.legality(),
                    manifest: RuntimeSpillManifestBase {
                        pre_physical: upstream.source().manifest(),
                        selected_lowering_completion: None,
                        pre_allocation_completion: None,
                        prefix: vec![PostAllocationSelectedTransformation::FixedViewCopy(
                            upstream.source().transformation(),
                        )],
                    },
                }
            }
            Self::ActiveResidentRematerialization(pressure) => {
                let upstream = pressure.custody();
                ResolvedSpillSource {
                    legality_stage: pressure.source(),
                    base: SelectedProgramRef::new(pressure.rematerialization()),
                    liveness: pressure.liveness(),
                    ranges: pressure.ranges(),
                    legality: pressure.legality(),
                    manifest: RuntimeSpillManifestBase {
                        pre_physical: upstream.source().manifest(),
                        selected_lowering_completion: None,
                        pre_allocation_completion: None,
                        prefix: vec![
                            PostAllocationSelectedTransformation::PressureRematerialization(
                                upstream.rematerialization(),
                            ),
                        ],
                    },
                }
            }
            Self::SelectedLowering(run) => {
                let upstream = run.custody();
                let (base, liveness, ranges, legality) = match run.steps().last() {
                    Some(step) => (
                        SelectedProgramRef::new(step.fold()),
                        step.liveness(),
                        step.ranges(),
                        step.legality(),
                    ),
                    None => {
                        let source = run.source_legality_stage();
                        (
                            SelectedProgramRef::new(source.selected()),
                            source.liveness(),
                            source.ranges(),
                            source.legality(),
                        )
                    }
                };
                ResolvedSpillSource {
                    legality_stage: run.source_legality_stage(),
                    base,
                    liveness,
                    ranges,
                    legality,
                    manifest: RuntimeSpillManifestBase {
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
                    },
                }
            }
            Self::PreAllocation(run) => {
                let upstream = run.custody();
                ResolvedSpillSource {
                    legality_stage: run.source_legality_stage(),
                    base: run.current(),
                    liveness: run.liveness(),
                    ranges: run.ranges(),
                    legality: run.legality(),
                    manifest: RuntimeSpillManifestBase {
                        pre_physical: upstream.source().manifest(),
                        selected_lowering_completion: None,
                        pre_allocation_completion: Some(upstream.identity()),
                        prefix: upstream
                            .iterations()
                            .iter()
                            .map(|iteration| iteration.transformation().into())
                            .collect(),
                    },
                }
            }
        }
    }
}

/// The declared shared-entry selection a copy policy binds; the leaf-local,
/// immediate-site, and shared-source-exit policies are default-path recovery
/// and bind none.
fn declared_shared_entry_selection(
    policy: FixedViewCopyPolicy,
) -> Option<optimization_core::Optimization> {
    (policy == FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1).then_some(
        optimization_core::Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
    )
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
#[derive(Debug, Clone)]
pub(crate) enum RuntimeSpillStepRewrite {
    FixedOccurrences(ValidatedFixedOccurrences),
    Split(ValidatedRuntimeSplit),
    Spill(ValidatedRuntimeSpill),
    Rematerialization(ValidatedRuntimeRematerialization),
}

impl RuntimeSpillStepRewrite {
    /// The manifest record of this step: its family and transformed program.
    pub(crate) fn transformation(&self) -> PostAllocationSelectedTransformation {
        match self {
            Self::FixedOccurrences(rewrite) => {
                PostAllocationSelectedTransformation::FixedOccurrences(
                    rewrite.receipt().transformed_selected(),
                )
            }
            Self::Split(rewrite) => PostAllocationSelectedTransformation::RuntimeSplit(
                rewrite.receipt().transformed_selected(),
            ),
            Self::Spill(rewrite) => PostAllocationSelectedTransformation::RuntimeSpill(
                rewrite.receipt().transformed_selected(),
            ),
            Self::Rematerialization(rewrite) => {
                PostAllocationSelectedTransformation::RuntimeRematerialization(
                    rewrite.receipt().transformed_selected(),
                )
            }
        }
    }

    pub(crate) fn transformed(&self) -> &SelectedInstructionPlan {
        match self {
            Self::FixedOccurrences(rewrite) => rewrite.transformed(),
            Self::Split(rewrite) => rewrite.transformed(),
            Self::Spill(rewrite) => rewrite.transformed(),
            Self::Rematerialization(rewrite) => rewrite.transformed(),
        }
    }

    pub(crate) fn selected(&self) -> SelectedProgramRef<'_> {
        match self {
            Self::FixedOccurrences(rewrite) => SelectedProgramRef::new(rewrite),
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
    FixedOccurrences(FixedOccurrencesError),
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
