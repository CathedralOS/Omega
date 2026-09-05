//! Optimizer module role: stage output. Current selected program with separate replay inputs.

use crate::*;

/// Only replay and custody assembly distinguish how the current program was obtained.
#[derive(Debug)]
pub enum SelectedInstructionOptimizationEvidence {
    Identity(StagedOptimizedLiveRanges),
    LiteralFolds(StagedSelectedLoweringOptimizationRun),
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
                let selected = ranges.liveness_stage().selected_stage();
                if !selected
                    .optimized_target()
                    .optimized()
                    .selections()
                    .for_phase(optimization_core::OptimizationExecutionPhase::SelectedLowering)
                    .is_empty()
                {
                    return Err(SelectedInstructionOptimizationError::MissingExecution);
                }
                Ok(OwnedSelectedProgram::retain(selected.selected()))
            }
            Self::LiteralFolds(run) => {
                validate_selected_lowering_optimization_custody(run)
                    .map_err(SelectedInstructionOptimizationError::Rewrite)?;
                // The no-change terminal attempt is the checked fixed-point output,
                // including when the selected suite applied no rewrite.
                Ok(OwnedSelectedProgram::retain(run.attempt().fold()))
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
