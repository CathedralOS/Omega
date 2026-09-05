use super::*;
use crate::borrow::accesses::collect_call_argument_accesses;

pub(super) struct BorrowCallCollection<'a> {
    pub(super) program: &'a typed_trees::TypedTrees,
    pub(super) machine: &'a typed_trees::machine::Machine,
    pub(super) state: &'a typed_trees::state::State,
    pub(super) statement_index: usize,
    pub(super) call_ordinal: &'a mut usize,
    access_segments: &'a mut arena::Arena<facts::PlaceSegment>,
    argument_accesses: &'a mut arena::Arena<BorrowArgumentAccessFact>,
    calls: &'a mut arena::Arena<BorrowCallFact>,
    state_calls: &'a mut arena::HandleSpan<BorrowCallFact>,
}

impl<'a> BorrowCallCollection<'a> {
    pub(super) fn new(
        program: &'a typed_trees::TypedTrees,
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
            access_segments,
            argument_accesses,
            calls,
            state_calls,
        }
    }

    pub(super) fn collect_call_argument_accesses(
        &mut self,
        arguments: &[ExpressionHandle],
    ) -> arena::HandleSpan<BorrowArgumentAccessFact> {
        collect_call_argument_accesses(
            self.program,
            self.access_segments,
            self.argument_accesses,
            arguments,
            self.state.symbol,
            self.statement_index,
            self.machine.symbol,
        )
    }

    pub(super) fn append_borrow_call(
        &mut self,
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
                receiver_symbol,
                target_symbol,
                has_receiver,
                accesses,
            },
        );
        *self.call_ordinal += 1;
    }
}
