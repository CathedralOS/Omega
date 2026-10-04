use super::records;
use crate::borrow::accesses::BorrowAccessPlace;
use crate::borrow::accesses::borrow_access_place;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{BorrowAccessKind, BorrowArgumentAccessFact};
use records::append_argument_access;
use symbols::SymbolHandle;
use validation::DispatchExtraction;

pub(super) struct BorrowAccessCollection<'a> {
    pub(super) program: &'a typed_trees::TypedTrees,
    access_segments: &'a mut arena::Arena<facts::PlaceSegment>,
    argument_accesses: &'a mut arena::Arena<BorrowArgumentAccessFact>,
    accesses: &'a mut arena::HandleSpan<BorrowArgumentAccessFact>,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    dispatch_extraction: DispatchExtraction,
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
            dispatch_extraction: DispatchExtraction::default(),
        }
    }

    /// Collect a transition arm's own target roster, whose payload bindings
    /// were copied out of the dispatch's saved subject before the call.
    pub(super) fn after_dispatch_extraction(mut self) -> Self {
        if let Some(state) = crate::semantic::calls::find_state(self.program, self.state_symbol) {
            self.dispatch_extraction =
                DispatchExtraction::of_transition_arm(self.program, state, self.statement_index);
        }
        self
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

    pub(super) fn expression_type_reference(
        &self,
        expression: ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        crate::flow::expression_type_reference_in_state(
            self.program,
            self.state_symbol,
            self.statement_index,
            expression,
        )
    }

    pub(super) fn append_argument_access(
        &mut self,
        access_place: BorrowAccessPlace,
        kind: BorrowAccessKind,
    ) {
        self.append_access(access_place, kind, None);
    }

    pub(super) fn append_read_access(
        &mut self,
        access_place: BorrowAccessPlace,
        expression: ExpressionHandle,
    ) {
        // A payload the dispatch already copied ended before the call's
        // first access; any other copied read ends after its own row.
        let end_before_access = self.is_plain_copy(expression).then(|| {
            if self.reads_extracted_copy(expression) {
                0
            } else {
                self.access_count() + 1
            }
        });
        self.append_access(access_place, BorrowAccessKind::Read, end_before_access);
    }

    pub(super) fn access_count(&self) -> usize {
        self.accesses.count() as usize
    }

    pub(super) fn reads_extracted_copy(&self, expression: ExpressionHandle) -> bool {
        self.dispatch_extraction
            .reads_extracted_copy(self.program, expression, |payload| {
                self.is_plain_copy(payload)
            })
    }

    pub(super) fn retain_receiver_accesses(&mut self, first_access: usize) {
        // A copyable receiver's storage may still back its borrowed result.
        // Only completion of a plain-copy-returning call ends these accesses.
        for access in self
            .argument_accesses
            .span_mut_or_empty(*self.accesses)
            .iter_mut()
            .skip(first_access)
        {
            access.end_before_access = None;
        }
    }

    pub(super) fn finish_nested_call(&mut self, first_access: usize, expression: ExpressionHandle) {
        if !self.is_plain_copy(expression) {
            return;
        }
        let end_before_access = self.access_count();
        for access in self
            .argument_accesses
            .span_mut_or_empty(*self.accesses)
            .iter_mut()
            .skip(first_access)
        {
            access.end_before_access.get_or_insert(end_before_access);
        }
    }

    pub(super) fn is_plain_copy(&self, expression: ExpressionHandle) -> bool {
        crate::flow::expression_type_reference_in_state(
            self.program,
            self.state_symbol,
            self.statement_index,
            expression,
        )
        .is_some_and(|reference| {
            validation::has_plain_owned_contents_with_numeric_constraints(self.program, reference)
                && self.program.type_multiplicity(reference)
                    == language_semantics::Multiplicity::Unrestricted
        })
    }

    fn append_access(
        &mut self,
        access_place: BorrowAccessPlace,
        kind: BorrowAccessKind,
        end_before_access: Option<usize>,
    ) {
        append_argument_access(
            self.access_segments,
            self.argument_accesses,
            self.accesses,
            access_place,
            kind,
            end_before_access,
        );
    }
}
