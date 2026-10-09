use crate::OptimizedProgramStorageSemanticWrapperObjectRecordError;
use isa_x86_64::X86_64SemanticUnitWrapperResolutionError;
use program_entry_plan::OptimizedProgramStorageSemanticWrapperEncodingError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OptimizedProgramStorageSemanticWrapperObjectError {
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

/// Record operations fail with the same-named stage variant, so a failure
/// reads identically whether the record owner or the stage raised it.
impl From<OptimizedProgramStorageSemanticWrapperObjectRecordError>
    for OptimizedProgramStorageSemanticWrapperObjectError
{
    fn from(error: OptimizedProgramStorageSemanticWrapperObjectRecordError) -> Self {
        use OptimizedProgramStorageSemanticWrapperObjectRecordError as Record;
        match error {
            Record::LengthOverflow => Self::LengthOverflow,
            Record::InvalidObject => Self::InvalidObject,
            Record::ManifestMismatch => Self::ManifestMismatch,
            Record::SourceObjectMismatch => Self::SourceObjectMismatch,
            Record::WrapperResolution(error) => Self::WrapperResolution(error),
        }
    }
}

/// Diagnostic-only replay failures for the installed, claim-consuming
/// ProgramStorage continuation. Validation of detached clones grants no object
/// or wrapper authority; the owning wrapper stage reruns this same check over
/// its retained opaque installation and selected-plan custody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstalledProgramStorageContinuationEvidenceError {
    EntryCallMissing,
    SourceKindMismatch,
    InstallationRosterMismatch,
    ProviderMismatch,
    StructuralContractMismatch,
    CallEvidenceMismatch,
    EntryClaimMismatch,
    ProviderFunctionMismatch,
}
