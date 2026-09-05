use psi_arena::HandleSpan;
use psi_language_semantics::{BlockingSummary, ServiceReachSummary, SuspensionSummary};
use psi_symbols::SymbolHandle;

use crate::{BorrowArgumentAccessFact, BorrowWritableRootFact, ContractProofFactRef};

use super::{
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBoundaryEdgeFact, FlowConstraintRef,
    FlowInvalidationFact, FlowSemanticContextRef,
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
    pub service_reach: ServiceReachSummary,
    pub suspension: SuspensionSummary,
    pub blocking: BlockingSummary,
    pub operational_acknowledgement: psi_language_semantics::CallOperationalAcknowledgement,
    /// Exact authored call location retained while the typed call identity is
    /// still stable. Provider settlement may later rewrite the typed call.
    pub authored_source_span: Option<psi_source::SourceSpan>,
    /// Whether source custody was internally coherent at capture. Ordinary
    /// compilation need not require package-review provenance, but package
    /// projection rejects a false value.
    pub authored_source_custody_valid: bool,
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
    pub transition_target: psi_typed_trees::statement::TransitionTargetHandle,
    pub parameter_origins: HandleSpan<FlowExitParameterOrigin>,
    pub entry_semantic_contexts: HandleSpan<FlowSemanticContextRef>,
    pub entry_constraints: HandleSpan<FlowConstraintRef>,
    pub ensures_contexts: HandleSpan<FlowSemanticContextRef>,
    pub ensures_constraints: HandleSpan<FlowConstraintRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowExitParameterOrigin {
    pub contract: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    pub entry_parameter: SymbolHandle,
    /// Invalid when explicit incoming edges do not establish one exact origin.
    pub state_parameter: SymbolHandle,
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
    pub boundary_edges: HandleSpan<FlowBoundaryEdgeFact>,
    pub statements: HandleSpan<FlowStatementFact>,
    pub calls: HandleSpan<FlowCallFact>,
    pub exits: HandleSpan<FlowExitFact>,
    pub service_reach: ServiceReachSummary,
    pub suspension: SuspensionSummary,
    pub blocking: BlockingSummary,
}
