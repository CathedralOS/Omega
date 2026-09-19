//! Compiler-independent fixtures for downstream boundary tests.

use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath,
    BuildFilesystemLogicalHandleIdentity, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemLogicalHandleOutputSource,
    BuildFilesystemOperationAttempt, BuildFilesystemOperationResult, BuildFilesystemProvider,
    BuildFilesystemRoot, BuildObservationSummary,
};

/// One retained failed descriptor operation with exact unknown-handle
/// custody. The build-evaluation owner constructs this value so downstream
/// codecs need not revive retired source-language filesystem authority.
pub fn unknown_descriptor_summary() -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        filesystem_host_observed: true,
        filesystem_operation_schema_version:
            checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: vec![BuildFilesystemOperationAttempt {
            operation_tag: 44,
            provider: BuildFilesystemProvider::RealScoped,
            result: BuildFilesystemOperationResult::Scalar(-1),
            post_error: 9,
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
        activation: crate::BuildActivation::default(),
        captured_source_inventory: None,
        included_source_handoffs: Vec::new(),
        required_output_settlements: Vec::new(),
        staged_output_tree: None,
        build_log: Vec::new(),
    }
}

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
        result,
        post_error: 0,
        authorized_paths: Vec::new(),
        logical_handle_inputs: Vec::new(),
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

/// Open a Source-rooted native handle with a fresh logical identity.
fn query_chain_open(
    relative_path: &[u8],
    identity: BuildFilesystemLogicalHandleIdentity,
) -> BuildFilesystemOperationAttempt {
    let mut open = query_chain_attempt(28, BuildFilesystemOperationResult::LogicalHandle(identity));
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

/// Query result associated with the same native logical handle.
fn query_chain_final_path(
    final_path: &[u8],
    identity: BuildFilesystemLogicalHandleIdentity,
) -> BuildFilesystemOperationAttempt {
    let mut query = query_chain_attempt(
        31,
        BuildFilesystemOperationResult::Scalar(
            i64::try_from(final_path.len()).expect("test path length fits"),
        ),
    );
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
/// retains the observed operation evidence, and a
/// distinct path or identity produces a distinct occurrence commitment.
pub fn native_handle_query_chain_summary(occurrences: &[(&[u8], u64)]) -> BuildObservationSummary {
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
        filesystem_host_observed: true,
        filesystem_operation_schema_version:
            checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: attempts,
        canonical_source_metadata_identity: None,
        activation: crate::BuildActivation::default(),
        captured_source_inventory: None,
        included_source_handoffs: Vec::new(),
        required_output_settlements: Vec::new(),
        staged_output_tree: None,
        build_log: Vec::new(),
    }
}
