//! Arena-ordered candidate rows for one immutable effect-planning request.
//! These selections grant no loan or retention verdict. Every consumer keeps
//! its original exact predicates and validates each selected resource normally.

use arena::Handle;
use checked_trees::{
    CheckFacts, CheckedCallBorrowLoanResource as LoanResource, FlowCallFact,
    IncomingReferenceIdentity as ReferenceIdentity, KeyedRows,
};
use symbols::SymbolHandle;

type CallKey = (Handle<FlowCallFact>, SymbolHandle, SymbolHandle);

pub(in crate::execution::terminal_unit) struct RetainedBorrowRecords<'facts> {
    facts: &'facts CheckFacts,
    calls: KeyedRows<CallKey, LoanResource>,
    incoming: KeyedRows<(SymbolHandle, SymbolHandle), ReferenceIdentity>,
}

impl<'facts> RetainedBorrowRecords<'facts> {
    pub(in crate::execution::terminal_unit) fn new(facts: &'facts CheckFacts) -> Self {
        Self {
            facts,
            calls: KeyedRows::default(),
            incoming: KeyedRows::default(),
        }
    }

    pub(super) fn facts(&self) -> &'facts CheckFacts {
        self.facts
    }

    pub(super) fn call_resources(
        &self,
        call: Handle<FlowCallFact>,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Vec<(Handle<LoanResource>, &'facts LoanResource)> {
        let arena = &self.facts.borrow.call_loan_resources;
        self.calls.with_rows(
            arena,
            |row| (row.call, row.machine_symbol, row.state_symbol),
            &(call, machine, state),
            |rows| {
                rows.iter()
                    .map(|&handle| (handle, arena.get(handle)))
                    .collect()
            },
        )
    }

    pub(super) fn incoming_references(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Vec<(Handle<ReferenceIdentity>, &'facts ReferenceIdentity)> {
        let arena = &self.facts.borrow.incoming_references;
        self.incoming.with_rows(
            arena,
            |row| (row.machine_symbol, row.state_symbol),
            &(machine, state),
            |rows| {
                rows.iter()
                    .map(|&handle| (handle, arena.get(handle)))
                    .collect()
            },
        )
    }
}

#[cfg(test)]
mod tests;
