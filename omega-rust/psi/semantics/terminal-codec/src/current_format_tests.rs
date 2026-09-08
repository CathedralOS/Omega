//! Current-only admission preserves exact canonical bytes and refuses migration.
use super::{CodecError, FORMAT_MARKER, decode_module, encode_module};
use terminal_psi::VocabularyMarker;

// Captured from d743b4e805 before retiring its legacy encoder. This is a
// complete previously accepted artifact, not a current body with stale markers.
const LEGACY_UNIT: &[u8] = &[
    80, 83, 73, 84, 69, 82, 77, 0, 56, 0, 59, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0,
    0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

// The same Unit body with current format/vocabulary markers; its payload is unchanged.
const CURRENT_UNIT: &[u8] = &[
    80, 83, 73, 84, 69, 82, 77, 0, 82, 0, 88, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 5, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[test]
fn complete_legacy_artifact_rejects_without_migration() {
    assert_eq!(
        decode_module(LEGACY_UNIT),
        Err(CodecError::UnsupportedFormatMarker(56))
    );
    let mut relabeled = LEGACY_UNIT.to_vec();
    relabeled[8..10].copy_from_slice(&FORMAT_MARKER.to_le_bytes());
    relabeled[10..12].copy_from_slice(&VocabularyMarker::CURRENT.get().to_le_bytes());
    assert!(
        decode_module(&relabeled).is_err(),
        "markers cannot repair missing current rosters"
    );
}

#[test]
fn current_unit_golden_bytes_remain_identical() {
    let module = decode_module(CURRENT_UNIT).expect("current Unit fixture");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    assert_eq!(module.machines.len(), 1);
    assert_eq!(
        module.machines[0].result,
        terminal_psi::TerminalMachineResult::Unit
    );
    assert_eq!(encode_module(&module).unwrap(), CURRENT_UNIT);
}

#[test]
fn every_noncurrent_format_and_vocabulary_marker_rejects() {
    let mut header = CURRENT_UNIT[..12].to_vec();
    for marker in 0..=u16::MAX {
        if marker != FORMAT_MARKER {
            header[8..10].copy_from_slice(&marker.to_le_bytes());
            assert_eq!(
                decode_module(&header),
                Err(CodecError::UnsupportedFormatMarker(marker))
            );
        }
    }
    header[8..10].copy_from_slice(&FORMAT_MARKER.to_le_bytes());
    for marker in 0..=u16::MAX {
        if marker != VocabularyMarker::CURRENT.get() {
            header[10..12].copy_from_slice(&marker.to_le_bytes());
            assert_eq!(
                decode_module(&header),
                Err(CodecError::UnsupportedVocabularyMarker(marker))
            );
        }
    }
}

#[test]
fn crossed_markers_truncation_and_trailing_bytes_reject() {
    for format in [
        0,
        56,
        FORMAT_MARKER - 1,
        FORMAT_MARKER,
        FORMAT_MARKER + 1,
        u16::MAX,
    ] {
        for vocabulary in [
            0,
            59,
            VocabularyMarker::CURRENT.get() - 1,
            VocabularyMarker::CURRENT.get(),
            VocabularyMarker::CURRENT.get() + 1,
            u16::MAX,
        ] {
            let mut bytes = CURRENT_UNIT.to_vec();
            bytes[8..10].copy_from_slice(&format.to_le_bytes());
            bytes[10..12].copy_from_slice(&vocabulary.to_le_bytes());
            assert_eq!(
                decode_module(&bytes).is_ok(),
                format == FORMAT_MARKER && vocabulary == VocabularyMarker::CURRENT.get()
            );
        }
    }
    for length in 0..CURRENT_UNIT.len() {
        assert!(
            decode_module(&CURRENT_UNIT[..length]).is_err(),
            "truncation at {length}"
        );
    }
    let mut trailing = CURRENT_UNIT.to_vec();
    trailing.push(0);
    assert_eq!(decode_module(&trailing), Err(CodecError::TrailingBytes(1)));
}

#[test]
fn portable_envelope_cannot_hide_legacy_semantics() {
    // Use the public envelope decoder: semantic admission precedes proof and
    // optimization admission, even when those sections have no bytes.
    let mut envelope = b"PSIART\0\0".to_vec();
    envelope.extend_from_slice(&2_u16.to_le_bytes());
    envelope.extend_from_slice(&(LEGACY_UNIT.len() as u64).to_le_bytes());
    envelope.extend_from_slice(&0_u64.to_le_bytes());
    envelope.extend_from_slice(&0_u64.to_le_bytes());
    envelope.push(0);
    envelope.extend_from_slice(LEGACY_UNIT);
    assert!(matches!(
        super::CanonicalTerminalArtifact::from_bytes(&envelope),
        Err(super::CanonicalTerminalArtifactError::Semantic(
            CodecError::UnsupportedFormatMarker(56)
        ))
    ));
}
