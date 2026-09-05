use super::{AllocationOutput, AllocationReplayError, AllocationSource, sealed};
use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

/// Current allocated program and a separate replay-only evidence graph.
/// Ordinary reads never traverse the evidence or select a program by history.
#[derive(Debug)]
pub struct RetainedAllocation {
    current: super::current::CurrentAllocation,
    replay: ReplayInputs,
}

#[derive(Debug)]
enum ReplayInputs {
    Baseline(Box<StagedOptimizedRegisterHomes>),
    FixedView(Box<StagedOptimizedRegisterHomesAfterFixedViewCopies>),
    LiteralFolds(Box<StagedOptimizedRegisterHomesAfterLiteralFolds>),
    SelectedLowering(Box<StagedOptimizedRegisterHomesAfterSelectedLowering>),
    Rematerialization(Box<StagedOptimizedActiveResidentRematerialization>),
}

impl RetainedAllocation {
    #[cfg(feature = "test-support")]
    pub fn substitute_current_program_for_test(
        &mut self,
        program: omega_register_homes::AllocatedProgram,
    ) {
        self.current.program = program;
    }

    #[cfg(feature = "test-support")]
    pub fn fixed_view_copy_proof_for_test(&self) -> Option<&crate::ValidatedFixedViewCopies> {
        match &self.replay {
            ReplayInputs::FixedView(source) => {
                Some(source.reanalysis_stage().transformation_stage().copies())
            }
            _ => None,
        }
    }

    #[cfg(feature = "test-support")]
    pub fn rematerialization_availability_for_test(
        &self,
    ) -> Option<&crate::ValidatedAllocatorAvailability> {
        match &self.replay {
            ReplayInputs::Rematerialization(source) => {
                Some(source.source().allocator_availability())
            }
            _ => None,
        }
    }

    /// Inspect exact rewrite proof details in cross-phase corruption controls.
    /// Production consumers cannot use this to select a source-history route.
    #[cfg(feature = "test-support")]
    pub fn rematerialization_proof_for_test(
        &self,
    ) -> Option<&crate::ValidatedPressureRematerialization> {
        match &self.replay {
            ReplayInputs::Rematerialization(source) => Some(source.rematerialization()),
            _ => None,
        }
    }

    pub fn program(&self) -> &omega_register_homes::AllocatedProgram {
        &self.current.program
    }

    pub fn current(&self) -> AllocationOutput<'_> {
        self.current.view()
    }
}

impl sealed::Sealed for RetainedAllocation {}

impl AllocationSource for RetainedAllocation {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        let current = match &self.replay {
            ReplayInputs::Baseline(source) => source.replay_allocation(),
            ReplayInputs::FixedView(source) => source.replay_allocation(),
            ReplayInputs::LiteralFolds(source) => source.replay_allocation(),
            ReplayInputs::SelectedLowering(source) => source.replay_allocation(),
            ReplayInputs::Rematerialization(source) => source.replay_allocation(),
        }?;
        validate_recovery_selection(&current)?;
        self.current.validate_against(&current)?;
        Ok(self.current())
    }
}

impl TryFrom<StagedOptimizedRegisterHomes> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(source: StagedOptimizedRegisterHomes) -> Result<Self, Self::Error> {
        let replayed = source.replay_allocation()?;
        validate_recovery_selection(&replayed)?;
        let current = super::current::CurrentAllocation::from_replayed(&replayed);
        Ok(Self {
            current,
            replay: ReplayInputs::Baseline(Box::new(source)),
        })
    }
}

impl TryFrom<StagedOptimizedRegisterHomesAfterFixedViewCopies> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(
        source: StagedOptimizedRegisterHomesAfterFixedViewCopies,
    ) -> Result<Self, Self::Error> {
        let replayed = source.replay_allocation()?;
        validate_recovery_selection(&replayed)?;
        let current = super::current::CurrentAllocation::from_replayed(&replayed);
        Ok(Self {
            current,
            replay: ReplayInputs::FixedView(Box::new(source)),
        })
    }
}

impl TryFrom<StagedOptimizedRegisterHomesAfterLiteralFolds> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(
        source: StagedOptimizedRegisterHomesAfterLiteralFolds,
    ) -> Result<Self, Self::Error> {
        let replayed = source.replay_allocation()?;
        validate_recovery_selection(&replayed)?;
        let current = super::current::CurrentAllocation::from_replayed(&replayed);
        Ok(Self {
            current,
            replay: ReplayInputs::LiteralFolds(Box::new(source)),
        })
    }
}

impl TryFrom<StagedOptimizedRegisterHomesAfterSelectedLowering> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(
        source: StagedOptimizedRegisterHomesAfterSelectedLowering,
    ) -> Result<Self, Self::Error> {
        let replayed = source.replay_allocation()?;
        validate_recovery_selection(&replayed)?;
        let current = super::current::CurrentAllocation::from_replayed(&replayed);
        Ok(Self {
            current,
            replay: ReplayInputs::SelectedLowering(Box::new(source)),
        })
    }
}

impl TryFrom<StagedOptimizedActiveResidentRematerialization> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(
        source: StagedOptimizedActiveResidentRematerialization,
    ) -> Result<Self, Self::Error> {
        let replayed = source.replay_allocation()?;
        validate_recovery_selection(&replayed)?;
        let current = super::current::CurrentAllocation::from_replayed(&replayed);
        Ok(Self {
            current,
            replay: ReplayInputs::Rematerialization(Box::new(source)),
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
