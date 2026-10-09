use super::{MachineFunctionIdentity, SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn caller(byte: u8) -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([byte; 32]),
    }
}

#[test]
fn callback_identity_is_scoped_to_the_terminal_program_and_placement() {
    let first = MachineFunctionIdentity::callback_thunk(caller(1), 7).unwrap();
    let other_program = MachineFunctionIdentity::callback_thunk(caller(2), 7).unwrap();
    let other_placement = MachineFunctionIdentity::callback_thunk(caller(1), 8).unwrap();
    for different in [other_program, other_placement] {
        assert_ne!(first, different);
        assert_ne!(first.to_bytes(), different.to_bytes());
        assert_ne!(first.private_symbol(), different.private_symbol());
    }
    assert_eq!(first.caller(), caller(1));
    assert_eq!(first.callback_thunk_placement_index(), Some(7));
    assert_eq!(
        MachineFunctionIdentity::from_bytes(first.to_bytes()),
        Some(first)
    );
}

#[test]
fn absent_and_unknown_vocabulary_identities_reject() {
    assert!(MachineFunctionIdentity::callback_thunk(caller(0), 0).is_none());
    let absent = MachineFunctionIdentity::default();
    assert!(!absent.is_valid());
    assert!(absent.private_symbol().is_none());
    assert!(MachineFunctionIdentity::from_bytes(absent.to_bytes()).is_none());
    let identity = MachineFunctionIdentity::callback_thunk(caller(1), 0).unwrap();
    let mut bytes = identity.to_bytes();
    bytes[0] = 0;
    bytes[1] = 0;
    assert!(MachineFunctionIdentity::from_bytes(bytes).is_none());
}
