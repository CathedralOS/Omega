//! Stage-level contract with the wrapper object's record owner. Record
//! construction, shape, codec, and custody mutation coverage lives beside the
//! records in `native_artifact::semantic_wrapper_object`; the end-to-end stage
//! route is exercised by the native-realization emission tests.

use super::OptimizedProgramStorageSemanticWrapperObjectError as Stage;
use isa_x86_64::X86_64SemanticUnitWrapperResolutionError;
use native_artifact::OptimizedProgramStorageSemanticWrapperObjectRecordError as Record;

#[test]
fn record_failures_surface_as_their_same_named_stage_variants() {
    for (record, stage) in [
        (Record::LengthOverflow, Stage::LengthOverflow),
        (Record::InvalidObject, Stage::InvalidObject),
        (Record::ManifestMismatch, Stage::ManifestMismatch),
        (Record::SourceObjectMismatch, Stage::SourceObjectMismatch),
        (
            Record::WrapperResolution(
                X86_64SemanticUnitWrapperResolutionError::RelativeDisplacementOutOfRange,
            ),
            Stage::WrapperResolution(
                X86_64SemanticUnitWrapperResolutionError::RelativeDisplacementOutOfRange,
            ),
        ),
    ] {
        assert_eq!(Stage::from(record), stage);
    }
}
