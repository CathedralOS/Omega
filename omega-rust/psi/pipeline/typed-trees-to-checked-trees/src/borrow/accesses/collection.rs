use super::*;
use records::append_argument_access;

pub(super) struct BorrowAccessCollection<'a> {
    pub(super) program: &'a typed_trees::TypedTrees,
    access_segments: &'a mut arena::Arena<facts::PlaceSegment>,
    argument_accesses: &'a mut arena::Arena<BorrowArgumentAccessFact>,
    accesses: &'a mut arena::HandleSpan<BorrowArgumentAccessFact>,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
}

impl<'a> BorrowAccessCollection<'a> {
    pub(super) fn new(
        program: &'a typed_trees::TypedTrees,
        access_segments: &'a mut arena::Arena<facts::PlaceSegment>,
        argument_accesses: &'a mut arena::Arena<BorrowArgumentAccessFact>,
        accesses: &'a mut arena::HandleSpan<BorrowArgumentAccessFact>,
        state_symbol: SymbolHandle,
        statement_index: usize,
        machine_symbol: SymbolHandle,
    ) -> Self {
        Self {
            program,
            access_segments,
            argument_accesses,
            accesses,
            state_symbol,
            statement_index,
            machine_symbol,
        }
    }

    pub(super) fn borrow_access_place(
        &self,
        expression: ExpressionHandle,
    ) -> Option<BorrowAccessPlace> {
        borrow_access_place(
            self.program,
            self.state_symbol,
            self.statement_index,
            expression,
            self.machine_symbol,
        )
    }

    pub(super) fn append_argument_access(
        &mut self,
        access_place: BorrowAccessPlace,
        kind: BorrowAccessKind,
    ) {
        append_argument_access(
            self.access_segments,
            self.argument_accesses,
            self.accesses,
            access_place,
            kind,
        );
    }
}
