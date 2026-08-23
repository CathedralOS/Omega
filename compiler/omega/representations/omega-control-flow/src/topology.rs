use psi_arena::HandleSpan;
use psi_language_semantics::{BlockingSummary, ServiceReachSummary, SuspensionSummary};
use psi_symbols::SymbolHandle;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::types::TypeReferenceHandle;

pub use omega_function_identity::{MachineFunctionIdentity, StateKey};

use crate::{
    Operation, StateBorrowSummary, StateBoundarySummary, StateContractSummary,
    StateOwnershipSummary, StateValueSummary, TransitionFlow,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFlow {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    pub service_reach: ServiceReachSummary,
    pub suspension: SuspensionSummary,
    pub blocking: BlockingSummary,
    pub contains: HandleSpan<ContainedFlow>,
    pub owned_data: HandleSpan<MachineOwnedDataFlow>,
    pub states: HandleSpan<StateFlow>,
}

impl Default for MachineFlow {
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
pub struct ContainedFlow {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_symbol: SymbolHandle,
    pub type_name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineOwnedDataFlow {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFlow {
    pub key: StateKey,
    pub name: Identifier,
    pub index: usize,
    pub service_reach: ServiceReachSummary,
    pub suspension: SuspensionSummary,
    pub blocking: BlockingSummary,
    pub parameters: HandleSpan<StateParameterFlow>,
    pub contracts: StateContractSummary,
    pub values: StateValueSummary,
    pub boundaries: StateBoundarySummary,
    pub borrow: StateBorrowSummary,
    pub ownership: StateOwnershipSummary,
    pub operations: HandleSpan<Operation>,
    pub transitions: HandleSpan<TransitionFlow>,
}

impl Default for StateFlow {
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
pub struct StateParameterFlow {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub type_symbol: SymbolHandle,
    pub type_name: Identifier,
    pub is_mutable_reference: bool,
    /// Complete checked conformances eligible for a bare `dyn Trait`
    /// parameter. Exact retained rows are authoritative for every candidate;
    /// carrier and method spellings are diagnostic data only.
    pub dyn_conformance_candidates: Vec<psi_checked_trees::DynamicConformanceCandidateFact>,
    /// Exact checked rows selected by a named closed dynamic parameter.
    pub dyn_conformance_rows: Vec<psi_checked_trees::DynamicConformanceRowFact>,
}
