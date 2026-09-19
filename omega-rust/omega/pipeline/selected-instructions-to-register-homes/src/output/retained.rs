use super::{AllocationOutput, AllocationReplayError, AllocationSource, ProjectAllocation, sealed};
use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

#[cfg(test)]
mod tests;

/// Current allocated program and a separate replay-only evidence graph.
/// Ordinary reads never traverse the evidence or select a program by history.
/// Every constructor independently replays its source before taking ownership.
/// The retained source has no mutable access or interior-mutable proof data;
/// shared Arc inputs can only be changed by copying them into a different owner.
#[derive(Debug)]
pub struct RetainedAllocation {
    current: super::current::CurrentAllocation,
    replay: ReplayInputs,
}

#[derive(Debug)]
enum ReplayInputs {
    RuntimeSpill(Box<crate::assignment::runtime_spill::RuntimeSpillAllocation>),
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
        program: register_homes::AllocatedProgram,
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

    /// Corrupt the recorded active-resident rematerialization prefix a
    /// runtime-spill composition carries, so cross-phase controls prove
    /// replay rejects the prefix before trusting its spill steps.
    /// Returns `false` when the retained source has no such prefix.
    #[cfg(feature = "test-support")]
    pub fn corrupt_runtime_spill_active_resident_prefix_custody_for_test(&mut self) -> bool {
        match &mut self.replay {
            ReplayInputs::RuntimeSpill(source) => {
                source.corrupt_active_resident_prefix_custody_for_test()
            }
            _ => false,
        }
    }

    /// Re-run the full independent replay of the retained source rather than
    /// the immutable-admission projection. Cross-phase corruption controls use
    /// this to prove a mutated source is rejected by the same validation the
    /// constructor ran.
    #[cfg(feature = "test-support")]
    pub fn fresh_source_replay_for_test(&self) -> Result<(), AllocationReplayError> {
        match &self.replay {
            ReplayInputs::RuntimeSpill(source) => source.replay_allocation().map(|_| ()),
            ReplayInputs::Baseline(source) => source.replay_allocation().map(|_| ()),
            ReplayInputs::FixedView(source) => source.replay_allocation().map(|_| ()),
            ReplayInputs::LiteralFolds(source) => source.replay_allocation().map(|_| ()),
            ReplayInputs::SelectedLowering(source) => source.replay_allocation().map(|_| ()),
            ReplayInputs::Rematerialization(source) => source.replay_allocation().map(|_| ()),
        }
    }

    pub fn program(&self) -> &register_homes::AllocatedProgram {
        &self.current.program
    }

    pub fn current(&self) -> AllocationOutput<'_> {
        self.current.view()
    }
}

impl sealed::Sealed for RetainedAllocation {}

impl AllocationSource for RetainedAllocation {
    fn replay_allocation(&self) -> Result<AllocationOutput<'_>, AllocationReplayError> {
        // Reuse admission of this exact privately owned, immutable source, not
        // a detached identity or a result from another allocation. Fresh source
        // inputs still take the full replay path in every TryFrom below. Rejoin
        // all current facts so even test-only current-program substitution rejects.
        let prefix_selection = match &self.replay {
            ReplayInputs::RuntimeSpill(source) => source.recovery_prefix_selection(),
            _ => None,
        };
        let current = match &self.replay {
            ReplayInputs::RuntimeSpill(source) => source.project_replayed_allocation()?,
            ReplayInputs::Baseline(source) => source.project_allocation(),
            ReplayInputs::FixedView(source) => source.project_allocation(),
            ReplayInputs::LiteralFolds(source) => source.project_allocation(),
            ReplayInputs::SelectedLowering(source) => source.project_allocation(),
            ReplayInputs::Rematerialization(source) => source.project_allocation(),
        };
        validate_recovery_selection(&current, prefix_selection)?;
        self.current.validate_against(&current)?;
        Ok(self.current())
    }
}

impl TryFrom<StagedOptimizedRegisterHomes> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(source: StagedOptimizedRegisterHomes) -> Result<Self, Self::Error> {
        let replayed = source.replay_allocation()?;
        validate_recovery_selection(&replayed, None)?;
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
        validate_recovery_selection(&replayed, None)?;
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
        validate_recovery_selection(&replayed, None)?;
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
        validate_recovery_selection(&replayed, None)?;
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
        validate_recovery_selection(&replayed, None)?;
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
    runtime_spill_prefix_selection: Option<optimization_core::Optimization>,
) -> Result<(), AllocationReplayError> {
    use super::AllocationEvidence;
    use optimization_core::{Optimization, OptimizationExecutionPhase};
    let expected: &[Optimization] = match current.evidence() {
        AllocationEvidence::FixedViewCopies(receipt) => {
            match receipt.source().source().policy() {
                crate::FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => {
                    &[Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1]
                }
                // Leaf-local and immediate-site copies are the default-path
                // recovery for authenticated fixed-site transitions, not a
                // declared selection.
                crate::FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1
                | crate::FixedViewCopyPolicy::ImmediateBeforeFixedUseV1 => &[],
            }
        }
        AllocationEvidence::RuntimeSpill(_) => match runtime_spill_prefix_selection {
            // A sequence that faced pressure hands custody to runtime spill —
            // a shared-entry fixed-view prefix whether the segment-home probe
            // declined it before custody was consumed or copies materialized
            // first, or an active-resident rematerialization sweep whose
            // rebuilt facts still reported `NoCompatibleHome`. The recorded
            // prefix comes from replayed custody evidence, so each declared
            // prefix binds its declared selection while the default paths —
            // direct legality or leaf-local copies — bind none.
            Some(Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1) => {
                &[Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1]
            }
            Some(Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1) => {
                &[Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1]
            }
            _ => &[],
        },
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

impl TryFrom<crate::assignment::runtime_spill::RuntimeSpillAllocation> for RetainedAllocation {
    type Error = AllocationReplayError;

    fn try_from(
        source: crate::assignment::runtime_spill::RuntimeSpillAllocation,
    ) -> Result<Self, Self::Error> {
        let replayed = source.replay_allocation()?;
        validate_recovery_selection(&replayed, source.recovery_prefix_selection())?;
        let current = super::current::CurrentAllocation::from_replayed(&replayed);
        Ok(Self {
            current,
            replay: ReplayInputs::RuntimeSpill(Box::new(source)),
        })
    }
}
