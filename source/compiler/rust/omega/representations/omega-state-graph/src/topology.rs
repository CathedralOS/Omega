use psi_arena::HandleSpan;
use psi_language_semantics::{BlockingSummary, ServiceReachSummary, SuspensionSummary};
use psi_symbols::SymbolHandle;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::types::TypeReferenceHandle;

use crate::{
    Operation, StateBorrowSummary, StateBoundarySummary, StateContractSummary,
    StateOwnershipSummary, StateValueSummary, TransitionEdge,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StateKey {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub segment_index: usize,
}

impl StateKey {
    pub fn is_valid(self) -> bool {
        self.machine.is_valid() && self.state.is_valid()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineGraph {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    pub service_reach: ServiceReachSummary,
    pub suspension: SuspensionSummary,
    pub blocking: BlockingSummary,
    pub contains: HandleSpan<ContainedGraph>,
    pub owned_data: HandleSpan<MachineOwnedDataGraph>,
    pub states: HandleSpan<StateNode>,
}

impl Default for MachineGraph {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            attached_data: None,
            service_reach: ServiceReachSummary::default(),
            suspension: SuspensionSummary::default(),
            blocking: BlockingSummary::default(),
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContainedGraph {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_symbol: SymbolHandle,
    pub type_name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineOwnedDataGraph {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateNode {
    pub key: StateKey,
    pub name: Identifier,
    pub index: usize,
    pub service_reach: ServiceReachSummary,
    pub suspension: SuspensionSummary,
    pub blocking: BlockingSummary,
    pub parameters: HandleSpan<StateParameterNode>,
    pub contracts: StateContractSummary,
    pub values: StateValueSummary,
    pub boundaries: StateBoundarySummary,
    pub borrow: StateBorrowSummary,
    pub ownership: StateOwnershipSummary,
    pub operations: HandleSpan<Operation>,
    pub transitions: HandleSpan<TransitionEdge>,
}

impl Default for StateNode {
    fn default() -> Self {
        Self {
            key: StateKey::default(),
            name: Identifier::default(),
            index: 0,
            service_reach: ServiceReachSummary::default(),
            suspension: SuspensionSummary::default(),
            blocking: BlockingSummary::default(),
            parameters: HandleSpan::empty(),
            contracts: StateContractSummary::default(),
            values: StateValueSummary::default(),
            boundaries: StateBoundarySummary::default(),
            borrow: StateBorrowSummary::default(),
            ownership: StateOwnershipSummary::default(),
            operations: HandleSpan::empty(),
            transitions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateParameterNode {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub type_symbol: SymbolHandle,
    pub type_name: Identifier,
    pub is_mutable_reference: bool,
    /// Complete checked conformances eligible for a bare `dyn Trait`
    /// parameter. Each candidate carries exact requirement-to-realization
    /// rows; downstream planning never rediscovers an attached machine from a
    /// carrier or method name.
    pub dyn_conformance_candidates: Vec<psi_checked_trees::DynamicConformanceCandidateFact>,
    /// Exact checked rows for a named closed dynamic conformance carried by
    /// this parameter. Non-empty rows suppress all attached-machine/name
    /// discovery in backend call planning.
    pub dyn_conformance_rows: Vec<psi_checked_trees::DynamicConformanceRowFact>,
}
