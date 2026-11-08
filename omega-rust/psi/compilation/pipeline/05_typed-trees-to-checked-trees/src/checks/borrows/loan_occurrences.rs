//! Source-slot associations are prepared once for an immutable pass or question.
//! Every use still selects its live loans and checks access and lifetime duties.

use checked_trees::{CheckFacts, FlowStateFact};
use validation::reference_result_custody::PreparedDescriptorLoadSlots;

pub(super) struct LoanOccurrences<'facts> {
    slots: PreparedDescriptorLoadSlots<'facts>,
}

impl<'facts> LoanOccurrences<'facts> {
    pub(super) fn new(program: &dyn validation::CheckingInput, facts: &'facts CheckFacts) -> Self {
        Self {
            slots: PreparedDescriptorLoadSlots::new(program, facts),
        }
    }

    pub(super) fn for_state(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'facts>,
        state: &FlowStateFact,
    ) -> Self {
        Self {
            slots: PreparedDescriptorLoadSlots::for_state(program, facts, state),
        }
    }

    pub(super) fn descriptor_slot(
        &self,
        loan: &checked_trees::BorrowLoanFact,
    ) -> Result<&validation::CanonicalPlace, &'static str> {
        self.slots.slot(loan)
    }
}
