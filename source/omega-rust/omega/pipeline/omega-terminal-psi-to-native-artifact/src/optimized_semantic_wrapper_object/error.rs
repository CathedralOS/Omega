use super::shared::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectError {
    Settlement(NativeProgramEntrySettlementError),
    Source(OptimizedObjectArtifactError),
    Encoding(OptimizedProgramStorageSemanticWrapperEncodingError),
    InstalledProviderContinuation(InstalledProgramStorageContinuationEvidenceError),
    MissingPairedCallingPlans,
    SemanticContract,
    SemanticWrapperPlanMismatch,
    TargetMismatch,
    TerminalEntryShapeMismatch,
    SourceObjectMismatch,
    WrapperResolution(X86_64SemanticUnitWrapperResolutionError),
    LengthOverflow,
    InvalidObject,
    ContainerMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedProgramStorageSemanticWrapperObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized ProgramStorage semantic wrapper object failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedProgramStorageSemanticWrapperObjectError {}

/// Diagnostic-only replay failures for the installed, claim-consuming
/// ProgramStorage continuation. Validation of detached clones grants no object
/// or wrapper authority; the owning wrapper stage reruns this same check over
/// its retained opaque installation and selected-plan custody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstalledProgramStorageContinuationEvidenceError {
    RootMismatch,
    FunctionRosterMismatch,
    EntryCallMissing,
    SourceKindMismatch,
    InstallationRosterMismatch,
    ProviderMismatch,
    StructuralContractMismatch,
    CallEvidenceMismatch,
    EntryClaimMismatch,
    ProviderFunctionMismatch,
    ProviderSettlementMismatch,
}

impl std::fmt::Display for InstalledProgramStorageContinuationEvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "installed ProgramStorage continuation evidence failed: {self:?}"
        )
    }
}

impl std::error::Error for InstalledProgramStorageContinuationEvidenceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidUtf8,
    InvalidLength,
    InvalidSymbol,
    InvalidMachine,
    InvalidVocabulary,
    InvalidTarget,
    UnknownTag,
    IdentityMismatch,
    InvalidObject,
    TrailingBytes,
}

impl std::fmt::Display for OptimizedProgramStorageSemanticWrapperObjectDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid optimized ProgramStorage wrapper object encoding: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedProgramStorageSemanticWrapperObjectDecodeError {}
