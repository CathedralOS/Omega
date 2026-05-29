use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;

use crate::{
    BorrowArgumentAccessFact, BorrowCallFact, BorrowLoanFact, BorrowWritableRootFact,
    StateBorrowFact,
};

use super::{
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBoundaryEdgeFact, FlowCallFact,
    FlowConstraintKind, FlowConstraintRef, FlowDropEventFact, FlowExitFact, FlowInvalidationFact,
    FlowInvalidationSource, FlowMoveEventFact, FlowSemanticContextRef, FlowStateFact,
    FlowStatementFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowContextFacts {
    pub semantic_context_refs: Arena<FlowSemanticContextRef>,
    pub constraint_refs: Arena<FlowConstraintRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowInvalidationFacts {
    pub segments: Arena<omega_facts::PlaceSegment>,
    pub events: Arena<FlowInvalidationFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowBorrowLifetimeFacts {
    pub activations: Arena<FlowBorrowActivationFact>,
    pub weakenings: Arena<FlowBorrowWeakeningFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowOwnershipFacts {
    pub segments: Arena<omega_facts::PlaceSegment>,
    pub moves: Arena<FlowMoveEventFact>,
    pub drops: Arena<FlowDropEventFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowBoundaryFacts {
    pub edges: Arena<FlowBoundaryEdgeFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowControlFacts {
    pub statements: Arena<FlowStatementFact>,
    pub calls: Arena<FlowCallFact>,
    pub exits: Arena<FlowExitFact>,
    pub states: Arena<FlowStateFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowFacts {
    pub contexts: FlowContextFacts,
    pub invalidations: FlowInvalidationFacts,
    pub borrow_lifetimes: FlowBorrowLifetimeFacts,
    pub ownership: FlowOwnershipFacts,
    pub boundaries: FlowBoundaryFacts,
    pub control: FlowControlFacts,
}

impl FlowFacts {
    pub fn constraints(&self, constraints: HandleSpan<FlowConstraintRef>) -> &[FlowConstraintRef] {
        self.contexts.constraint_refs.span_or_empty(constraints)
    }

    pub fn semantic_constraint_contexts(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = omega_facts::FactContextHandle> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::SemanticContext { context } => Some(context),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_state_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<StateBorrowFact>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowState { state } => Some(state),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_call_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowCallFact>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowCall { call } => Some(call),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_writable_root_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowWritableRootFact>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowWritableRoot { root } => Some(root),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowAccess { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_access_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowArgumentAccessFact>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowAccess { access } => Some(access),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowLoan { .. } => None,
            })
    }

    pub fn borrow_loan_constraints(
        &self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowLoanFact>> + '_ {
        self.constraints(constraints)
            .iter()
            .filter_map(|constraint| match constraint.kind {
                FlowConstraintKind::BorrowLoan { loan } => Some(loan),
                FlowConstraintKind::Unknown
                | FlowConstraintKind::SemanticContext { .. }
                | FlowConstraintKind::BorrowState { .. }
                | FlowConstraintKind::BorrowCall { .. }
                | FlowConstraintKind::BorrowWritableRoot { .. }
                | FlowConstraintKind::BorrowAccess { .. } => None,
            })
    }

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
    ) -> impl Iterator<Item = omega_facts::FactContextHandle> + '_ {
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
