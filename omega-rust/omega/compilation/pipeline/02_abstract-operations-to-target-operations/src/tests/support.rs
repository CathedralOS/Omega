//! Shared scalar fixture projections used across lowering test families.

use super::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
pub(crate) fn identity() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
    }
}
