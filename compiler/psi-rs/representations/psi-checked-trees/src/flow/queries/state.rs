use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

use super::super::{
    FlowCallFact, FlowConstraintRef, FlowFacts, FlowInvalidationFact, FlowInvalidationSource,
    FlowStateFact, FlowStatementFact,
};

impl FlowFacts {
    pub fn state_statement(
        &self,
        state: &FlowStateFact,
        statement_index: usize,
    ) -> Option<&FlowStatementFact> {
        self.control
            .statements
            .span_or_empty(state.statements)
            .iter()
            .find(|statement| statement.statement_index == statement_index)
    }

    pub fn state_call(
        &self,
        state: &FlowStateFact,
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
        receiver_symbol: SymbolHandle,
    ) -> Option<&FlowCallFact> {
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
        &self,
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
        &self,
        state: &FlowStateFact,
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
        receiver_symbol: SymbolHandle,
    ) -> impl Iterator<Item = psi_facts::FactContextHandle> + '_ {
        self.semantic_constraint_contexts(self.state_call_entry_constraints(
            state,
            statement_index,
            call_ordinal,
            target_symbol,
            receiver_symbol,
        ))
    }

    pub fn state_call_prior_invalidations<'a>(
        &'a self,
        state: &'a FlowStateFact,
        call: &'a FlowCallFact,
    ) -> impl Iterator<Item = &'a FlowInvalidationFact> + 'a {
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
