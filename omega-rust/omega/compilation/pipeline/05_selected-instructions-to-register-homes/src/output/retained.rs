use super::{AllocationAdmissionError, AllocationOutput, ProjectAllocation};
use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    StagedOptimizedRegisterHomesAfterLiteralFolds, StagedOptimizedRegisterHomesAfterPreAllocation,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};
use selected_instructions_to_selected_instructions::FixedViewCopyPolicy;

#[cfg(test)]
mod tests;

/// The current allocated program and its admitted facts, as the allocation
/// route produced them. Each constructor projects one staged result, checks
/// the exercised recovery against the declared allocation-recovery selection,
/// and keeps owned copies of the current facts; the staged producer is then
/// dropped. Later stages read these facts and never replay allocation: each
/// fact was checked by the stage that produced it in this compile
/// (verification placement in `omega-rust/pipeline.md`).
#[derive(Debug)]
pub struct RetainedAllocation {
    current: super::current::CurrentAllocation,
}

impl RetainedAllocation {
    pub fn program(&self) -> &register_homes::AllocatedProgram {
        &self.current.program
    }

    pub fn current(&self) -> AllocationOutput<'_> {
        self.current.view()
    }

    fn capture(
        current: &AllocationOutput<'_>,
        runtime_spill_prefix_selection: Option<optimization_core::Optimization>,
    ) -> Result<Self, AllocationAdmissionError> {
        validate_recovery_selection(current, runtime_spill_prefix_selection)?;
        Ok(Self {
            current: super::current::CurrentAllocation::capture(current),
        })
    }
}

impl TryFrom<StagedOptimizedRegisterHomes> for RetainedAllocation {
    type Error = AllocationAdmissionError;

    fn try_from(source: StagedOptimizedRegisterHomes) -> Result<Self, Self::Error> {
        Self::capture(&source.project_allocation(), None)
    }
}

impl TryFrom<StagedOptimizedRegisterHomesAfterFixedViewCopies> for RetainedAllocation {
    type Error = AllocationAdmissionError;

    fn try_from(
        source: StagedOptimizedRegisterHomesAfterFixedViewCopies,
    ) -> Result<Self, Self::Error> {
        Self::capture(&source.project_allocation(), None)
    }
}

impl TryFrom<StagedOptimizedRegisterHomesAfterLiteralFolds> for RetainedAllocation {
    type Error = AllocationAdmissionError;

    fn try_from(
        source: StagedOptimizedRegisterHomesAfterLiteralFolds,
    ) -> Result<Self, Self::Error> {
        Self::capture(&source.project_allocation(), None)
    }
}

impl TryFrom<StagedOptimizedRegisterHomesAfterSelectedLowering> for RetainedAllocation {
    type Error = AllocationAdmissionError;

    fn try_from(
        source: StagedOptimizedRegisterHomesAfterSelectedLowering,
    ) -> Result<Self, Self::Error> {
        Self::capture(&source.project_allocation(), None)
    }
}

impl TryFrom<StagedOptimizedRegisterHomesAfterPreAllocation> for RetainedAllocation {
    type Error = AllocationAdmissionError;

    fn try_from(
        source: StagedOptimizedRegisterHomesAfterPreAllocation,
    ) -> Result<Self, Self::Error> {
        Self::capture(&source.project_allocation(), None)
    }
}

impl TryFrom<StagedOptimizedActiveResidentRematerialization> for RetainedAllocation {
    type Error = AllocationAdmissionError;

    fn try_from(
        source: StagedOptimizedActiveResidentRematerialization,
    ) -> Result<Self, Self::Error> {
        Self::capture(&source.project_allocation(), None)
    }
}

impl TryFrom<crate::assignment::runtime_spill::RuntimeSpillAllocation> for RetainedAllocation {
    type Error = AllocationAdmissionError;

    fn try_from(
        source: crate::assignment::runtime_spill::RuntimeSpillAllocation,
    ) -> Result<Self, Self::Error> {
        Self::capture(
            &source.project_allocation()?,
            source.recovery_prefix_selection(),
        )
    }
}

// Completion belongs to allocation, not to a layout adapter. Derive the
// exercised recovery selection from the produced evidence, then compare it
// with the retained build policy; copying policy into a manifest is not proof.
fn validate_recovery_selection(
    current: &AllocationOutput<'_>,
    runtime_spill_prefix_selection: Option<optimization_core::Optimization>,
) -> Result<(), AllocationAdmissionError> {
    use super::AllocationEvidence;
    use optimization_core::{Optimization, OptimizationExecutionPhase};
    let expected: &[Optimization] = match current.evidence() {
        AllocationEvidence::FixedViewCopies(receipt) => {
            match receipt.source().source().policy() {
                FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => {
                    &[Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1]
                }
                // Leaf-local, immediate-site, and shared-source-exit copies
                // are the default-path recovery for authenticated fixed-site
                // transitions, not a declared selection.
                FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1
                | FixedViewCopyPolicy::ImmediateBeforeFixedUseV1
                | FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1 => &[],
            }
        }
        AllocationEvidence::RuntimeSpill(_) => match runtime_spill_prefix_selection {
            // A sequence that faced pressure hands custody to runtime spill —
            // a shared-entry fixed-view prefix whether the segment-home probe
            // declined it before custody was consumed or copies materialized
            // first, or an active-resident rematerialization sweep whose
            // rebuilt facts still reported `NoCompatibleHome`. The recorded
            // prefix comes from the recovery's source custody, so each
            // declared prefix binds its declared selection while the default
            // paths — direct legality or leaf-local copies — bind none.
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
        // A pre-allocation run can never compose with a declared recovery
        // selection: `optimize_selected_instructions` rejects a selection
        // mixing an executed phase with the carried-but-unexecuted
        // allocation-recovery slice before this stage runs.
        AllocationEvidence::RegisterHomes(_)
        | AllocationEvidence::LiteralFolds(_)
        | AllocationEvidence::SelectedLowering(_)
        | AllocationEvidence::PreAllocation(_) => &[],
    };
    if current
        .selections()
        .for_phase(OptimizationExecutionPhase::AllocationRecovery)
        .as_slice()
        != expected
    {
        return Err(AllocationAdmissionError::SelectionMismatch);
    }
    Ok(())
}
