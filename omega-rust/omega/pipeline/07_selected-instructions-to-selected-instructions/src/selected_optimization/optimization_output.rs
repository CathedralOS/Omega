//! Optimizer module role: stage output. Current selected program with separate replay inputs.

use crate::{
    AllocationRecoveryRuleCatalogError, OptimizedAllocationLegalityCustodyError,
    OptimizedLiteralFoldCustodyError, OptimizedLiveRangeCustodyError,
    OptimizedLivenessCustodyError, OptimizedPreAllocationCustodyError, OwnedSelectedProgram,
    StagedOptimizedLiveRanges, StagedPreAllocationOptimizationRun,
    StagedSelectedLoweringOptimizationRun, validate_optimized_live_range_custody,
    validate_pre_allocation_optimization_custody, validate_selected_lowering_optimization_custody,
};

/// Only replay and custody assembly distinguish how the current program was obtained.
#[derive(Debug)]
pub enum SelectedInstructionOptimizationEvidence {
    Identity(StagedOptimizedLiveRanges),
    LiteralFolds(StagedSelectedLoweringOptimizationRun),
    PreAllocation(StagedPreAllocationOptimizationRun),
}

#[derive(Debug)]
pub struct SelectedInstructionOptimizationOutput {
    current: OwnedSelectedProgram,
    evidence: SelectedInstructionOptimizationEvidence,
}

impl SelectedInstructionOptimizationOutput {
    #[cfg(feature = "test-support")]
    pub fn substitute_current_program_for_test(&mut self, program: OwnedSelectedProgram) {
        self.current = program;
    }
    pub(crate) fn from_evidence(
        evidence: SelectedInstructionOptimizationEvidence,
    ) -> Result<Self, SelectedInstructionOptimizationError> {
        let current = evidence.replay()?;
        Ok(Self { current, evidence })
    }

    pub fn program(&self) -> &OwnedSelectedProgram {
        &self.current
    }

    pub fn into_replayed_evidence(
        self,
    ) -> Result<SelectedInstructionOptimizationEvidence, SelectedInstructionOptimizationError> {
        if self.evidence.replay()? != self.current {
            return Err(SelectedInstructionOptimizationError::CurrentProgramMismatch);
        }
        Ok(self.evidence)
    }
}

impl SelectedInstructionOptimizationEvidence {
    fn replay(&self) -> Result<OwnedSelectedProgram, SelectedInstructionOptimizationError> {
        match self {
            Self::Identity(ranges) => {
                validate_optimized_live_range_custody(ranges.liveness_stage(), ranges.ranges())
                    .map_err(SelectedInstructionOptimizationError::LiveRanges)?;
                // Identity output admits only selections under executor-less
                // catalog slices; a selection under any phase the entrance
                // executes without its rewrite run is a missing execution.
                // The executed set comes from the same catalog admission
                // `optimize_selected_instructions` reads.
                if super::executed_slice_phases()
                    .any(|phase| !ranges.selections().for_phase(phase).is_empty())
                {
                    return Err(SelectedInstructionOptimizationError::MissingExecution);
                }
                Ok(OwnedSelectedProgram::retain(ranges.selected()))
            }
            Self::LiteralFolds(run) => {
                validate_selected_lowering_optimization_custody(run)
                    .map_err(SelectedInstructionOptimizationError::Rewrite)?;
                // The no-change terminal attempt is the checked fixed-point output,
                // including when the selected suite applied no rewrite.
                Ok(OwnedSelectedProgram::retain(run.attempt().fold()))
            }
            Self::PreAllocation(run) => {
                validate_pre_allocation_optimization_custody(run)
                    .map_err(SelectedInstructionOptimizationError::PreAllocation)?;
                // The terminal clean discovery sweep is the checked joint
                // fixed-point output, including when no candidate was
                // admissible.
                Ok(OwnedSelectedProgram::retain(&run.current()))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedInstructionOptimizationError {
    Liveness(OptimizedLivenessCustodyError),
    LiveRanges(OptimizedLiveRangeCustodyError),
    Legality(OptimizedAllocationLegalityCustodyError),
    Rewrite(OptimizedLiteralFoldCustodyError),
    PreAllocation(OptimizedPreAllocationCustodyError),
    RecoveryCatalog(AllocationRecoveryRuleCatalogError),
    UnsupportedComposition,
    CurrentProgramMismatch,
    MissingExecution,
}

impl std::fmt::Display for SelectedInstructionOptimizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "selected-instruction optimization failed: {self:?}"
        )
    }
}
impl std::error::Error for SelectedInstructionOptimizationError {}
