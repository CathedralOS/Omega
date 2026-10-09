//! Closed construction and decoding failure surfaces.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidPsiTransformationLedger {
    BrokenRevisionChain,
    FinalRevisionMismatch,
    DuplicateCandidate,
    EmptyProvenance,
    NonCanonicalProvenance,
    FuelProvenanceMismatch,
    ZeroFuelSettlement,
    NonCanonicalMachineRoster,
    DuplicatePrunedMachine,
    DuplicatePrunedSourceOrdinal,
}

impl std::fmt::Display for InvalidPsiTransformationLedger {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid Psi transformation ledger: {self:?}")
    }
}

impl std::error::Error for InvalidPsiTransformationLedger {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsiTransformationLedgerDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVocabulary(u16),
    InvalidFuelSchedule,
    InvalidSemanticIdentity,
    UnknownProvenanceTag(u8),
    UnknownRealizationSiteTag(u8),
    UnknownDispositionTag(u8),
    LengthOverflow,
    TrailingBytes,
    InvalidLedger(InvalidPsiTransformationLedger),
}

impl std::fmt::Display for PsiTransformationLedgerDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Psi transformation-ledger encoding: {self:?}"
        )
    }
}

impl std::error::Error for PsiTransformationLedgerDecodeError {}

impl From<crate::cursor::TakeError> for PsiTransformationLedgerDecodeError {
    fn from(error: crate::cursor::TakeError) -> Self {
        match error {
            crate::cursor::TakeError::Truncated => Self::Truncated,
            crate::cursor::TakeError::LengthOverflow => Self::LengthOverflow,
        }
    }
}
