/*
We admit selected-instruction optimization compositions, stage liveness,
live-range, and frameless-leaf allocation legality prerequisites, run the one
selected catalog slice (`SelectedLowering` or `PreAllocation`), and hand
Omega 05 one `SelectedAllocationInput`: the current program, the analyses this
stage already validated for it, and the record of the rewrites that produced
it. Whether a phase ran changes that record, never the form Omega 05 receives.

The default compile trusts the run it just produced and does not replay it
(omega-rust/pipeline.md#verification-placement). The completed run stays
available as `SelectedOptimizationReplay` only when the driver asks for it with
`retain_replay_for_validation`; its `validate` is the independent validator
that rebuilds every step and checks the run is the one an allocated program's
record names.
*/

pub(crate) mod optimization_output {
    use crate::{
        OptimizedAllocationLegalityCustodyError, OptimizedLiteralFoldCustodyError,
        OptimizedLiveRangeCustodyError, OptimizedLivenessCustodyError,
        OptimizedPreAllocationCustodyError, StagedPreAllocationOptimizationRun,
        StagedSelectedLoweringOptimizationRun, validate_pre_allocation_optimization_custody,
        validate_selected_lowering_optimization_custody,
    };
    use selected_instruction_analyses::{
        SelectedAllocationInput, SelectedOptimizationRecord, SelectedProgramRef,
    };
    use selected_instructions::SelectedProgram;

    /// Omega 04's result: the input Omega 05 allocates, and the completed
    /// rewrite run behind it when a phase ran.
    #[derive(Debug)]
    pub struct SelectedInstructionOptimizationOutput {
        allocation: SelectedAllocationInput,
        run: Option<CompletedRun>,
    }

    impl SelectedInstructionOptimizationOutput {
        pub(crate) const fn new(
            allocation: SelectedAllocationInput,
            run: Option<CompletedRun>,
        ) -> Self {
            Self { allocation, run }
        }

        /// The current selected program.
        pub const fn program(&self) -> &SelectedProgram {
            self.allocation.selected()
        }

        /// The input for register allocation. The completed run is dropped:
        /// a default compile does not replay it.
        pub fn into_allocation_input(self) -> SelectedAllocationInput {
            self.allocation
        }

        /// The input for register allocation, keeping the completed run for an
        /// explicitly requested validator. An identity run has no replay.
        pub fn retain_replay_for_validation(
            self,
        ) -> (SelectedAllocationInput, Option<SelectedOptimizationReplay>) {
            (
                self.allocation,
                self.run.map(|run| SelectedOptimizationReplay { run }),
            )
        }
    }

    #[derive(Debug)]
    pub(crate) enum CompletedRun {
        SelectedLowering(StagedSelectedLoweringOptimizationRun),
        PreAllocation(StagedPreAllocationOptimizationRun),
    }

    /// One completed Omega 04 rewrite run, kept only for an explicit
    /// validator. It is evidence beside the allocated program, never input to
    /// a later stage.
    #[derive(Debug)]
    pub struct SelectedOptimizationReplay {
        run: CompletedRun,
    }

    impl SelectedOptimizationReplay {
        /// The program the run ended with.
        pub fn output(&self) -> SelectedProgramRef<'_> {
            match &self.run {
                CompletedRun::SelectedLowering(run) => run.current().selected,
                CompletedRun::PreAllocation(run) => run.current(),
            }
        }

        /// Independently rebuild every step of the run from its own input,
        /// and require `record` (what an allocated program says produced it)
        /// to name exactly this run's completion and transformations.
        pub fn validate(
            &self,
            record: &SelectedOptimizationRecord,
        ) -> Result<(), SelectedOptimizationReplayError> {
            match &self.run {
                CompletedRun::SelectedLowering(run) => {
                    if record.selected_lowering_completion != Some(run.custody().identity())
                        || record.pre_allocation_completion
                            != run
                                .source_legality_stage()
                                .record()
                                .pre_allocation_completion
                        || *record != run.record()
                    {
                        return Err(SelectedOptimizationReplayError::OtherProgram);
                    }
                    validate_selected_lowering_optimization_custody(run)
                        .map_err(SelectedOptimizationReplayError::SelectedLowering)?;
                }
                CompletedRun::PreAllocation(run) => {
                    if record.pre_allocation_completion != Some(run.custody().identity())
                        || record.selected_lowering_completion
                            != run
                                .source_legality_stage()
                                .record()
                                .selected_lowering_completion
                        || *record != run.record()
                    {
                        return Err(SelectedOptimizationReplayError::OtherProgram);
                    }
                    validate_pre_allocation_optimization_custody(run)
                        .map_err(SelectedOptimizationReplayError::PreAllocation)?;
                }
            }
            Ok(())
        }
    }

    impl selected_instruction_analyses::RetainedSelectedOptimizationRun for SelectedOptimizationReplay {
        fn validate(&self, record: &SelectedOptimizationRecord) -> Result<(), String> {
            SelectedOptimizationReplay::validate(self, record).map_err(|error| error.to_string())
        }
    }

    /// Why a retained Omega 04 run failed its independent validation.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SelectedOptimizationReplayError {
        /// The run is not the one the checked program's record names.
        OtherProgram,
        SelectedLowering(OptimizedLiteralFoldCustodyError),
        PreAllocation(OptimizedPreAllocationCustodyError),
    }

    failed_display!(
        SelectedOptimizationReplayError,
        "selected-instruction optimization replay failed"
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SelectedInstructionOptimizationError {
        Liveness(OptimizedLivenessCustodyError),
        LiveRanges(OptimizedLiveRangeCustodyError),
        Legality(OptimizedAllocationLegalityCustodyError),
        Rewrite(OptimizedLiteralFoldCustodyError),
        PreAllocation(OptimizedPreAllocationCustodyError),
        UnsupportedComposition,
    }

    failed_display!(
        SelectedInstructionOptimizationError,
        "selected-instruction optimization failed"
    );
}

use crate::{
    SELECTED_STAGE_RULE_CATALOG, SelectedInstructionOptimizationError,
    SelectedInstructionOptimizationOutput, SelectedOptimizationInput, SelectedProgramView,
    SelectedStageRuleRows, run_pre_allocation_optimizations, run_selected_lowering_optimizations,
    stage_optimized_live_ranges, stage_optimized_liveness,
};
use optimization_core::{OptimizationExecutionPhase, OptimizationSelections};
use optimization_output::CompletedRun;
use selected_instruction_analyses::{
    AllocatorAvailabilityRequirement, SelectedAllocationInput, SelectedOptimizationRecord,
    SelectedProgramRef, StagedOptimizedAllocationLegality, ValidatedAllocationLegality,
    ValidatedLiveRanges, ValidatedLiveness, admit_optimized_allocation_legality,
    admit_optimized_live_ranges, admit_optimized_liveness, stage_optimized_allocation_legality,
};
use selected_instructions::SelectedProgram;

pub fn optimize_selected_instructions(
    selected: SelectedOptimizationInput,
) -> Result<SelectedInstructionOptimizationOutput, SelectedInstructionOptimizationError> {
    let liveness = stage_optimized_liveness(selected)
        .map_err(SelectedInstructionOptimizationError::Liveness)?;
    let ranges = stage_optimized_live_ranges(liveness)
        .map_err(SelectedInstructionOptimizationError::LiveRanges)?;
    let selections = ranges.selections();
    if !has_executed_selections(selections) {
        return Ok(SelectedInstructionOptimizationOutput::new(
            SelectedAllocationInput::with_live_ranges(ranges),
            None,
        ));
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
    let legality = stage_optimized_allocation_legality(
        ranges,
        AllocatorAvailabilityRequirement::FramelessLeafCallerSavedViews,
    )
    .map_err(SelectedInstructionOptimizationError::Legality)?;
    let run = match phase {
        OptimizationExecutionPhase::SelectedLowering => CompletedRun::SelectedLowering(
            run_selected_lowering_optimizations(legality)
                .map_err(SelectedInstructionOptimizationError::Rewrite)?,
        ),
        OptimizationExecutionPhase::PreAllocation => CompletedRun::PreAllocation(
            run_pre_allocation_optimizations(legality)
                .map_err(SelectedInstructionOptimizationError::PreAllocation)?,
        ),
        _ => return Err(SelectedInstructionOptimizationError::UnsupportedComposition),
    };
    let current = match &run {
        CompletedRun::SelectedLowering(run) => run
            .current()
            .restage(run.source_legality_stage(), run.record())?,
        CompletedRun::PreAllocation(run) => run
            .rewritten()
            .restage(run.source_legality_stage(), run.record())?,
    };
    Ok(SelectedInstructionOptimizationOutput::new(
        SelectedAllocationInput::with_legality(current),
        Some(run),
    ))
}

/// The program a rewrite run ended with and the analyses the run validated
/// for it.
pub(crate) struct RewrittenProgram<'run> {
    pub(crate) selected: SelectedProgramRef<'run>,
    pub(crate) liveness: &'run ValidatedLiveness,
    pub(crate) ranges: &'run ValidatedLiveRanges,
    pub(crate) legality: &'run ValidatedAllocationLegality,
}

impl RewrittenProgram<'_> {
    /// Stage the rewritten program for allocation under the run's own
    /// custody, registers, selections and allocator availability, reusing the
    /// run's analyses. Each analysis must name the exact program and analysis
    /// before it, so nothing is recomputed and nothing unrelated is admitted.
    fn restage(
        &self,
        source: &StagedOptimizedAllocationLegality,
        record: SelectedOptimizationRecord,
    ) -> Result<StagedOptimizedAllocationLegality, SelectedInstructionOptimizationError> {
        let input = SelectedOptimizationInput {
            program: SelectedProgram {
                plan: self.selected.shared_selected_plan(),
                identity: self.selected.selected_identity(),
                optimization_unit: self.selected.optimization_unit_identity(),
                work_schedule: self.selected.work_schedule_identity(),
            },
            custody: source.custody().core(),
            register_environment: source.register_environment().clone(),
            selections: source.selections().clone(),
            budget_per_pass: source.budget_per_pass(),
            record,
        };
        let liveness = admit_optimized_liveness(input, self.liveness.clone())
            .map_err(SelectedInstructionOptimizationError::Liveness)?;
        let ranges = admit_optimized_live_ranges(liveness, self.ranges.clone())
            .map_err(SelectedInstructionOptimizationError::LiveRanges)?;
        admit_optimized_allocation_legality(
            ranges,
            source.allocator_availability().clone(),
            self.legality.clone(),
        )
        .map_err(SelectedInstructionOptimizationError::Legality)
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
