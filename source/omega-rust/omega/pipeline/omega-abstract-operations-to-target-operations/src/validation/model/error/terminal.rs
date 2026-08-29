//! Terminal scalar replay errors.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineScalarCrashTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    TargetProvenance,
    TargetOperation,
}
