use crate::OptimizedProgramStorageSemanticWrapperObjectRecordError;
use isa_x86_64::X86_64SemanticUnitWrapperResolutionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OptimizedProgramStorageSemanticWrapperObjectError {
    MissingPairedCallingPlans,
    SemanticContract(String),
    TargetMismatch,
    TerminalEntryShapeMismatch,
    SourceObjectMismatch,
    WrapperResolution(X86_64SemanticUnitWrapperResolutionError),
    LengthOverflow,
    InvalidObject,
    ManifestMismatch,
}

diagnostics::failed_display!(
    OptimizedProgramStorageSemanticWrapperObjectError,
    "optimized ProgramStorage semantic wrapper object failed"
);

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
