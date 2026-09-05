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
