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

optimization_core::invalid_error_display!(
    InvalidPsiTransformationLedger,
    "Psi transformation ledger"
);

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

optimization_core::invalid_error_display!(
    PsiTransformationLedgerDecodeError,
    "Psi transformation-ledger encoding"
);

optimization_core::take_error_from_adapter!(PsiTransformationLedgerDecodeError);
