use crate::{
    StagedOptimizedAllocationLegality, StagedOptimizedSelectedReanalysis,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
    ValidatedRuntimeRematerialization, ValidatedRuntimeSpill,
};
use selected_instructions::{SelectedInstructionPlan, VirtualRegisterId};

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
    /// fixed-view policy this recovery's recorded prefix ran under.
    pub(crate) fn fixed_view_copy_policy(&self) -> Option<crate::FixedViewCopyPolicy> {
        self.source.fixed_view_copy_policy()
    }
}

/// The staged custody a spill recovery began from. The direct route enters
/// with raw legality; a fixed-view sequence that still faces pressure enters
/// with its complete post-copy reanalysis, and the copy transformation stays
/// recorded in the manifest prefix both producer and replay reconstruct.
#[derive(Debug)]
pub(crate) enum RuntimeSpillSource {
    Legality(StagedOptimizedAllocationLegality),
    FixedViewCopies(StagedOptimizedSelectedReanalysis),
}

impl RuntimeSpillSource {
    /// Every arm shares one underlying legality stage: the fixed-view entry
    /// reaches it through the copy transformation's source custody.
    fn legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        match self {
            Self::Legality(source) => source,
            Self::FixedViewCopies(reanalysis) => {
                reanalysis.transformation_stage().source_legality_stage()
            }
        }
    }

    /// The register environment is an upstream fact both arms share through
    /// the underlying legality stage.
    pub(crate) fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment()
    }

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

    pub(crate) fn optimized_target(
        &self,
    ) -> &abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations {
        self.legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target()
    }

    pub(crate) fn allocator_availability(&self) -> &crate::ValidatedAllocatorAvailability {
        self.legality_stage().allocator_availability()
    }

    /// The fixed-view policy the recorded prefix ran under when recovery
    /// entered through a post-copy reanalysis; replayed custody evidence, not
    /// a producer assertion. `None` on the direct legality path.
    pub(crate) fn fixed_view_copy_policy(&self) -> Option<crate::FixedViewCopyPolicy> {
        match self {
            Self::Legality(_) => None,
            Self::FixedViewCopies(reanalysis) => Some(reanalysis.custody().source().policy()),
        }
    }

    /// The program the first recovery step consumes: the selected program on
    /// the direct path, or the fixed-view transformation's copy output.
    pub(crate) fn base(&self) -> crate::SelectedProgramRef<'_> {
        match self {
            Self::Legality(source) => crate::SelectedProgramRef::new(
                source
                    .live_range_stage()
                    .liveness_stage()
                    .selected_stage()
                    .selected(),
            ),
            Self::FixedViewCopies(reanalysis) => {
                crate::SelectedProgramRef::new(reanalysis.transformation_stage().copies())
            }
        }
    }

    /// Facts valid when recovery begins: the original legality facts on the
    /// direct path, or the completely reanalyzed facts after fixed-view
    /// copies. No analysis result is carried across the rewrite boundary.
    pub(crate) fn liveness(&self) -> &ValidatedLiveness {
        match self {
            Self::Legality(source) => source.live_range_stage().liveness_stage().liveness(),
            Self::FixedViewCopies(reanalysis) => reanalysis.liveness(),
        }
    }

    pub(crate) fn ranges(&self) -> &ValidatedLiveRanges {
        match self {
            Self::Legality(source) => source.live_range_stage().ranges(),
            Self::FixedViewCopies(reanalysis) => reanalysis.ranges(),
        }
    }

    pub(crate) fn legality(&self) -> &ValidatedAllocationLegality {
        match self {
            Self::Legality(source) => source.legality(),
            Self::FixedViewCopies(reanalysis) => reanalysis.legality(),
        }
    }

    /// Independently revalidate the source custody chain, returning the
    /// pre-physical manifest base plus the transformation prefix every
    /// manifest this recovery produces must carry. The fixed-view arm keeps
    /// its copy transformation first so the recorded ledger replays the
    /// actual rewrite order.
    pub(crate) fn upstream_manifest(
        &self,
    ) -> Result<
        (
            optimization_core::PrePhysicalOptimizationManifestIdentity,
            Vec<crate::PostAllocationSelectedTransformation>,
        ),
        RuntimeSpillAllocationError,
    > {
        match self {
            Self::Legality(source) => {
                let upstream = crate::validate_optimized_allocation_legality_custody(
                    source.live_range_stage(),
                    source.allocator_availability(),
                    source.legality(),
                )
                .map_err(RuntimeSpillAllocationError::Upstream)?;
                Ok((upstream.manifest(), Vec::new()))
            }
            Self::FixedViewCopies(reanalysis) => {
                let upstream = crate::validate_optimized_selected_reanalysis_custody(
                    reanalysis.transformation_stage(),
                    reanalysis.liveness(),
                    reanalysis.ranges(),
                    reanalysis.legality(),
                )
                .map_err(RuntimeSpillAllocationError::UpstreamReanalysis)?;
                Ok((
                    upstream.source().manifest(),
                    vec![crate::PostAllocationSelectedTransformation::FixedViewCopy(
                        upstream.source().transformation(),
                    )],
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

    pub(crate) fn selected(&self) -> crate::SelectedProgramRef<'_> {
        match self {
            Self::Spill(rewrite) => crate::SelectedProgramRef::new(rewrite),
            Self::Rematerialization(rewrite) => crate::SelectedProgramRef::new(rewrite),
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
    Upstream(crate::OptimizedAllocationLegalityCustodyError),
    UpstreamReanalysis(crate::OptimizedSelectedReanalysisError),
    Rewrite(crate::RuntimeSpillError),
    Rematerialization(crate::RuntimeRematerializationError),
    Liveness(crate::LivenessError),
    Ranges(crate::LiveRangeError),
    Legality(crate::AllocationLegalityError),
    Homes(crate::RegisterHomeError),
    Manifest(crate::PostAllocationOptimizationManifestError),
    RecoveryNotRequired,
    CandidateMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for RuntimeSpillAllocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "runtime spill allocation failed: {self:?}")
    }
}

impl std::error::Error for RuntimeSpillAllocationError {}
