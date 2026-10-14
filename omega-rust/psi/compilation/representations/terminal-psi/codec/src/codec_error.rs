//! The codec's single error type and its malformed-foundation constructor.

use semantic_vocabulary::PropositionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    InvalidMagic,
    UnsupportedFormatMarker(u16),
    UnsupportedVocabularyMarker(u16),
    UnexpectedEnd,
    TrailingBytes(usize),
    InvalidBoolean(u8),
    InvalidTag(&'static str, u8),
    ZeroIdentity(&'static str),
    CollectionTooLong(&'static str),
    NonCanonicalOrder(&'static str),
    NonCanonicalEncoding,
    NestedConjunction,
    NestedDisjunction,
    PropositionNestingTooDeep,
    ScalarTermNestingTooDeep,
    ContentTermNestingTooDeep,
    StringTooLong(&'static str),
    InvalidUtf8(&'static str),
    MalformedStructuralFoundation(&'static str),
    PlacementPlan(String),
    PlacedPlan(terminal_semantics::PlacedPlanError),
    PlacedRamCorrespondence(String),
    PlacedViewInput(terminal_semantics::PlacedViewInputError),
    MalformedMathematicalCertificate(&'static str),
    MalformedProposition(PropositionError),
    ObligationLedgerMismatch,
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CodecError {}

pub(crate) fn malformed<T>(message: &'static str) -> Result<T, CodecError> {
    Err(CodecError::MalformedStructuralFoundation(message))
}
