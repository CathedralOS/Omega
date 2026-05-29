use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;

use crate::{
    BorrowArgumentAccessFact, BorrowCallFact, BorrowLoanFact, BorrowWritableRootFact,
    ContractProofFactRef, StateBorrowFact,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowSemanticContextRef {
    pub context: omega_facts::FactContextHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowConstraintKind {
    #[default]
    Unknown,
    SemanticContext {
        context: omega_facts::FactContextHandle,
    },
    BorrowState {
        state: Handle<StateBorrowFact>,
    },
    BorrowCall {
        call: Handle<BorrowCallFact>,
    },
    BorrowWritableRoot {
        root: Handle<BorrowWritableRootFact>,
    },
    BorrowAccess {
        access: Handle<BorrowArgumentAccessFact>,
    },
    BorrowLoan {
        loan: Handle<BorrowLoanFact>,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowConstraintRef {
    pub kind: FlowConstraintKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowInvalidationSource {
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
    },
}

impl Default for FlowInvalidationSource {
    fn default() -> Self {
        Self::Statement { statement_index: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowInvalidationFact {
    pub source: FlowInvalidationSource,
    pub context: omega_facts::FactContextHandle,
    pub fact: omega_facts::FactHandle,
    pub mutated_root: omega_facts::PlaceRoot,
    pub mutated_segments: HandleSpan<omega_facts::PlaceSegment>,
    pub dependency_segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowBorrowWeakeningReason {
    #[default]
    LastUseExpired,
    StateExit,
    LocalReassigned,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowBorrowActivationFact {
    pub source: FlowInvalidationSource,
    pub loan: Handle<BorrowLoanFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowBorrowWeakeningFact {
    pub source: FlowInvalidationSource,
    pub loan: Handle<BorrowLoanFact>,
    pub reason: FlowBorrowWeakeningReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowOwnershipEventSource {
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
    },
    StateExit,
}

impl Default for FlowOwnershipEventSource {
    fn default() -> Self {
        Self::Statement { statement_index: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowMoveEventFact {
    pub source: FlowOwnershipEventSource,
    pub root: omega_facts::PlaceRoot,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowDropEventFact {
    pub source: FlowOwnershipEventSource,
    pub root: omega_facts::PlaceRoot,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowBoundaryEdgeFact {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub boundary_trait_symbol: SymbolHandle,
    pub boundary_signature_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowCallFact {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub has_receiver: bool,
    pub accesses: HandleSpan<BorrowArgumentAccessFact>,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
    pub requires_contexts: HandleSpan<FlowSemanticContextRef>,
    pub requires_constraints: HandleSpan<FlowConstraintRef>,
    pub exit_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub exit_constraints: HandleSpan<FlowConstraintRef>,
    pub invalidations: HandleSpan<FlowInvalidationFact>,
    pub boundary_edges: HandleSpan<FlowBoundaryEdgeFact>,
    pub requires: HandleSpan<ContractProofFactRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
    pub direct_effects: omega_effects::EffectSet,
    pub transitive_effects: omega_effects::EffectSet,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowStatementFact {
    pub statement_index: usize,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowExitFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
    pub ensures_contexts: HandleSpan<FlowSemanticContextRef>,
    pub ensures_constraints: HandleSpan<FlowConstraintRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowStateFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub writable_roots: HandleSpan<BorrowWritableRootFact>,
    pub mutable_parameter_count: usize,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
    pub invalidations: HandleSpan<FlowInvalidationFact>,
    pub borrow_activations: HandleSpan<FlowBorrowActivationFact>,
    pub borrow_weakenings: HandleSpan<FlowBorrowWeakeningFact>,
    pub moves: HandleSpan<FlowMoveEventFact>,
    pub drops: HandleSpan<FlowDropEventFact>,
    pub boundary_edges: HandleSpan<FlowBoundaryEdgeFact>,
    pub statements: HandleSpan<FlowStatementFact>,
    pub calls: HandleSpan<FlowCallFact>,
    pub exits: HandleSpan<FlowExitFact>,
    pub direct_effects: omega_effects::EffectSet,
    pub transitive_effects: omega_effects::EffectSet,
}

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

    pub fn semantic_constraint_contexts<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = omega_facts::FactContextHandle> + 'a {
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

    pub fn borrow_state_constraints<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<StateBorrowFact>> + 'a {
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

    pub fn borrow_call_constraints<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowCallFact>> + 'a {
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

    pub fn borrow_writable_root_constraints<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowWritableRootFact>> + 'a {
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

    pub fn borrow_access_constraints<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowArgumentAccessFact>> + 'a {
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

    pub fn borrow_loan_constraints<'a>(
        &'a self,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> impl Iterator<Item = Handle<BorrowLoanFact>> + 'a {
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

    pub fn state_call_entry_semantic_contexts<'a>(
        &'a self,
        state: &FlowStateFact,
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
        receiver_symbol: SymbolHandle,
    ) -> impl Iterator<Item = omega_facts::FactContextHandle> + 'a {
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
