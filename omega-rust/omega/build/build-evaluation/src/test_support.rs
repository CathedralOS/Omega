//! Compiler-independent fixtures for downstream boundary tests.

use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemOperationAttempt, BuildFilesystemOperationObservationClass,
    BuildFilesystemOperationResult, BuildFilesystemProvider, BuildFilesystemReplayDisposition,
    BuildFilesystemReplayVerdict, BuildObservationClass, BuildObservationSummary,
};

/// One replayable failed descriptor operation with exact unknown-handle
/// custody. The build-evaluation owner constructs this value so downstream
/// codecs need not revive retired source-language filesystem authority.
pub fn replayable_unknown_descriptor_summary() -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: vec![BuildFilesystemOperationAttempt {
            operation_tag: 44,
            provider: BuildFilesystemProvider::RealScoped,
            observation_class: BuildFilesystemOperationObservationClass::Receipted,
            result: BuildFilesystemOperationResult::Scalar(-1),
            post_error: 9,
            scalar_operands: Vec::new(),
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: Vec::new(),
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: Vec::new(),
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: vec![BuildFilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: BuildFilesystemLogicalHandleKind::Descriptor,
                resolution: BuildFilesystemLogicalHandleInputResolution::Unknown,
            }],
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        }],
        canonical_source_metadata_identity: None,
        filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
            BuildFilesystemReplayDisposition::SourceInputsOnly,
        ),
        included_source_handoffs: Vec::new(),
        staged_output_tree: None,
        build_log: Vec::new(),
    }
}
