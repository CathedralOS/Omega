//! Compiler-independent fixtures for downstream boundary tests.

use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath,
    BuildFilesystemLogicalHandleIdentity, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemLogicalHandleOutputSource,
    BuildFilesystemMutableByteOperand, BuildFilesystemMutableByteOperandResolution,
    BuildFilesystemOperationAttempt, BuildFilesystemOperationObservationClass,
    BuildFilesystemOperationResult, BuildFilesystemProvider, BuildFilesystemReplayDisposition,
    BuildFilesystemReplayVerdict, BuildFilesystemReturnedPath,
    BuildFilesystemReturnedPathCompleteness, BuildFilesystemReturnedPathKind, BuildFilesystemRoot,
    BuildFilesystemRootedPathOperandResolution, BuildFilesystemScalarOperand,
    BuildFilesystemScalarOperandValue, BuildObservationClass, BuildObservationSummary,
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
        replay_activation: crate::BuildReplayActivation::default(),
        captured_source_inventory: None,
        filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
            BuildFilesystemReplayDisposition::SourceInputsOnly,
        ),
        included_source_handoffs: Vec::new(),
        required_output_settlements: Vec::new(),
        staged_output_tree: None,
        build_log: Vec::new(),
    }
}

/// Query-buffer capacity carried by the fixture's `final_path_name_by_handle`
/// observation.
const QUERY_CHAIN_CAPACITY: u64 = 260;

fn query_chain_identity(value: u64) -> BuildFilesystemLogicalHandleIdentity {
    BuildFilesystemLogicalHandleIdentity::new(value).expect("test chain identity is nonzero")
}

fn query_chain_attempt(
    operation_tag: u16,
    result: BuildFilesystemOperationResult,
) -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag,
        provider: BuildFilesystemProvider::RealScoped,
        observation_class: BuildFilesystemOperationObservationClass::Receipted,
        result,
        post_error: 0,
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
        logical_handle_inputs: Vec::new(),
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

/// Constrained `open_path_handle` acquisition: access zero, full
/// read/write/delete sharing, null security attributes, `OPEN_EXISTING`,
/// `FILE_FLAG_BACKUP_SEMANTICS` without `FILE_FLAG_DELETE_ON_CLOSE`, a null
/// template input, and a freshly minted Native logical handle.
fn query_chain_open(
    relative_path: &[u8],
    identity: BuildFilesystemLogicalHandleIdentity,
) -> BuildFilesystemOperationAttempt {
    let mut open = query_chain_attempt(28, BuildFilesystemOperationResult::LogicalHandle(identity));
    open.scalar_operands = vec![
        BuildFilesystemScalarOperand {
            operand_ordinal: 1,
            value: BuildFilesystemScalarOperandValue::U32(0),
        },
        BuildFilesystemScalarOperand {
            operand_ordinal: 2,
            value: BuildFilesystemScalarOperandValue::U32(0x7),
        },
        BuildFilesystemScalarOperand {
            operand_ordinal: 3,
            value: BuildFilesystemScalarOperandValue::I64(0),
        },
        BuildFilesystemScalarOperand {
            operand_ordinal: 4,
            value: BuildFilesystemScalarOperandValue::U32(3),
        },
        BuildFilesystemScalarOperand {
            operand_ordinal: 5,
            value: BuildFilesystemScalarOperandValue::U32(0x0200_0000),
        },
    ];
    open.rooted_path_operand_resolutions
        .push(BuildFilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: BuildFilesystemRoot::Source,
            relative_path: relative_path.to_vec(),
        });
    open.authorized_paths.push(BuildFilesystemAuthorizedPath {
        operand_ordinal: 0,
        access: crate::BuildFilesystemGrantAccess::Read,
        root: BuildFilesystemRoot::Source,
        relative_path: relative_path.to_vec(),
    });
    open.logical_handle_inputs
        .push(BuildFilesystemLogicalHandleInput {
            operand_ordinal: 6,
            kind: BuildFilesystemLogicalHandleKind::Native,
            resolution: BuildFilesystemLogicalHandleInputResolution::Null,
        });
    open.logical_handle_output = Some(BuildFilesystemLogicalHandleOutput {
        kind: BuildFilesystemLogicalHandleKind::Native,
        identity,
        source: BuildFilesystemLogicalHandleOutputSource::Created,
    });
    open
}

/// One `final_path_name_by_handle` observation that preserves the chain
/// identity: the resolved carrier precedes the call and the post state
/// carries the returned path plus its NUL terminator over an unchanged tail.
fn query_chain_final_path(
    final_path: &[u8],
    identity: BuildFilesystemLogicalHandleIdentity,
) -> BuildFilesystemOperationAttempt {
    let mut post_bytes = final_path.to_vec();
    post_bytes.push(0);
    post_bytes.resize(usize::try_from(QUERY_CHAIN_CAPACITY).unwrap(), 0);
    let mut query = query_chain_attempt(
        31,
        BuildFilesystemOperationResult::Scalar(
            i64::try_from(final_path.len()).expect("test path length fits"),
        ),
    );
    query.scalar_operands = vec![
        BuildFilesystemScalarOperand {
            operand_ordinal: 2,
            value: BuildFilesystemScalarOperandValue::U64(QUERY_CHAIN_CAPACITY),
        },
        BuildFilesystemScalarOperand {
            operand_ordinal: 3,
            value: BuildFilesystemScalarOperandValue::U32(0),
        },
    ];
    query.returned_paths.push(BuildFilesystemReturnedPath {
        operand_ordinal: 1,
        kind: BuildFilesystemReturnedPathKind::FinalPath,
        completeness: BuildFilesystemReturnedPathCompleteness::Complete,
        bytes: final_path.to_vec(),
    });
    query
        .mutable_byte_operand_resolutions
        .push(BuildFilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: vec![0; usize::try_from(QUERY_CHAIN_CAPACITY).unwrap()],
        });
    query
        .mutable_byte_operands
        .push(BuildFilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: vec![0; usize::try_from(QUERY_CHAIN_CAPACITY).unwrap()],
            post_bytes,
        });
    query
        .logical_handle_inputs
        .push(BuildFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: BuildFilesystemLogicalHandleKind::Native,
            resolution: BuildFilesystemLogicalHandleInputResolution::Resolved(identity),
        });
    query
}

/// Successful `close_handle` retiring the chain identity exactly once.
fn query_chain_close(
    identity: BuildFilesystemLogicalHandleIdentity,
) -> BuildFilesystemOperationAttempt {
    let mut close = query_chain_attempt(29, BuildFilesystemOperationResult::Scalar(1));
    close
        .logical_handle_inputs
        .push(BuildFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: BuildFilesystemLogicalHandleKind::Native,
            resolution: BuildFilesystemLogicalHandleInputResolution::Resolved(identity),
        });
    close.retired_logical_handles.push(identity);
    close
}

/// One or more retained bounded Source native-handle query-release
/// occurrences under the constrained query-only contract, one
/// `open_path_handle` / `final_path_name_by_handle` / `get_last_error` /
/// `close_handle` lifecycle per `(relative_path, handle_identity)` pair, in
/// authored order.
///
/// The build-evaluation owner constructs this value so downstream reviewers
/// need not revive retired source-language filesystem authority: each chain
/// is the exact retained evidence a verified replay record carries, and a
/// distinct path or identity produces a distinct occurrence commitment.
pub fn replayable_native_handle_query_chain_summary(
    occurrences: &[(&[u8], u64)],
) -> BuildObservationSummary {
    let mut attempts = Vec::new();
    for &(relative_path, identity_value) in occurrences {
        let identity = query_chain_identity(identity_value);
        let final_path = {
            let mut resolved = b"C:\\".to_vec();
            resolved.extend(relative_path.iter().map(
                |byte| {
                    if *byte == b'/' { b'\\' } else { *byte }
                },
            ));
            resolved
        };
        attempts.push(query_chain_open(relative_path, identity));
        attempts.push(query_chain_final_path(&final_path, identity));
        attempts.push(query_chain_attempt(
            35,
            BuildFilesystemOperationResult::Scalar(0),
        ));
        attempts.push(query_chain_close(identity));
    }
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: attempts,
        canonical_source_metadata_identity: None,
        replay_activation: crate::BuildReplayActivation::default(),
        captured_source_inventory: None,
        filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
            BuildFilesystemReplayDisposition::SourceInputsOnly,
        ),
        included_source_handoffs: Vec::new(),
        required_output_settlements: Vec::new(),
        staged_output_tree: None,
        build_log: Vec::new(),
    }
}
