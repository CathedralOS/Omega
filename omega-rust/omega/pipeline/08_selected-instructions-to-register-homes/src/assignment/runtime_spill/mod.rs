//! Optimizer module role: stage group. Executable spill recovery and its independent replay.

mod recovery;
pub(crate) mod replay;

use crate::{ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes};
pub(crate) use recovery::{
    assign_source, recover, recover_after_active_resident_rematerialization,
    recover_after_declined_fixed_view_probe, recover_after_fixed_view_copies,
};
use register_homes::{
    PostAllocationOptimizationManifestError, PostAllocationSelectedTransformation,
};
use selected_instructions::{SelectedInstructionPlan, VirtualRegisterId};
use selected_instructions_to_selected_instructions::{
    AllocationLegalityError, FixedPrecoloredSegmentHomeDecline, FixedViewCopyPolicy,
    LiveRangeError, LivenessError, OptimizedAllocationLegalityCustodyError,
    OptimizedSelectedReanalysisError, RuntimeRematerializationError, RuntimeSpillError,
    SelectedProgramRef, StagedOptimizedAllocationLegality, StagedOptimizedSelectedReanalysis,
    ValidatedAllocationLegality, ValidatedAllocatorAvailability, ValidatedLiveRanges,
    ValidatedLiveness, ValidatedRuntimeRematerialization, ValidatedRuntimeSpill,
    probe_optimized_fixed_precolored_segment_homes, validate_optimized_allocation_legality_custody,
    validate_optimized_selected_reanalysis_custody,
};

/// Original source custody and rewrite evidence are retained only for replay.
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

    /// Corrupt the recorded active-resident prefix custody so cross-phase
    /// controls can prove replay rejects it before trusting any spill step.
    /// Returns `false` when this recovery has no such prefix.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub(crate) fn corrupt_active_resident_prefix_custody_for_test(&mut self) -> bool {
        let RuntimeSpillSource::ActiveResidentRematerialization(pressure) = &mut self.source else {
            return false;
        };
        crate::rewrites::corrupt_active_resident_rematerialization_pressure_custody_for_test(
            pressure,
        );
        true
    }
}

/// The staged custody a spill recovery began from. The direct route enters
/// with raw legality; a fixed-view sequence that still faces pressure enters
/// with its complete post-copy reanalysis, and the copy transformation stays
/// recorded in the manifest prefix both producer and replay reconstruct. A
/// declared sequence whose segment-home probe reported a capacity decline
/// before custody was consumed enters with the same legality plus the
/// declined policy and verdict — the probe outcome is re-derived on every
/// replay so the recorded selection binding is custody evidence, not a
/// producer assertion. An active-resident rematerialization sweep whose
/// rebuilt facts still report `NoCompatibleHome` enters with that proven
/// prefix: the rematerialization stays recorded ahead of every spill step
/// and the whole prefix is replayed before any step is trusted.
#[derive(Debug)]
pub(crate) enum RuntimeSpillSource {
    Legality(StagedOptimizedAllocationLegality),
    DeclinedFixedView {
        legality: StagedOptimizedAllocationLegality,
        policy: FixedViewCopyPolicy,
        decline: FixedPrecoloredSegmentHomeDecline,
    },
    FixedViewCopies(StagedOptimizedSelectedReanalysis),
    ActiveResidentRematerialization(crate::StagedOptimizedActiveResidentRematerializationPressure),
}

impl RuntimeSpillSource {
    /// Every arm shares one underlying legality stage: the fixed-view entries
    /// reach it through the copy transformation's source custody or carry it
    /// directly when the sequence was declined before committing, and the
    /// active-resident entry reaches it as the rematerialization's own
    /// staged source.
    fn legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        match self {
            Self::Legality(source) => source,
            Self::DeclinedFixedView { legality, .. } => legality,
            Self::FixedViewCopies(reanalysis) => {
                reanalysis.transformation_stage().source_legality_stage()
            }
            Self::ActiveResidentRematerialization(pressure) => pressure.source(),
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

    /// The retained optimized-target proof input, kept as replay evidence;
    /// custody checks downstream compare the owner handle by identity.
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

    /// The fixed-view policy this recovery's recorded prefix ran under:
    /// replayed custody evidence, not a producer assertion. The post-copy
    /// arm reads it from the validated reanalysis custody; the declined arm
    /// carries it from the probe the route re-proves on every replay.
    /// `None` on the direct legality and active-resident paths.
    pub(crate) fn fixed_view_copy_policy(&self) -> Option<FixedViewCopyPolicy> {
        match self {
            Self::Legality(_) | Self::ActiveResidentRematerialization(_) => None,
            Self::DeclinedFixedView { policy, .. } => Some(*policy),
            Self::FixedViewCopies(reanalysis) => Some(reanalysis.custody().source().policy()),
        }
    }

    /// The declared allocation-recovery selection this recovery's recorded
    /// prefix ran under: a shared-entry prefix binds the shared-entry
    /// selection and an active-resident sweep binds the rematerialization
    /// selection. Every other prefix is default-path recovery and binds
    /// none. Derived from replayed custody evidence, never asserted.
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
    /// transformation's copy output, or the active-resident
    /// rematerialization's transformed program.
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
        }
    }

    /// Facts valid when recovery begins: the original legality facts on the
    /// direct and pre-copy declined paths, the completely reanalyzed facts
    /// after fixed-view copies, or the rebuilt facts after the
    /// rematerialization sweep. No analysis result is carried across the
    /// rewrite boundary.
    pub(crate) fn liveness(&self) -> &ValidatedLiveness {
        match self {
            Self::Legality(source) => source.liveness(),
            Self::DeclinedFixedView { legality, .. } => legality.liveness(),
            Self::FixedViewCopies(reanalysis) => reanalysis.liveness(),
            Self::ActiveResidentRematerialization(pressure) => pressure.liveness(),
        }
    }

    pub(crate) fn ranges(&self) -> &ValidatedLiveRanges {
        match self {
            Self::Legality(source) => source.ranges(),
            Self::DeclinedFixedView { legality, .. } => legality.ranges(),
            Self::FixedViewCopies(reanalysis) => reanalysis.ranges(),
            Self::ActiveResidentRematerialization(pressure) => pressure.ranges(),
        }
    }

    pub(crate) fn legality(&self) -> &ValidatedAllocationLegality {
        match self {
            Self::Legality(source) => source.legality(),
            Self::DeclinedFixedView { legality, .. } => legality.legality(),
            Self::FixedViewCopies(reanalysis) => reanalysis.legality(),
            Self::ActiveResidentRematerialization(pressure) => pressure.legality(),
        }
    }

    /// Independently revalidate the source custody chain, returning the
    /// pre-physical manifest base plus the transformation prefix every
    /// manifest this recovery produces must carry. The fixed-view arm keeps
    /// its copy transformation first so the recorded ledger replays the
    /// actual rewrite order; the declined arm re-runs the segment-home probe
    /// and requires the same capacity verdict, so a recorded policy was
    /// earned by the front-end's own rejection. The active-resident arm
    /// replays the proven rematerialization prefix and keeps its sweep
    /// transformation first for the same reason.
    pub(crate) fn upstream_manifest(
        &self,
    ) -> Result<
        (
            optimization_core::PrePhysicalOptimizationManifestIdentity,
            Vec<PostAllocationSelectedTransformation>,
        ),
        RuntimeSpillAllocationError,
    > {
        match self {
            Self::Legality(source) => {
                let upstream = validate_optimized_allocation_legality_custody(
                    source.live_range_stage(),
                    source.allocator_availability(),
                    source.legality(),
                )
                .map_err(RuntimeSpillAllocationError::Upstream)?;
                Ok((upstream.manifest(), Vec::new()))
            }
            Self::DeclinedFixedView {
                legality, decline, ..
            } => {
                let upstream = validate_optimized_allocation_legality_custody(
                    legality.live_range_stage(),
                    legality.allocator_availability(),
                    legality.legality(),
                )
                .map_err(RuntimeSpillAllocationError::Upstream)?;
                let budget = legality.budget_per_pass();
                match probe_optimized_fixed_precolored_segment_homes(legality, budget) {
                    Err(error) if error.capacity_decline() == Some(*decline) => {
                        Ok((upstream.manifest(), Vec::new()))
                    }
                    _ => Err(RuntimeSpillAllocationError::ProbeMismatch),
                }
            }
            Self::FixedViewCopies(reanalysis) => {
                let upstream = validate_optimized_selected_reanalysis_custody(
                    reanalysis.transformation_stage(),
                    reanalysis.liveness(),
                    reanalysis.ranges(),
                    reanalysis.legality(),
                )
                .map_err(RuntimeSpillAllocationError::UpstreamReanalysis)?;
                Ok((
                    upstream.source().manifest(),
                    vec![PostAllocationSelectedTransformation::FixedViewCopy(
                        upstream.source().transformation(),
                    )],
                ))
            }
            Self::ActiveResidentRematerialization(pressure) => {
                let upstream =
                    crate::validate_optimized_active_resident_rematerialization_pressure(pressure)
                        .map_err(RuntimeSpillAllocationError::UpstreamRematerialization)?;
                Ok((
                    upstream.source().manifest(),
                    vec![
                        PostAllocationSelectedTransformation::PressureRematerialization(
                            upstream.rematerialization(),
                        ),
                    ],
                ))
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct RuntimeSpillStep {
    pub(crate) function: usize,
    pub(crate) register: VirtualRegisterId,
    pub(crate) rewrite: RuntimeSpillStepRewrite,
}

/// The recorded recovery decision for one pressured value. Rematerialization
/// is the cheaper form and is chosen whenever its admission accepts the
/// victim's pure immediate definition; private storage is the fallback.
#[derive(Debug)]
pub(crate) enum RuntimeSpillStepRewrite {
    Spill(ValidatedRuntimeSpill),
    Rematerialization(ValidatedRuntimeRematerialization),
}

impl RuntimeSpillStepRewrite {
    pub(crate) fn transformed(&self) -> &SelectedInstructionPlan {
        match self {
            Self::Spill(rewrite) => rewrite.transformed(),
            Self::Rematerialization(rewrite) => rewrite.transformed(),
        }
    }

    pub(crate) fn selected(&self) -> SelectedProgramRef<'_> {
        match self {
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
    Upstream(OptimizedAllocationLegalityCustodyError),
    UpstreamReanalysis(OptimizedSelectedReanalysisError),
    /// The recorded active-resident prefix could not be independently
    /// replayed — the rematerialization custody the recovery claims was
    /// never proven.
    UpstreamRematerialization(crate::OptimizedActiveResidentRematerializationError),
    Rewrite(RuntimeSpillError),
    Rematerialization(RuntimeRematerializationError),
    Liveness(LivenessError),
    Ranges(LiveRangeError),
    Legality(AllocationLegalityError),
    Homes(crate::RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    RecoveryNotRequired,
    /// A declined fixed-view source could not re-prove the capacity verdict
    /// its recorded policy claims — the probe succeeded, declined on a
    /// different verdict, or reported a non-decline failure, so the
    /// selection binding was never earned.
    ProbeMismatch,
    CandidateMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for RuntimeSpillAllocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "runtime spill allocation failed: {self:?}")
    }
}

impl std::error::Error for RuntimeSpillAllocationError {}
