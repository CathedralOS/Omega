/*
We admit selected-instruction optimization compositions, stage liveness,
live-range, and frameless-leaf allocation legality prerequisites, run the one
selected catalog slice (`SelectedLowering` or `PreAllocation`), and hand
register allocation one `SelectedAllocationInput`. It carries the current
program, its validated analyses and the record of its rewrites. The same
input type carries an identity run when no phase is selected.

Ordinary compilation drops the completed run after producing the allocation
input. The driver can keep it as `SelectedOptimizationReplay` with
`retain_replay_for_validation`; its `validate` re-executes the producer to check
determinism and checks the run is the one an allocated program's record names.
It cannot establish rewrite correctness independently of that producer.
*/

pub(crate) mod optimization_output {
    use crate::selected_rewrites::{
        OptimizedAllocationLegalityCustodyError, OptimizedLiteralFoldCustodyError,
        OptimizedLiveRangeCustodyError, OptimizedLivenessCustodyError,
        OptimizedPreAllocationCustodyError, StagedPreAllocationOptimizationRun,
        StagedSelectedLoweringOptimizationRun, validate_pre_allocation_optimization_custody,
        validate_selected_lowering_optimization_custody,
    };
    use selected_instruction_analyses::{
        SelectedAllocationInput, SelectedOptimizationRecord, SelectedProgramRef,
    };

    /// The input to register allocation and the completed rewrite run when
    /// an optimization phase ran.
    #[derive(Debug)]
    pub struct SelectedInstructionOptimizationOutput {
        body: std::sync::Arc<physical_program::NativeBodyFacts>,
        allocation: SelectedAllocationInput,
        run: Option<CompletedRun>,
    }

    impl SelectedInstructionOptimizationOutput {
        pub fn body_facts(&self) -> &std::sync::Arc<physical_program::NativeBodyFacts> {
            &self.body
        }

        pub(crate) const fn new(
            body: std::sync::Arc<physical_program::NativeBodyFacts>,
            allocation: SelectedAllocationInput,
            run: Option<CompletedRun>,
        ) -> Self {
            Self {
                body,
                allocation,
                run,
            }
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

    /// One completed selected-instruction rewrite run, kept for an explicit
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

        /// Re-execute every step with the producer from the run's own input,
        /// and require `record` (what an allocated program says produced it)
        /// to name exactly this run's completion and transformations. This
        /// establishes determinism and custody, not independent correctness.
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

    /// Why a retained rewrite run failed its determinism and custody check.
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

    impl SelectedInstructionOptimizationError {
        /// The selected optimization that this target has no implementation of, when that
        /// is why the run failed.
        pub fn unimplemented_optimization(&self) -> Option<optimization_core::Optimization> {
            use crate::selected_rewrites::{LiteralFoldError, OptimizedLiteralFoldCustodyError};
            match self {
                Self::Rewrite(OptimizedLiteralFoldCustodyError::Fold(
                    LiteralFoldError::UnsupportedTarget { optimization },
                )) => Some(*optimization),
                _ => None,
            }
        }
    }
}

use crate::selected_rewrites::{
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

/// A caller must supply the result issued by instruction selection.
///
/// ```compile_fail,E0308
/// fn bypass(bare: selected_instruction_analyses::SelectedOptimizationInput) {
///     let _ = selected_instructions_to_selected_instructions::optimize_selected_instructions(bare);
/// }
/// ```
pub fn optimize_selected_instructions(
    selected: instruction_selection::StagedOptimizedSelectedInstructions,
) -> Result<SelectedInstructionOptimizationOutput, SelectedInstructionOptimizationError> {
    let body = std::sync::Arc::clone(selected.body_facts());
    let program = selected.selected_program();
    let custody = selected.custody().core();
    let (register_environment, selections, budget_per_pass) = selected.into_optimization_settings();
    let input = SelectedOptimizationInput {
        program,
        custody,
        register_environment,
        selections,
        budget_per_pass,
        record: SelectedOptimizationRecord::default(),
    };
    let liveness = stage_optimized_liveness(input)
        .map_err(SelectedInstructionOptimizationError::Liveness)?;
    let ranges = stage_optimized_live_ranges(liveness)
        .map_err(SelectedInstructionOptimizationError::LiveRanges)?;
    let selections = ranges.selections();
    if !has_executed_selections(selections) {
        return Ok(SelectedInstructionOptimizationOutput::new(
            body,
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
        body,
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
