use psi_arena::HandleSpan;
use psi_language_semantics::{BlockingSummary, ServiceReachSummary, SuspensionSummary};
use psi_symbols::SymbolHandle;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::types::TypeReferenceHandle;
use std::hash::{Hash, Hasher};

use crate::{
    Operation, StateBorrowSummary, StateBoundarySummary, StateContractSummary,
    StateOwnershipSummary, StateValueSummary, TransitionFlow,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StateKey {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub segment_index: usize,
}

impl Hash for StateKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.machine.arena_index().hash(state);
        self.machine.generation().hash(state);
        self.state.arena_index().hash(state);
        self.state.generation().hash(state);
        self.segment_index.hash(state);
    }
}

impl StateKey {
    pub fn is_valid(self) -> bool {
        self.machine.is_valid() && self.state.is_valid()
    }
}

/// Canonical compiler-private identity of one lowered native function.
///
/// Source functions retain their exact control-flow key. Generated functions
/// instead name one closed compiler-owned role and the exact source
/// continuation they adapt; they never acquire a fabricated source key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MachineFunctionIdentity {
    kind: MachineFunctionIdentityKind,
    continuation: StateKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MachineFunctionIdentityKind {
    Source,
    ProgramStorageEntryWrapper,
    CallbackThunk { placement_index: usize },
}

impl MachineFunctionIdentity {
    pub const fn source(source_key: StateKey) -> Self {
        Self {
            kind: MachineFunctionIdentityKind::Source,
            continuation: source_key,
        }
    }

    pub fn program_storage_entry_wrapper(continuation: StateKey) -> Option<Self> {
        continuation.is_valid().then_some(Self {
            kind: MachineFunctionIdentityKind::ProgramStorageEntryWrapper,
            continuation,
        })
    }

    /// Construct the compiler-private identity of one inbound callback thunk.
    ///
    /// The placement index is an exact join into the backend plan, while the
    /// continuation retains the selected source entry that the thunk adapts.
    pub fn callback_thunk(continuation: StateKey, placement_index: usize) -> Option<Self> {
        continuation.is_valid().then_some(Self {
            kind: MachineFunctionIdentityKind::CallbackThunk { placement_index },
            continuation,
        })
    }

    pub const fn source_key(self) -> Option<StateKey> {
        match self.kind {
            MachineFunctionIdentityKind::Source => Some(self.continuation),
            MachineFunctionIdentityKind::ProgramStorageEntryWrapper
            | MachineFunctionIdentityKind::CallbackThunk { .. } => None,
        }
    }

    pub const fn program_storage_entry_continuation(self) -> Option<StateKey> {
        match self.kind {
            MachineFunctionIdentityKind::Source
            | MachineFunctionIdentityKind::CallbackThunk { .. } => None,
            MachineFunctionIdentityKind::ProgramStorageEntryWrapper => Some(self.continuation),
        }
    }

    pub const fn callback_thunk_placement_index(self) -> Option<usize> {
        match self.kind {
            MachineFunctionIdentityKind::CallbackThunk { placement_index } => Some(placement_index),
            MachineFunctionIdentityKind::Source
            | MachineFunctionIdentityKind::ProgramStorageEntryWrapper => None,
        }
    }

    pub const fn associated_source_continuation(self) -> StateKey {
        self.continuation
    }

    pub fn is_valid(self) -> bool {
        self.continuation.is_valid()
    }
}

impl Default for MachineFunctionIdentity {
    fn default() -> Self {
        Self::source(StateKey::default())
    }
}

#[cfg(test)]
mod machine_function_identity_tests {
    use super::{MachineFunctionIdentity, StateKey};
    use psi_symbols::SymbolHandle;

    fn source_key(state: u32) -> StateKey {
        StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(state),
            segment_index: 0,
        }
    }

    #[test]
    fn generated_program_entry_identity_cannot_impersonate_its_source_continuation() {
        let key = source_key(2);
        let source = MachineFunctionIdentity::source(key);
        let wrapper = MachineFunctionIdentity::program_storage_entry_wrapper(key)
            .expect("valid continuation should admit one canonical wrapper identity");

        assert_ne!(source, wrapper);
        assert_eq!(source.source_key(), Some(key));
        assert_eq!(wrapper.source_key(), None);
        assert_eq!(wrapper.program_storage_entry_continuation(), Some(key));
        assert_eq!(wrapper.associated_source_continuation(), key);
        assert!(
            MachineFunctionIdentity::program_storage_entry_wrapper(StateKey::default()).is_none()
        );
    }

    #[test]
    fn callback_thunk_identity_binds_placement_and_cannot_impersonate_source() {
        let key = source_key(2);
        let source = MachineFunctionIdentity::source(key);
        let thunk = MachineFunctionIdentity::callback_thunk(key, 7)
            .expect("valid callback continuation should admit a thunk identity");

        assert_ne!(source, thunk);
        assert_ne!(
            thunk,
            MachineFunctionIdentity::callback_thunk(key, 8).unwrap()
        );
        assert_eq!(thunk.source_key(), None);
        assert_eq!(thunk.callback_thunk_placement_index(), Some(7));
        assert_eq!(thunk.associated_source_continuation(), key);
        assert!(MachineFunctionIdentity::callback_thunk(StateKey::default(), 7).is_none());

        let generation_drift = MachineFunctionIdentity::callback_thunk(
            StateKey {
                state: psi_symbols::SymbolHandle::from_parts(2, 2),
                ..key
            },
            7,
        )
        .unwrap();
        let identities = std::collections::HashSet::from([thunk, generation_drift]);
        assert_eq!(
            identities.len(),
            2,
            "identity hashing must retain generation"
        );
    }
}

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
