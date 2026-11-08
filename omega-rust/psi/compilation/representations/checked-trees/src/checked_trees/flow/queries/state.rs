use arena::HandleSpan;
use symbols::SymbolHandle;

use super::super::{
    FlowCallFact, FlowConstraintRef, FlowFacts, FlowInvalidationFact, FlowInvalidationSource,
    FlowStateFact, FlowStatementFact,
};
use crate::FlowFactsRef;

impl FlowFacts {
    pub fn state_statement(
        &self,
        state: &FlowStateFact,
        statement_index: usize,
    ) -> Option<&FlowStatementFact> {
        self.view().state_statement(state, statement_index)
    }

    pub fn state_call(
        &self,
        state: &FlowStateFact,
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
        receiver_symbol: SymbolHandle,
    ) -> Option<&FlowCallFact> {
        self.view().state_call(
            state,
            statement_index,
            call_ordinal,
            target_symbol,
            receiver_symbol,
        )
    }

    pub fn state_call_entry_constraints(
        &self,
        state: &FlowStateFact,
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
        receiver_symbol: SymbolHandle,
    ) -> HandleSpan<FlowConstraintRef> {
        self.view().state_call_entry_constraints(
            state,
            statement_index,
            call_ordinal,
            target_symbol,
            receiver_symbol,
        )
    }

    pub fn state_call_entry_semantic_contexts(
        &self,
        state: &FlowStateFact,
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
        receiver_symbol: SymbolHandle,
    ) -> impl Iterator<Item = facts::FactContextHandle> + '_ {
        self.view().state_call_entry_semantic_contexts(
            state,
            statement_index,
            call_ordinal,
            target_symbol,
            receiver_symbol,
        )
    }

    pub fn state_call_prior_invalidations<'a>(
        &'a self,
        state: &'a FlowStateFact,
        call: &'a FlowCallFact,
    ) -> impl Iterator<Item = &'a FlowInvalidationFact> + 'a {
        self.view().state_call_prior_invalidations(state, call)
    }
}

/// The same queries on a shared view, so a pass holding the flow facts split
/// field by field (the permission walk) answers them identically.
impl<'f> FlowFactsRef<'f> {
    pub fn state_statement(
        self,
        state: &FlowStateFact,
        statement_index: usize,
    ) -> Option<&'f FlowStatementFact> {
        self.control
            .statements
            .span_or_empty(state.statements)
            .iter()
            .find(|statement| statement.statement_index == statement_index)
    }

    pub fn state_call(
        self,
        state: &FlowStateFact,
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
        receiver_symbol: SymbolHandle,
    ) -> Option<&'f FlowCallFact> {
        self.control
            .calls
            .span_or_empty(state.calls)
            .iter()
            .find(|call| {
                call.statement_index == statement_index
                    && call.call_ordinal == call_ordinal
                    && call.target_symbol == target_symbol
                    && call.receiver_symbol == receiver_symbol
            })
    }

    pub fn state_call_entry_constraints(
        self,
        state: &FlowStateFact,
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
        receiver_symbol: SymbolHandle,
    ) -> HandleSpan<FlowConstraintRef> {
        self.state_call(
            state,
            statement_index,
            call_ordinal,
            target_symbol,
            receiver_symbol,
        )
        .map(|call| call.entry_constraints)
        .or_else(|| {
            self.state_statement(state, statement_index)
                .map(|statement| statement.entry_constraints)
        })
        .unwrap_or(state.entry_constraints)
    }

    pub fn state_call_entry_semantic_contexts(
        self,
        state: &FlowStateFact,
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
        receiver_symbol: SymbolHandle,
    ) -> impl Iterator<Item = facts::FactContextHandle> + use<'f> {
        self.semantic_constraint_contexts(self.state_call_entry_constraints(
            state,
            statement_index,
            call_ordinal,
            target_symbol,
            receiver_symbol,
        ))
    }

    pub fn state_call_prior_invalidations<'a>(
        self,
        state: &'a FlowStateFact,
        call: &'a FlowCallFact,
    ) -> impl Iterator<Item = &'f FlowInvalidationFact> + 'a
    where
        'f: 'a,
    {
        self.invalidations
            .events
            .span_or_empty(state.invalidations)
            .iter()
            .filter(move |invalidation| match invalidation.source {
                FlowInvalidationSource::Statement { statement_index } => {
                    statement_index < call.statement_index
                }
                FlowInvalidationSource::Call {
                    statement_index,
                    call_ordinal,
                    ..
                } => {
                    statement_index < call.statement_index
                        || (statement_index == call.statement_index
                            && call_ordinal < call.call_ordinal)
                }
            })
    }
}
