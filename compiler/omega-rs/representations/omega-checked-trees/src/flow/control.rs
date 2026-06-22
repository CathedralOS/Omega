use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

use crate::{BorrowArgumentAccessFact, BorrowWritableRootFact, ContractProofFactRef};

use super::{
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBoundaryEdgeFact, FlowConstraintRef,
    FlowDropEventFact, FlowInvalidationFact, FlowMoveEventFact, FlowSemanticContextRef,
};

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
