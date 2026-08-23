#![forbid(unsafe_code)]

//! Compiler-private identities shared by source-shaped and terminal backends.
//!
//! These identities cross the realization boundary, but they contain no
//! source trees, control-flow plans, target operations, or native addresses.

use psi_symbols::SymbolHandle;
use std::hash::{Hash, Hasher};

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
mod tests {
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
                state: SymbolHandle::from_parts(2, 2),
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
