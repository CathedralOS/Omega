//! The proof codec error.

use semantic_vocabulary::PropositionError;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofCodecError {
    InvalidMagic,
    UnsupportedFormatMarker(u16),
    UnsupportedProofSystemMarker(u16),
    UnknownIntegerAffineLiteralTag(u8),
    UnknownIntegerCorrelatedAffineLiteralTag(u8),
    UnexpectedEnd,
    TrailingBytes(usize),
    InvalidBoolean(u8),
    InvalidTag(&'static str, u8),
    ZeroIdentity(&'static str),
    CollectionTooLong(&'static str),
    IndexTooLarge(&'static str),
    IndexOutsideHost,
    NonCanonicalEvidenceOrder,
    NonCanonicalRecursiveComponentEvidence,
    NonCanonicalControlCycleEvidence,
    NonCanonicalEvidenceProducerOrder,
    NonCanonicalEvidenceProducerRows,
    InvalidEvidenceProducer,
    NonCanonicalEncoding,
    PropositionNestingTooDeep,
    ScalarTermNestingTooDeep,
    ContentTermNestingTooDeep,
    ProofNestingTooDeep,
    StringTooLong(&'static str),
    InvalidUtf8(&'static str),
    MalformedProposition(PropositionError),
    MalformedMathematicalSignature(&'static str),
    TrustGraph(crate::TrustGraphError),
    SubjectIdentity(crate::CodecError),
    UnsupportedProofSectionVocabulary(u16),
    ProofSubjectMismatch {
        claimed: TerminalPsiIdentity,
        reconstructed: TerminalPsiIdentity,
    },
}

impl std::fmt::Display for ProofCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProofCodecError {}
