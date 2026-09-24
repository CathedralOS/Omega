use super::{MachineFunctionIdentity, StateKey};
use symbols::SymbolHandle;

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
    assert!(MachineFunctionIdentity::program_storage_entry_wrapper(StateKey::default()).is_none());
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
