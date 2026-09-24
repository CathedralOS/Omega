use super::model::{
    PreAllocationTransformationIdentity, StagedOptimizedPreAllocationIterationReceipt,
    StagedPreAllocationOptimizationRun,
};

fn alternate_budget() -> optimization_core::OptimizationWorkBudget {
    optimization_core::OptimizationWorkBudget::new(7, 7, 7, 7, 7).expect("nonzero budget axes")
}

/// One substitutable field of [`StagedPreAllocationOptimizationCustodyReceipt`](super::StagedPreAllocationOptimizationCustodyReceipt).
/// The custody matrix substitutes exactly one field per leg so a rejection
/// attributes to that claim alone. Every field is representable in memory;
/// the receipt has no wire form, so no field is canonical-encoding-closed.
/// The independent checker is `validate_pre_allocation_optimization_custody`:
/// it re-runs discovery, admission, and analysis staging against the retained
/// legality source and rejects the substitution.
#[derive(Debug, Clone, Copy)]
pub enum PreAllocationOptimizationCustodyFieldForTest {
    Identity,
    Source,
    Selections,
    PreAllocationSelections,
    Policy,
    Budget,
    Usage,
    IterationBound,
    RemovalCount,
    InitialVirtualRegisterCount,
    Iterations,
    Attempt,
    FinalSelected,
    FinalLiveness,
    FinalRanges,
    FinalLegality,
    FinalVirtualRegisterCount,
}

impl StagedPreAllocationOptimizationRun {
    /// Mutate only retained receipt facts; the donor's nested receipts are
    /// authentic foreign evidence and grant no new authority here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: PreAllocationOptimizationCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            PreAllocationOptimizationCustodyFieldForTest::Identity => {
                self.custody.identity =
                    optimization_core::PreAllocationOptimizationCompletionIdentity::from_bytes(
                        [0xc0; 32],
                    );
            }
            PreAllocationOptimizationCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source;
            }
            PreAllocationOptimizationCustodyFieldForTest::Selections => {
                self.custody.selections =
                    optimization_core::OptimizationSelectionIdentity::from_bytes([0xc1; 32]);
            }
            PreAllocationOptimizationCustodyFieldForTest::PreAllocationSelections => {
                self.custody.pre_allocation_selections =
                    optimization_core::OptimizationSelectionIdentity::from_bytes([0xc2; 32]);
            }
            PreAllocationOptimizationCustodyFieldForTest::Policy => {
                // The honest run always admits at least one family bit, so
                // the empty policy is the distinct canonical payload.
                self.custody.policy = super::PreAllocationPolicy::empty();
            }
            PreAllocationOptimizationCustodyFieldForTest::Budget => {
                self.custody.budget = alternate_budget();
            }
            PreAllocationOptimizationCustodyFieldForTest::Usage => {
                self.custody.usage.iterations += 1;
            }
            PreAllocationOptimizationCustodyFieldForTest::IterationBound => {
                self.custody.iteration_bound += 1;
            }
            PreAllocationOptimizationCustodyFieldForTest::RemovalCount => {
                self.custody.removal_count += 1;
            }
            PreAllocationOptimizationCustodyFieldForTest::InitialVirtualRegisterCount => {
                self.custody.initial_virtual_register_count += 1;
            }
            PreAllocationOptimizationCustodyFieldForTest::Iterations => {
                // A run may legitimately apply zero removals, leaving an empty
                // list where a foreign donor's is also empty. Substitute one
                // receipt assembled from this run's own terminal attempt so
                // the mutation is non-vacuous on every honest run.
                let attempt = self.custody.attempt;
                self.custody.iterations = vec![StagedOptimizedPreAllocationIterationReceipt {
                    source_selected: attempt.source_selected,
                    transformation: PreAllocationTransformationIdentity::CopyRemoval(
                        selected_instructions::CopyRemovalIdentity::from_bytes([0xc3; 32]),
                    ),
                    function_index: 0,
                    instruction: selected_instructions::SelectedInstructionId(0),
                    transformed_selected: attempt.source_selected,
                    fresh_liveness: selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]),
                    fresh_ranges: selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]),
                    fresh_legality: register_homes::AllocationLegalityIdentity::from_bytes(
                        [0xb1; 32],
                    ),
                    declined: attempt.declined,
                    evaluated: attempt.candidates,
                }];
            }
            PreAllocationOptimizationCustodyFieldForTest::Attempt => {
                self.custody.attempt = donor.custody.attempt;
            }
            PreAllocationOptimizationCustodyFieldForTest::FinalSelected => {
                self.custody.final_selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xa9; 32]);
            }
            PreAllocationOptimizationCustodyFieldForTest::FinalLiveness => {
                self.custody.final_liveness =
                    selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]);
            }
            PreAllocationOptimizationCustodyFieldForTest::FinalRanges => {
                self.custody.final_ranges =
                    selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]);
            }
            PreAllocationOptimizationCustodyFieldForTest::FinalLegality => {
                self.custody.final_legality =
                    register_homes::AllocationLegalityIdentity::from_bytes([0xb1; 32]);
            }
            PreAllocationOptimizationCustodyFieldForTest::FinalVirtualRegisterCount => {
                self.custody.final_virtual_register_count += 1;
            }
        }
    }
}
