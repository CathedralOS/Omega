use crate::borrow::accesses::{ArgumentAccessSchedule, collect_argument_accesses};
use checked_trees::expression::ExpressionHandle;
use checked_trees::{BorrowArgumentAccessFact, BorrowCallFact};
use symbols::SymbolHandle;

pub(super) struct BorrowCallCollection<'a> {
    pub(super) program: &'a dyn ::validation::CheckingInput,
    pub(super) machine: &'a typed_trees::machine::Machine,
    pub(super) state: &'a typed_trees::state::State,
    pub(super) statement_index: usize,
    pub(super) call_ordinal: &'a mut usize,
    pub(super) expressions: Vec<ExpressionHandle>,
    access_segments: &'a mut arena::Arena<facts::PlaceSegment>,
    argument_accesses: &'a mut arena::Arena<BorrowArgumentAccessFact>,
    calls: &'a mut arena::Arena<BorrowCallFact>,
    state_calls: &'a mut arena::HandleSpan<BorrowCallFact>,
}

impl<'a> BorrowCallCollection<'a> {
    pub(super) fn new(
        program: &'a dyn ::validation::CheckingInput,
        machine: &'a typed_trees::machine::Machine,
        state: &'a typed_trees::state::State,
        statement_index: usize,
        call_ordinal: &'a mut usize,
        access_segments: &'a mut arena::Arena<facts::PlaceSegment>,
        argument_accesses: &'a mut arena::Arena<BorrowArgumentAccessFact>,
        calls: &'a mut arena::Arena<BorrowCallFact>,
        state_calls: &'a mut arena::HandleSpan<BorrowCallFact>,
    ) -> Self {
        Self {
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            expressions: Vec::new(),
            access_segments,
            argument_accesses,
            calls,
            state_calls,
        }
    }

    pub(super) fn collect_argument_accesses(
        &mut self,
        arguments: &[ExpressionHandle],
        target_symbol: SymbolHandle,
        schedule: ArgumentAccessSchedule,
    ) -> arena::HandleSpan<BorrowArgumentAccessFact> {
        collect_argument_accesses(
            self.program,
            self.access_segments,
            self.argument_accesses,
            arguments,
            self.state.symbol,
            self.statement_index,
            self.machine.symbol,
            target_symbol,
            schedule,
        )
    }

    pub(super) fn append_borrow_call(
        &mut self,
        authored_expression: ExpressionHandle,
        receiver_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        has_receiver: bool,
        accesses: arena::HandleSpan<BorrowArgumentAccessFact>,
    ) {
        self.calls.append_to_span(
            self.state_calls,
            BorrowCallFact {
                statement_index: self.statement_index,
                call_ordinal: *self.call_ordinal,
                authored_expression,
                reference_slot_effects: Default::default(),
                completed_reference_loans: Default::default(),
                receiver_symbol,
                target_symbol,
                has_receiver,
                accesses,
            },
        );
        *self.call_ordinal += 1;
    }
}
