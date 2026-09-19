use super::model::{
    StagedOptimizedLiteralFoldIterationReceipt, StagedOptimizedLiteralFolds,
    StagedSelectedLoweringOptimizationRun,
};

fn alternate_budget() -> optimization_core::OptimizationWorkBudget {
    optimization_core::OptimizationWorkBudget::new(7, 7, 7, 7, 7).expect("nonzero budget axes")
}

/// One substitutable field of [`StagedOptimizedLiteralFoldCustodyReceipt`](super::StagedOptimizedLiteralFoldCustodyReceipt).
/// The custody matrix substitutes exactly one field per leg so a rejection
/// attributes to that claim alone. Every field is representable in memory;
/// the receipt has no wire form, so no field is canonical-encoding-closed.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedLiteralFoldCustodyFieldForTest {
    Source,
    Iterations,
    Transformations,
    FinalSelected,
    FinalLiveness,
    FinalRanges,
    FinalLegality,
    FinalVirtualRegisterCount,
    FinalEntryTransitionCount,
}

impl StagedOptimizedLiteralFolds {
    /// Mutate only retained receipt facts; the donor's nested source receipt
    /// and iteration evidence are authentic foreign evidence and grant no
    /// new authority here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedLiteralFoldCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            OptimizedLiteralFoldCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source;
            }
            OptimizedLiteralFoldCustodyFieldForTest::Iterations => {
                self.custody.iterations = donor.custody.iterations.clone();
            }
            OptimizedLiteralFoldCustodyFieldForTest::Transformations => {
                self.custody.transformations = donor.custody.transformations.clone();
            }
            OptimizedLiteralFoldCustodyFieldForTest::FinalSelected => {
                self.custody.final_selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xa9; 32]);
            }
            OptimizedLiteralFoldCustodyFieldForTest::FinalLiveness => {
                self.custody.final_liveness =
                    selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]);
            }
            OptimizedLiteralFoldCustodyFieldForTest::FinalRanges => {
                self.custody.final_ranges =
                    selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]);
            }
            OptimizedLiteralFoldCustodyFieldForTest::FinalLegality => {
                self.custody.final_legality =
                    crate::AllocationLegalityIdentity::from_bytes([0xb1; 32]);
            }
            OptimizedLiteralFoldCustodyFieldForTest::FinalVirtualRegisterCount => {
                self.custody.final_virtual_register_count += 1;
            }
            OptimizedLiteralFoldCustodyFieldForTest::FinalEntryTransitionCount => {
                self.custody.final_entry_transition_count += 1;
            }
        }
    }
}

/// One substitutable field of [`StagedSelectedLoweringOptimizationCustodyReceipt`](super::StagedSelectedLoweringOptimizationCustodyReceipt).
/// The custody matrix substitutes exactly one field per leg so a rejection
/// attributes to that claim alone. Every field is representable in memory;
/// the receipt has no wire form, so no field is canonical-encoding-closed.
#[derive(Debug, Clone, Copy)]
pub enum SelectedLoweringOptimizationCustodyFieldForTest {
    Identity,
    Source,
    Selections,
    SelectedLoweringSelections,
    Budget,
    Usage,
    IterationBound,
    ActionCount,
    InitialVirtualRegisterCount,
    Iterations,
    Attempt,
    FinalSelected,
    FinalLiveness,
    FinalRanges,
    FinalLegality,
    FinalVirtualRegisterCount,
}

impl StagedSelectedLoweringOptimizationRun {
    /// Mutate only retained receipt facts; the donor's nested receipts are
    /// authentic foreign evidence and grant no new authority here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: SelectedLoweringOptimizationCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            SelectedLoweringOptimizationCustodyFieldForTest::Identity => {
                self.custody.identity =
                    optimization_core::SelectedLoweringOptimizationCompletionIdentity::from_bytes(
                        [0xc0; 32],
                    );
            }
            SelectedLoweringOptimizationCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source;
            }
            SelectedLoweringOptimizationCustodyFieldForTest::Selections => {
                self.custody.selections =
                    optimization_core::OptimizationSelectionIdentity::from_bytes([0xc1; 32]);
            }
            SelectedLoweringOptimizationCustodyFieldForTest::SelectedLoweringSelections => {
                self.custody.selected_lowering_selections =
                    optimization_core::OptimizationSelectionIdentity::from_bytes([0xc2; 32]);
            }
            SelectedLoweringOptimizationCustodyFieldForTest::Budget => {
                self.custody.budget = alternate_budget();
            }
            SelectedLoweringOptimizationCustodyFieldForTest::Usage => {
                self.custody.usage.iterations += 1;
            }
            SelectedLoweringOptimizationCustodyFieldForTest::IterationBound => {
                self.custody.iteration_bound += 1;
            }
            SelectedLoweringOptimizationCustodyFieldForTest::ActionCount => {
                self.custody.action_count += 1;
            }
            SelectedLoweringOptimizationCustodyFieldForTest::InitialVirtualRegisterCount => {
                self.custody.initial_virtual_register_count += 1;
            }
            SelectedLoweringOptimizationCustodyFieldForTest::Iterations => {
                // A run may legitimately apply zero folds, leaving an empty
                // list where a foreign donor's is also empty. Substitute one
                // receipt assembled from this run's own terminal attempt so
                // the mutation is non-vacuous on every honest run.
                let attempt = self.custody.attempt;
                self.custody.iterations = vec![StagedOptimizedLiteralFoldIterationReceipt {
                    source_selected: attempt.source_selected,
                    source_ranges: attempt.source_ranges,
                    source_legality: attempt.source_legality,
                    choices: attempt.choices,
                    choice_policy: attempt.choice_policy,
                    choice_usage: attempt.choice_usage,
                    recovery: attempt.recovery,
                    recovery_policy: attempt.recovery_policy,
                    recovery_usage: attempt.recovery_usage,
                    fold: attempt.fold,
                    fold_policy: attempt.fold_policy,
                    fold_usage: attempt.fold_usage,
                    transformed_selected: attempt.transformed_selected,
                    fresh_liveness: selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]),
                    fresh_ranges: selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]),
                    fresh_legality: crate::AllocationLegalityIdentity::from_bytes([0xb1; 32]),
                }];
            }
            SelectedLoweringOptimizationCustodyFieldForTest::Attempt => {
                self.custody.attempt = donor.custody.attempt;
            }
            SelectedLoweringOptimizationCustodyFieldForTest::FinalSelected => {
                self.custody.final_selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xa9; 32]);
            }
            SelectedLoweringOptimizationCustodyFieldForTest::FinalLiveness => {
                self.custody.final_liveness =
                    selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]);
            }
            SelectedLoweringOptimizationCustodyFieldForTest::FinalRanges => {
                self.custody.final_ranges =
                    selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]);
            }
            SelectedLoweringOptimizationCustodyFieldForTest::FinalLegality => {
                self.custody.final_legality =
                    crate::AllocationLegalityIdentity::from_bytes([0xb1; 32]);
            }
            SelectedLoweringOptimizationCustodyFieldForTest::FinalVirtualRegisterCount => {
                self.custody.final_virtual_register_count += 1;
            }
        }
    }
}
