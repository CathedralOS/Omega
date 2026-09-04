use super::{AllocationOutput, AllocationReplayError, AllocationSource, ProjectAllocation, sealed};
use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

/// Owned allocation inputs admitted by independent replay. The history is
/// private and immutable; consumers read current facts without replaying the
/// analysis on every accessor. Full replay remains available at proof gates.
#[derive(Debug)]
pub struct RetainedAllocation {
    history: History,
}

#[derive(Debug)]
enum History {
    Baseline(Box<StagedOptimizedRegisterHomes>),
    FixedView(Box<StagedOptimizedRegisterHomesAfterFixedViewCopies>),
    LiteralFolds(Box<StagedOptimizedRegisterHomesAfterLiteralFolds>),
    SelectedLowering(Box<StagedOptimizedRegisterHomesAfterSelectedLowering>),
    Rematerialization(Box<StagedOptimizedActiveResidentRematerialization>),
}

impl RetainedAllocation {
    #[cfg(feature = "test-support")]
    pub fn fixed_view_copy_proof_for_test(
        &self,
    ) -> Option<&omega_regalloc::ValidatedFixedViewCopies> {
        match &self.history {
            History::FixedView(source) => {
                Some(source.reanalysis_stage().transformation_stage().copies())
            }
            _ => None,
        }
    }

    #[cfg(feature = "test-support")]
    pub fn rematerialization_availability_for_test(
        &self,
    ) -> Option<&omega_regalloc::ValidatedAllocatorAvailability> {
        match &self.history {
            History::Rematerialization(source) => Some(source.source().allocator_availability()),
            _ => None,
        }
    }

    /// Inspect exact rewrite proof details in cross-phase corruption controls.
    /// Production consumers cannot use this to select a source-history route.
    #[cfg(feature = "test-support")]
    pub fn rematerialization_proof_for_test(
        &self,
    ) -> Option<&omega_regalloc::ValidatedPressureRematerialization> {
        match &self.history {
            History::Rematerialization(source) => Some(source.rematerialization()),
            _ => None,
        }
    }

    pub fn current(&self) -> AllocationOutput<'_> {
        match &self.history {
            History::Baseline(source) => source.project_allocation(),
            History::FixedView(source) => source.project_allocation(),
            History::LiteralFolds(source) => source.project_allocation(),
            History::SelectedLowering(source) => source.project_allocation(),
            History::Rematerialization(source) => source.project_allocation(),
        }
    }
}

impl sealed::Sealed for RetainedAllocation {}

impl AllocationSource for RetainedAllocation {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        let current = match &self.history {
            History::Baseline(source) => source.replay_allocation(),
            History::FixedView(source) => source.replay_allocation(),
            History::LiteralFolds(source) => source.replay_allocation(),
            History::SelectedLowering(source) => source.replay_allocation(),
            History::Rematerialization(source) => source.replay_allocation(),
        }?;
        validate_recovery_selection(&current)?;
        Ok(current)
    }
}

impl TryFrom<StagedOptimizedRegisterHomes> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(source: StagedOptimizedRegisterHomes) -> Result<Self, Self::Error> {
        validate_recovery_selection(&source.replay_allocation()?)?;
        Ok(Self {
            history: History::Baseline(Box::new(source)),
        })
    }
}

impl TryFrom<StagedOptimizedRegisterHomesAfterFixedViewCopies> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(
        source: StagedOptimizedRegisterHomesAfterFixedViewCopies,
    ) -> Result<Self, Self::Error> {
        validate_recovery_selection(&source.replay_allocation()?)?;
        Ok(Self {
            history: History::FixedView(Box::new(source)),
        })
    }
}

impl TryFrom<StagedOptimizedRegisterHomesAfterLiteralFolds> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(
        source: StagedOptimizedRegisterHomesAfterLiteralFolds,
    ) -> Result<Self, Self::Error> {
        validate_recovery_selection(&source.replay_allocation()?)?;
        Ok(Self {
            history: History::LiteralFolds(Box::new(source)),
        })
    }
}

impl TryFrom<StagedOptimizedRegisterHomesAfterSelectedLowering> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(
        source: StagedOptimizedRegisterHomesAfterSelectedLowering,
    ) -> Result<Self, Self::Error> {
        validate_recovery_selection(&source.replay_allocation()?)?;
        Ok(Self {
            history: History::SelectedLowering(Box::new(source)),
        })
    }
}

impl TryFrom<StagedOptimizedActiveResidentRematerialization> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(
        source: StagedOptimizedActiveResidentRematerialization,
    ) -> Result<Self, Self::Error> {
        validate_recovery_selection(&source.replay_allocation()?)?;
        Ok(Self {
            history: History::Rematerialization(Box::new(source)),
        })
    }
}

// Completion belongs to allocation, not to a layout adapter. Reconstruct the
// exercised recovery selection from the replayed evidence, then compare it
// with the retained build policy; copying policy into a manifest is not proof.
fn validate_recovery_selection(
    current: &AllocationOutput<'_>,
) -> Result<(), AllocationReplayError> {
    use super::AllocationEvidence;
    use omega_optimization_core::{Optimization, OptimizationExecutionPhase};
    let expected: &[Optimization] = match current.evidence() {
        AllocationEvidence::FixedViewCopies(_) => {
            &[Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1]
        }
        AllocationEvidence::ActiveResidentRematerialization(_) => {
            &[Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1]
        }
        AllocationEvidence::RegisterHomes(_)
        | AllocationEvidence::LiteralFolds(_)
        | AllocationEvidence::SelectedLowering(_) => &[],
    };
    if current
        .selections()
        .for_phase(OptimizationExecutionPhase::AllocationRecovery)
        .as_slice()
        != expected
    {
        return Err(AllocationReplayError::SelectionMismatch);
    }
    Ok(())
}
