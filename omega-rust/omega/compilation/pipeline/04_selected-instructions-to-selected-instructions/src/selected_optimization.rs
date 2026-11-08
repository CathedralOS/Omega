/*
We admit selected-instruction optimization compositions, stage liveness,
live-range, and frameless-leaf allocation legality prerequisites, dispatch the
active catalog slice (`SelectedLowering` or `PreAllocation`), and package the
resulting current program and proof into `SelectedInstructionOptimizationOutput`.
*/

pub(crate) mod optimization_output {
    use crate::{
        OptimizedAllocationLegalityCustodyError, OptimizedLiteralFoldCustodyError,
        OptimizedLiveRangeCustodyError, OptimizedLivenessCustodyError,
        OptimizedPreAllocationCustodyError, OwnedSelectedProgram, StagedOptimizedLiveRanges,
        StagedPreAllocationOptimizationRun, StagedSelectedLoweringOptimizationRun,
        validate_pre_allocation_optimization_custody,
        validate_selected_lowering_optimization_custody,
    };
    use selected_instruction_analyses::AllocationRecoveryRuleCatalogError;

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
        pub(crate) fn from_evidence(
            evidence: SelectedInstructionOptimizationEvidence,
        ) -> Result<Self, SelectedInstructionOptimizationError> {
            let current = evidence.replay()?;
            Ok(Self { current, evidence })
        }

        pub fn program(&self) -> &OwnedSelectedProgram {
            &self.current
        }

        pub fn into_selected_output(
            self,
        ) -> selected_instruction_analyses::SelectedOptimizationOutput {
            use selected_instruction_analyses::SelectedOptimizationOutput as Output;
            match self.evidence {
                SelectedInstructionOptimizationEvidence::Identity(ranges) => {
                    Output::Identity(ranges)
                }
                SelectedInstructionOptimizationEvidence::LiteralFolds(run) => {
                    Output::SelectedLowering(run.into_completed())
                }
                SelectedInstructionOptimizationEvidence::PreAllocation(run) => {
                    Output::PreAllocation(run.into_completed())
                }
            }
        }
    }

    impl SelectedInstructionOptimizationEvidence {
        fn replay(&self) -> Result<OwnedSelectedProgram, SelectedInstructionOptimizationError> {
            match self {
                Self::Identity(ranges) => {
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
                    Ok(OwnedSelectedProgram::retain(run.attempt().fold()))
                }
                Self::PreAllocation(run) => {
                    validate_pre_allocation_optimization_custody(run)
                        .map_err(SelectedInstructionOptimizationError::PreAllocation)?;
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
        MissingExecution,
    }

    failed_display!(
        SelectedInstructionOptimizationError,
        "selected-instruction optimization failed"
    );
}

use crate::analyses::stage_optimized_allocation_legality_for_frameless_leaf;
use crate::{
    SELECTED_STAGE_RULE_CATALOG, SelectedInstructionOptimizationError,
    SelectedInstructionOptimizationEvidence, SelectedInstructionOptimizationOutput,
    SelectedOptimizationInput, SelectedStageRuleRows, run_pre_allocation_optimizations,
    run_selected_lowering_optimizations, stage_optimized_live_ranges, stage_optimized_liveness,
};
use optimization_core::{OptimizationExecutionPhase, OptimizationSelections};

pub fn optimize_selected_instructions(
    selected: SelectedOptimizationInput,
) -> Result<SelectedInstructionOptimizationOutput, SelectedInstructionOptimizationError> {
    let liveness = stage_optimized_liveness(selected)
        .map_err(SelectedInstructionOptimizationError::Liveness)?;
    let ranges = stage_optimized_live_ranges(liveness)
        .map_err(SelectedInstructionOptimizationError::LiveRanges)?;
    let selections = ranges.selections();
    if !has_executed_selections(selections) {
        return SelectedInstructionOptimizationOutput::from_evidence(
            SelectedInstructionOptimizationEvidence::Identity(ranges),
        );
    }
    if unexecutable_catalog_composition(selections).is_some() {
        return Err(SelectedInstructionOptimizationError::UnsupportedComposition);
    }
    let mut executed =
        executed_slice_phases().filter(|phase| !selections.for_phase(*phase).is_empty());
    let phase = executed
        .next()
        .expect("executed selections exist past the identity check");
    if executed.next().is_some() {
        return Err(SelectedInstructionOptimizationError::UnsupportedComposition);
    }
    let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges)
        .map_err(SelectedInstructionOptimizationError::Legality)?;
    match phase {
        OptimizationExecutionPhase::SelectedLowering => {
            let run = run_selected_lowering_optimizations(legality)
                .map_err(SelectedInstructionOptimizationError::Rewrite)?;
            SelectedInstructionOptimizationOutput::from_evidence(
                SelectedInstructionOptimizationEvidence::LiteralFolds(run),
            )
        }
        OptimizationExecutionPhase::PreAllocation => {
            let run = run_pre_allocation_optimizations(legality)
                .map_err(SelectedInstructionOptimizationError::PreAllocation)?;
            SelectedInstructionOptimizationOutput::from_evidence(
                SelectedInstructionOptimizationEvidence::PreAllocation(run),
            )
        }
        _ => Err(SelectedInstructionOptimizationError::UnsupportedComposition),
    }
}

fn slice_executes_at_stage(rows: SelectedStageRuleRows) -> bool {
    matches!(
        rows,
        SelectedStageRuleRows::SelectedLowering(_) | SelectedStageRuleRows::PreAllocation(_)
    )
}

pub(crate) fn executed_slice_phases() -> impl Iterator<Item = OptimizationExecutionPhase> {
    SELECTED_STAGE_RULE_CATALOG
        .iter()
        .filter(|slice| slice_executes_at_stage(slice.rows()))
        .map(|slice| slice.phase())
}

fn has_executed_selections(selections: &OptimizationSelections) -> bool {
    executed_slice_phases().any(|phase| !selections.for_phase(phase).is_empty())
}

fn unexecutable_catalog_composition(
    selections: &OptimizationSelections,
) -> Option<OptimizationExecutionPhase> {
    if !has_executed_selections(selections) {
        return None;
    }
    SELECTED_STAGE_RULE_CATALOG
        .iter()
        .filter(|slice| !slice_executes_at_stage(slice.rows()))
        .map(|slice| slice.phase())
        .find(|phase| !selections.for_phase(*phase).is_empty())
}
