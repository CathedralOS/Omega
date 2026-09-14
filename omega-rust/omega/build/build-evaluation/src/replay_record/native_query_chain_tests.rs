//! Transport coverage for the bounded Source native-handle query-release
//! chain (`open_path_handle` / `final_path_name_by_handle`+`get_last_error` /
//! `close_handle`, tags `28;(31|35)+;29`) through canonical record encoding,
//! recovery, and typed rehydration.

use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath,
    BuildFilesystemLogicalHandleIdentity, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemLogicalHandleOutputSource,
    BuildFilesystemMutableByteOperand, BuildFilesystemMutableByteOperandResolution,
    BuildFilesystemReturnedPath, BuildFilesystemReturnedPathCompleteness,
    BuildFilesystemReturnedPathKind, BuildFilesystemRootedPathOperandResolution,
    BuildFilesystemScalarOperand, BuildObservationClass,
};

const HANDLE_IDENTITY: u64 = 7;
const QUERY_CAPACITY: u64 = 260;

fn identity(value: u64) -> BuildFilesystemLogicalHandleIdentity {
    BuildFilesystemLogicalHandleIdentity::new(value).expect("test identity is nonzero")
}

fn attempt(
    operation_tag: u16,
    result: BuildFilesystemOperationResult,
) -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag,
        provider: BuildFilesystemProvider::RealScoped,
        observation_class: crate::BuildFilesystemOperationObservationClass::Receipted,
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

/// Constrained `open_path_handle` acquisition: access zero, full sharing,
/// null security attributes, `OPEN_EXISTING`, `FILE_FLAG_BACKUP_SEMANTICS`,
/// null template, minted Native identity.
fn query_open() -> BuildFilesystemOperationAttempt {
    let mut open = attempt(
        28,
        BuildFilesystemOperationResult::LogicalHandle(identity(HANDLE_IDENTITY)),
    );
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
            relative_path: b"pkg/main.omg".to_vec(),
        });
    open.authorized_paths.push(BuildFilesystemAuthorizedPath {
        operand_ordinal: 0,
        access: BuildFilesystemGrantAccess::Read,
        root: BuildFilesystemRoot::Source,
        relative_path: b"pkg/main.omg".to_vec(),
    });
    open.logical_handle_inputs
        .push(BuildFilesystemLogicalHandleInput {
            operand_ordinal: 6,
            kind: BuildFilesystemLogicalHandleKind::Native,
            resolution: BuildFilesystemLogicalHandleInputResolution::Null,
        });
    open.logical_handle_output = Some(BuildFilesystemLogicalHandleOutput {
        kind: BuildFilesystemLogicalHandleKind::Native,
        identity: identity(HANDLE_IDENTITY),
        source: BuildFilesystemLogicalHandleOutputSource::Created,
    });
    open
}

/// Handle-free `get_last_error` read of a clear error slot.
fn last_error_read() -> BuildFilesystemOperationAttempt {
    attempt(35, BuildFilesystemOperationResult::Scalar(0))
}

/// One `final_path_name_by_handle` observation on the chain identity: the
/// resolved carrier precedes the call and the post state carries the
/// returned path plus its NUL terminator over an unchanged tail.
fn final_path_query() -> BuildFilesystemOperationAttempt {
    let path = b"C:\\pkg\\main.omg".to_vec();
    let mut post_bytes = path.clone();
    post_bytes.push(0);
    post_bytes.resize(usize::try_from(QUERY_CAPACITY).unwrap(), 0);
    let mut query = attempt(
        31,
        BuildFilesystemOperationResult::Scalar(
            i64::try_from(path.len()).expect("test path length fits"),
        ),
    );
    query.scalar_operands = vec![
        BuildFilesystemScalarOperand {
            operand_ordinal: 2,
            value: BuildFilesystemScalarOperandValue::U64(QUERY_CAPACITY),
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
        bytes: path,
    });
    query
        .mutable_byte_operand_resolutions
        .push(BuildFilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: vec![0; usize::try_from(QUERY_CAPACITY).unwrap()],
        });
    query
        .mutable_byte_operands
        .push(BuildFilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: vec![0; usize::try_from(QUERY_CAPACITY).unwrap()],
            post_bytes,
        });
    query
        .logical_handle_inputs
        .push(BuildFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: BuildFilesystemLogicalHandleKind::Native,
            resolution: BuildFilesystemLogicalHandleInputResolution::Resolved(identity(
                HANDLE_IDENTITY,
            )),
        });
    query
}

/// Successful `close_handle` that retires the chain identity.
fn query_close() -> BuildFilesystemOperationAttempt {
    let mut close = attempt(29, BuildFilesystemOperationResult::Scalar(1));
    close
        .logical_handle_inputs
        .push(BuildFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: BuildFilesystemLogicalHandleKind::Native,
            resolution: BuildFilesystemLogicalHandleInputResolution::Resolved(identity(
                HANDLE_IDENTITY,
            )),
        });
    close
        .retired_logical_handles
        .push(identity(HANDLE_IDENTITY));
    close
}

fn chain_attempts() -> Vec<BuildFilesystemOperationAttempt> {
    vec![
        query_open(),
        last_error_read(),
        final_path_query(),
        last_error_read(),
        query_close(),
    ]
}

fn minimal_chain_attempts() -> Vec<BuildFilesystemOperationAttempt> {
    vec![query_open(), final_path_query(), query_close()]
}

fn summary(attempts: Vec<BuildFilesystemOperationAttempt>) -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: attempts,
        canonical_source_metadata_identity: None,
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

fn chain_summary() -> BuildObservationSummary {
    summary(chain_attempts())
}

#[test]
fn native_query_chain_record_round_trips_full_and_minimal_chains() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    for (fixture, expected_tags) in [
        (chain_attempts(), vec![28, 35, 31, 35, 29]),
        (minimal_chain_attempts(), vec![28, 31, 29]),
    ] {
        let attempt_count = expected_tags.len();
        let captured = capture_verified_build_filesystem_replay_record(&summary(fixture), limits)
            .expect("native-handle query chain encodes")
            .expect("verified query chain retains replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("query chain record recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("query chain rehydrates through the typed record");
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            expected_tags
        );
        assert_eq!(
            crate::source_input_replay_prefix_end(replay.attempts()),
            Some(attempt_count)
        );
        assert!(!replay.has_output_attempts());

        let open = &replay.attempts()[0];
        let Some(checked_interpreter::FilesystemOperationResult::LogicalHandle(open_identity)) =
            open.result()
        else {
            panic!("native-handle query open mints one handle")
        };
        assert_eq!(open_identity.get(), HANDLE_IDENTITY);
        let query = replay
            .attempts()
            .iter()
            .find(|attempt| attempt.operation_tag() == 31)
            .expect("query chain retains the final-path query");
        assert_eq!(query.returned_paths()[0].bytes(), b"C:\\pkg\\main.omg");
        let checked_interpreter::FilesystemLogicalHandleInputResolution::Resolved(query_identity) =
            query.logical_handle_inputs()[0].resolution()
        else {
            panic!("final-path query input resolves the chain identity")
        };
        assert_eq!(query_identity.get(), HANDLE_IDENTITY);
        let close = replay.attempts().last().expect("query chain has a close");
        let [retired] = close.retired_logical_handles() else {
            panic!("native-handle query close retires exactly one identity")
        };
        assert_eq!(retired.get(), HANDLE_IDENTITY);
    }
}

#[test]
fn native_query_chain_record_rejects_relaxed_acquisition_lanes() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    for (ordinal, value) in [
        (1u8, BuildFilesystemScalarOperandValue::U32(1)),
        (2, BuildFilesystemScalarOperandValue::U32(0x3)),
        (3, BuildFilesystemScalarOperandValue::I64(4)),
        (4, BuildFilesystemScalarOperandValue::U32(4)),
        (5, BuildFilesystemScalarOperandValue::U32(0x0600_0000)),
    ] {
        let mut tampered = chain_summary();
        tampered.filesystem_operation_attempts[0].scalar_operands[usize::from(ordinal - 1)].value =
            value;
        assert!(
            capture_verified_build_filesystem_replay_record(&tampered, limits).is_err(),
            "acquisition scalar {ordinal} must stay on the constrained contract"
        );
    }

    let mut failed_acquisition = chain_summary();
    failed_acquisition.filesystem_operation_attempts[0].result =
        BuildFilesystemOperationResult::Scalar(-1);
    assert!(capture_verified_build_filesystem_replay_record(&failed_acquisition, limits).is_err());

    // An Output-rooted acquisition declines custody under the
    // SourceInputsOnly verdict rather than retaining a record.
    let mut output_root = chain_summary();
    output_root.filesystem_operation_attempts[0].rooted_path_operand_resolutions[0].root =
        BuildFilesystemRoot::Output;
    assert!(!matches!(
        capture_verified_build_filesystem_replay_record(&output_root, limits),
        Ok(Some(_))
    ));

    // Rooted and authorized Source paths must agree byte-for-byte.
    let mut mismatched_paths = chain_summary();
    mismatched_paths.filesystem_operation_attempts[0].authorized_paths[0].relative_path =
        b"pkg/other.omg".to_vec();
    assert!(capture_verified_build_filesystem_replay_record(&mismatched_paths, limits).is_err());

    let mut write_access = chain_summary();
    write_access.filesystem_operation_attempts[0].authorized_paths[0].access =
        BuildFilesystemGrantAccess::Write;
    assert!(capture_verified_build_filesystem_replay_record(&write_access, limits).is_err());

    let mut aliased_template = chain_summary();
    aliased_template.filesystem_operation_attempts[0].logical_handle_inputs[0].resolution =
        BuildFilesystemLogicalHandleInputResolution::Resolved(identity(9));
    assert!(capture_verified_build_filesystem_replay_record(&aliased_template, limits).is_err());

    let mut borrowed_output = chain_summary();
    borrowed_output.filesystem_operation_attempts[0].logical_handle_output =
        Some(BuildFilesystemLogicalHandleOutput {
            kind: BuildFilesystemLogicalHandleKind::Native,
            identity: identity(HANDLE_IDENTITY),
            source: BuildFilesystemLogicalHandleOutputSource::Borrowed(identity(9)),
        });
    assert!(capture_verified_build_filesystem_replay_record(&borrowed_output, limits).is_err());

    let mut descriptor_output = chain_summary();
    descriptor_output.filesystem_operation_attempts[0].logical_handle_output =
        Some(BuildFilesystemLogicalHandleOutput {
            kind: BuildFilesystemLogicalHandleKind::Descriptor,
            identity: identity(HANDLE_IDENTITY),
            source: BuildFilesystemLogicalHandleOutputSource::Created,
        });
    assert!(capture_verified_build_filesystem_replay_record(&descriptor_output, limits).is_err());

    let mut noncanonical_path = chain_summary();
    noncanonical_path.filesystem_operation_attempts[0].rooted_path_operand_resolutions[0]
        .relative_path = b"../escape".to_vec();
    assert!(capture_verified_build_filesystem_replay_record(&noncanonical_path, limits).is_err());
}

#[test]
fn native_query_chain_record_rejects_lineage_and_release_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    // A substituted intervening input breaks the proven lineage.
    let mut substituted = chain_summary();
    substituted.filesystem_operation_attempts[2].logical_handle_inputs[0].resolution =
        BuildFilesystemLogicalHandleInputResolution::Resolved(identity(9));
    assert!(capture_verified_build_filesystem_replay_record(&substituted, limits).is_err());

    // A cross-domain intervening input is not this handle.
    let mut cross_domain = chain_summary();
    cross_domain.filesystem_operation_attempts[2].logical_handle_inputs[0].kind =
        BuildFilesystemLogicalHandleKind::Descriptor;
    assert!(capture_verified_build_filesystem_replay_record(&cross_domain, limits).is_err());

    // An intervening handle output is an escape, not a preserving query.
    let mut escape = chain_summary();
    escape.filesystem_operation_attempts[2].logical_handle_output =
        Some(BuildFilesystemLogicalHandleOutput {
            kind: BuildFilesystemLogicalHandleKind::Native,
            identity: identity(9),
            source: BuildFilesystemLogicalHandleOutputSource::Duplicated(identity(HANDLE_IDENTITY)),
        });
    assert!(capture_verified_build_filesystem_replay_record(&escape, limits).is_err());

    // An early retirement invalidates the later close.
    let mut early_retire = chain_summary();
    early_retire.filesystem_operation_attempts[2]
        .retired_logical_handles
        .push(identity(HANDLE_IDENTITY));
    assert!(capture_verified_build_filesystem_replay_record(&early_retire, limits).is_err());

    // A failed close is not the proven release.
    let mut failed_close = chain_summary();
    failed_close.filesystem_operation_attempts[4].result =
        BuildFilesystemOperationResult::Scalar(0);
    failed_close.filesystem_operation_attempts[4].post_error = 6;
    assert!(capture_verified_build_filesystem_replay_record(&failed_close, limits).is_err());

    // A close that does not retire the identity is not the proven release.
    let mut unretired = chain_summary();
    unretired.filesystem_operation_attempts[4]
        .retired_logical_handles
        .clear();
    assert!(capture_verified_build_filesystem_replay_record(&unretired, limits).is_err());

    // A use after the release is a later event the record cannot attribute.
    let mut late_use = chain_summary();
    late_use
        .filesystem_operation_attempts
        .push(final_path_query());
    assert!(capture_verified_build_filesystem_replay_record(&late_use, limits).is_err());

    // Two chains cannot share one identity.
    let mut reused = chain_attempts();
    reused.extend(chain_attempts());
    assert!(capture_verified_build_filesystem_replay_record(&summary(reused), limits).is_err());
}

#[test]
fn native_query_chain_record_rejects_incomplete_and_tampered_queries() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    // No release at all.
    let mut missing_close = chain_attempts();
    missing_close.pop();
    assert!(
        capture_verified_build_filesystem_replay_record(&summary(missing_close), limits).is_err()
    );

    // A chain without a final-path query is not this family.
    let mut no_query = chain_attempts();
    no_query.remove(2);
    assert!(capture_verified_build_filesystem_replay_record(&summary(no_query), limits).is_err());

    // The error-slot read must return the slot it observes.
    let mut error_read = chain_summary();
    error_read.filesystem_operation_attempts[1].result = BuildFilesystemOperationResult::Scalar(5);
    assert!(capture_verified_build_filesystem_replay_record(&error_read, limits).is_err());

    // The result must equal the returned path length.
    let mut wrong_result = chain_summary();
    wrong_result.filesystem_operation_attempts[2].result =
        BuildFilesystemOperationResult::Scalar(16);
    assert!(capture_verified_build_filesystem_replay_record(&wrong_result, limits).is_err());

    // The post state must carry the returned path with its terminator.
    let mut bad_post = chain_summary();
    bad_post.filesystem_operation_attempts[2].mutable_byte_operands[0].post_bytes[14] = 1;
    assert!(capture_verified_build_filesystem_replay_record(&bad_post, limits).is_err());

    // The resolved snapshot must equal the pre-call carrier.
    let mut bad_resolution = chain_summary();
    bad_resolution.filesystem_operation_attempts[2].mutable_byte_operand_resolutions[0].bytes[1] ^=
        1;
    assert!(capture_verified_build_filesystem_replay_record(&bad_resolution, limits).is_err());

    // A returned path outside the FinalPath lane is not this query.
    let mut wrong_kind = chain_summary();
    wrong_kind.filesystem_operation_attempts[2].returned_paths[0].kind =
        BuildFilesystemReturnedPathKind::CanonicalPath;
    assert!(capture_verified_build_filesystem_replay_record(&wrong_kind, limits).is_err());

    // A capacity that cannot hold the path terminator is inconsistent.
    let mut tight_capacity = chain_summary();
    tight_capacity.filesystem_operation_attempts[2].scalar_operands[0].value =
        BuildFilesystemScalarOperandValue::U64(15);
    assert!(capture_verified_build_filesystem_replay_record(&tight_capacity, limits).is_err());
}
