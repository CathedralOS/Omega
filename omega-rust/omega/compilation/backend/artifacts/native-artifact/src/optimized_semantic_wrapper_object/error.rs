use crate::OptimizedProgramStorageSemanticWrapperObjectRecordError;
use isa_x86_64::X86_64SemanticUnitWrapperResolutionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OptimizedProgramStorageSemanticWrapperObjectError {
    MissingPairedCallingPlans,
    SemanticContract,
    TargetMismatch,
    TerminalEntryShapeMismatch,
    SourceObjectMismatch,
    WrapperResolution(X86_64SemanticUnitWrapperResolutionError),
    LengthOverflow,
    InvalidObject,
    ManifestMismatch,
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


