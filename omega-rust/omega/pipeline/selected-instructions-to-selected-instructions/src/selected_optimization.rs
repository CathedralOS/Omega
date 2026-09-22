//! Optimizer module role: executable entrance. Selected CFG to selected CFG.

pub(crate) mod optimization_output;

#[cfg(test)]
mod catalog_route_tests;

use crate::analyses::stage_optimized_allocation_legality_for_frameless_leaf;
use crate::{
    SELECTED_STAGE_RULE_CATALOG, SelectedInstructionOptimizationError,
    SelectedInstructionOptimizationEvidence, SelectedInstructionOptimizationOutput,
    SelectedStageRuleRows, run_selected_lowering_optimizations, stage_optimized_live_ranges,
    stage_optimized_liveness,
};
use optimization_core::{OptimizationExecutionPhase, OptimizationSelections};
use target_operations_to_selected_instructions::StagedOptimizedSelectedInstructions;

/// Stage liveness and live ranges over the selected program, then execute
/// the catalog slices this stage owns. Identity and nonempty selections both
/// publish the same current-program carrier.
pub fn optimize_selected_instructions(
    selected: StagedOptimizedSelectedInstructions,
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
    // The stage catalog is the admission authority: a selection under any
    // other carried phase — which has a slice but no executor — composes
    // unsupportedly. Rejection names the catalog slice rather than a
    // hardcoded phase, so a new executor-less slice joins it automatically.
    if unexecutable_catalog_composition(selections).is_some() {
        return Err(SelectedInstructionOptimizationError::UnsupportedComposition);
    }
    let legality = stage_optimized_allocation_legality_for_frameless_leaf(ranges)
        .map_err(SelectedInstructionOptimizationError::Legality)?;
    let run = run_selected_lowering_optimizations(legality)
        .map_err(SelectedInstructionOptimizationError::Rewrite)?;
    SelectedInstructionOptimizationOutput::from_evidence(
        SelectedInstructionOptimizationEvidence::LiteralFolds(run),
    )
}

/// Whether this entrance executes a catalog slice's selections, keyed on the
/// slice's family rows rather than a phase literal: a family joins the
/// executed set by adding its `SelectedStageRuleRows` arm here alongside its
/// executor and evidence variant, not by editing admission predicates.
fn slice_executes_at_stage(rows: SelectedStageRuleRows) -> bool {
    matches!(rows, SelectedStageRuleRows::SelectedLowering(_))
}

/// The catalog phases this entrance executes, in catalog order. This is the
/// stage's one admission decision: `optimize_selected_instructions`
/// composes and rejects through it, and identity replay in
/// `optimization_output` reads the same set, so a family that gains a
/// `SelectedStageRuleRows` arm also gains its missing-execution rejection
/// without a second site to edit.
pub(crate) fn executed_slice_phases() -> impl Iterator<Item = OptimizationExecutionPhase> {
    SELECTED_STAGE_RULE_CATALOG
        .iter()
        .filter(|slice| slice_executes_at_stage(slice.rows()))
        .map(|slice| slice.phase())
}

/// Whether any slice with a stage executor has selections in flight.
/// Declared-but-unexecuted slices alone compose to a no-op identity: the
/// rule is admitted but no stage runs it yet.
fn has_executed_selections(selections: &OptimizationSelections) -> bool {
    executed_slice_phases().any(|phase| !selections.for_phase(phase).is_empty())
}

/// With an executed composition in flight, the first other phase the stage
/// catalog carries whose selection this entrance cannot execute. A selection
/// under an executor-less slice with no executed members is tolerated: the
/// rule is declared but no stage runs it yet, and the output stays a no-op
/// identity.
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

#[cfg(test)]
mod admission_tests {
    use super::{executed_slice_phases, unexecutable_catalog_composition};
    use optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

    /// Composition and identity replay read the same admission set: every
    /// phase the entrance executes appears exactly once, in catalog order —
    /// today the selected-lowering slice alone.
    #[test]
    fn executed_phases_come_from_the_stage_catalog() {
        assert_eq!(
            executed_slice_phases().collect::<Vec<_>>(),
            [OptimizationExecutionPhase::SelectedLowering]
        );
    }

    /// An executor-less catalog slice's selections are in flight but never
    /// rejected as missing execution by identity replay — the admission set
    /// (not a phase literal) bounds what an identity output may absorb.
    #[test]
    fn executor_less_phases_stay_outside_the_executed_set() {
        assert!(
            !executed_slice_phases()
                .any(|phase| phase == OptimizationExecutionPhase::AllocationRecovery)
        );
    }

    /// A selection mixing selected-lowering rules with rules under a
    /// carried but executor-less catalog phase rejects — the gate reads
    /// `SELECTED_STAGE_RULE_CATALOG`, so a new executor-less slice joins the
    /// rejection without touching the entrance.
    #[test]
    fn mixed_executor_less_phase_is_an_unsupported_composition() {
        let selections = OptimizationSelections::new([
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
        ])
        .unwrap();
        assert_eq!(
            unexecutable_catalog_composition(&selections),
            Some(OptimizationExecutionPhase::AllocationRecovery)
        );
    }

    /// An executor-less carried phase by itself stays a no-op identity —
    /// declared rules are tolerated until a stage executor lands — while
    /// lowering-only, foreign-phase, and empty selections all pass.
    #[test]
    fn only_mixed_compositions_reject() {
        assert_eq!(
            unexecutable_catalog_composition(
                &OptimizationSelections::new([
                    Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
                ])
                .unwrap()
            ),
            None
        );
        assert_eq!(
            unexecutable_catalog_composition(
                &OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate,])
                    .unwrap()
            ),
            None
        );
        assert_eq!(
            unexecutable_catalog_composition(
                &OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()
            ),
            None
        );
        assert_eq!(
            unexecutable_catalog_composition(&OptimizationSelections::default()),
            None
        );
    }
}
