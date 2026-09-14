use super::{
    FilesystemNativeHandleErrorObservationReplayRecord,
    FilesystemNativeHandleFinalPathQueryReplayRecord,
    FilesystemNativeHandleQueryOperationReplayRecord,
    FilesystemSourceNativeHandleQueryChainReplayRecord, source_native_handle_query_chain_is_exact,
};
use crate::{
    EvaluationObservations, FilesystemGrantRootIdentity, FilesystemLogicalHandleIdentity,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutputSource, FilesystemObservationProvider, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemReplay,
    FilesystemScalarOperandValue, FilesystemSourceInputReplayEventRecord,
    FilesystemSourceInputReplayRecord,
};

fn root(value: u32) -> FilesystemGrantRootIdentity {
    FilesystemGrantRootIdentity::new(value).expect("test root is nonzero")
}

fn identity(value: u64) -> FilesystemLogicalHandleIdentity {
    FilesystemLogicalHandleIdentity::new(value).expect("test identity is nonzero")
}

fn final_path_query() -> FilesystemNativeHandleQueryOperationReplayRecord {
    let path = b"C:\\pkg\\main.omg".to_vec();
    let mut post_bytes = path.clone();
    post_bytes.push(0);
    post_bytes.resize(260, 0);
    FilesystemNativeHandleQueryOperationReplayRecord::FinalPathName(
        FilesystemNativeHandleFinalPathQueryReplayRecord::new(
            260,
            0,
            i64::try_from(path.len()).expect("test path length fits"),
            0,
            vec![0; 260],
            vec![0; 260],
            post_bytes,
            path,
        )
        .expect("final-path query is canonical"),
    )
}

fn last_error(error: i32) -> FilesystemNativeHandleQueryOperationReplayRecord {
    FilesystemNativeHandleQueryOperationReplayRecord::LastError(
        FilesystemNativeHandleErrorObservationReplayRecord::new(error),
    )
}

fn chain() -> FilesystemSourceNativeHandleQueryChainReplayRecord {
    FilesystemSourceNativeHandleQueryChainReplayRecord::new(
        root(1),
        b"pkg/main.omg".to_vec(),
        7,
        0,
        vec![last_error(0), final_path_query(), last_error(0)],
        1,
        0,
    )
    .expect("closed native-handle query chain is canonical")
}

fn canonical_attempts() -> Vec<FilesystemOperationAttempt> {
    let source = FilesystemSourceInputReplayRecord::new(vec![
        FilesystemSourceInputReplayEventRecord::NativeHandleQueryChain(chain()),
    ])
    .expect("native-handle query chain is one Source event");
    FilesystemReplay::from_source_input_record(source)
        .expect("typed query chain fits replay custody")
        .attempts()
        .to_vec()
}

fn assert_observation_rejected(attempts: Vec<FilesystemOperationAttempt>) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(attempts, Vec::new());
    assert!(FilesystemReplay::from_source_input_observations(&observations).is_err());
}

#[test]
fn typed_query_chain_reconstructs_exact_open_queries_and_close() {
    let attempts = canonical_attempts();
    assert_eq!(
        attempts
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![28, 35, 31, 35, 29]
    );
    assert!(source_native_handle_query_chain_is_exact(&attempts));

    let open = &attempts[0];
    assert_eq!(open.provider(), FilesystemObservationProvider::RealScoped);
    assert_eq!(
        open.result(),
        Some(FilesystemOperationResult::LogicalHandle(identity(7)))
    );
    assert_eq!(
        open.scalar_operands()
            .iter()
            .map(|operand| operand.value())
            .collect::<Vec<_>>(),
        vec![
            FilesystemScalarOperandValue::U32(0),
            FilesystemScalarOperandValue::U32(0x7),
            FilesystemScalarOperandValue::I64(0),
            FilesystemScalarOperandValue::U32(3),
            FilesystemScalarOperandValue::U32(0x0200_0000),
        ]
    );
    assert_eq!(open.logical_handle_inputs()[0].operand_ordinal(), 6);
    assert_eq!(
        open.logical_handle_inputs()[0].resolution(),
        FilesystemLogicalHandleInputResolution::Null
    );
    let output = open.logical_handle_output().expect("open mints one handle");
    assert_eq!(output.kind(), FilesystemLogicalHandleKind::Native);
    assert_eq!(
        output.source(),
        FilesystemLogicalHandleOutputSource::Created
    );

    let query = &attempts[2];
    assert_eq!(
        query.logical_handle_inputs()[0].resolution(),
        FilesystemLogicalHandleInputResolution::Resolved(identity(7))
    );
    assert_eq!(query.returned_paths()[0].bytes(), b"C:\\pkg\\main.omg");

    let close = &attempts[4];
    assert_eq!(close.result(), Some(FilesystemOperationResult::Scalar(1)));
    assert_eq!(close.retired_logical_handles(), &[identity(7)]);

    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(attempts, Vec::new());
    FilesystemReplay::from_source_input_observations(&observations)
        .expect("observed query chain passes the same exact grammar");
}

#[test]
fn minimal_query_chain_is_exact() {
    let source = FilesystemSourceInputReplayRecord::new(vec![
        FilesystemSourceInputReplayEventRecord::NativeHandleQueryChain(
            FilesystemSourceNativeHandleQueryChainReplayRecord::new(
                root(1),
                b"pkg/main.omg".to_vec(),
                7,
                0,
                vec![final_path_query()],
                1,
                0,
            )
            .unwrap(),
        ),
    ])
    .unwrap();
    let attempts = FilesystemReplay::from_source_input_record(source)
        .unwrap()
        .attempts()
        .to_vec();
    assert_eq!(
        attempts
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![28, 31, 29]
    );
    assert!(source_native_handle_query_chain_is_exact(&attempts));
}

#[test]
fn query_chain_requires_the_constrained_acquisition_contract() {
    for value in [
        FilesystemScalarOperandValue::U32(1),   // nonzero desired access
        FilesystemScalarOperandValue::U32(0x3), // sharing loses FILE_SHARE_DELETE
        FilesystemScalarOperandValue::I64(4),   // nonnull security attributes
        FilesystemScalarOperandValue::U32(4),   // not OPEN_EXISTING
        FilesystemScalarOperandValue::U32(0x0600_0000), // FILE_FLAG_DELETE_ON_CLOSE attached
    ] {
        let mut attempts = canonical_attempts();
        attempts[0].scalar_operands = vec![crate::FilesystemScalarOperand {
            operand_ordinal: 1,
            value,
        }];
        assert_observation_rejected(attempts);
    }
    for (ordinal, value) in [
        (1u8, FilesystemScalarOperandValue::U32(1)),
        (2, FilesystemScalarOperandValue::U32(0x3)),
        (3, FilesystemScalarOperandValue::I64(4)),
        (4, FilesystemScalarOperandValue::U32(4)),
        (5, FilesystemScalarOperandValue::U32(0x0600_0000)),
    ] {
        let mut attempts = canonical_attempts();
        let index = usize::from(ordinal - 1);
        attempts[0].scalar_operands[index].value = value;
        assert_observation_rejected(attempts);
    }
}

#[test]
fn query_chain_rejects_failed_acquisition() {
    let mut attempts = canonical_attempts();
    attempts[0].outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 2,
    });
    assert_observation_rejected(attempts);
}

#[test]
fn query_chain_rejects_acquisition_alias_and_escape() {
    let mut attempts = canonical_attempts();
    attempts[0].logical_handle_inputs[0].resolution =
        FilesystemLogicalHandleInputResolution::Resolved(identity(9));
    assert_observation_rejected(attempts);

    let mut attempts = canonical_attempts();
    attempts[0].logical_handle_output = Some(crate::FilesystemLogicalHandleOutput {
        kind: FilesystemLogicalHandleKind::Native,
        identity: identity(7),
        source: FilesystemLogicalHandleOutputSource::Borrowed(identity(9)),
    });
    assert_observation_rejected(attempts);

    // An intervening operation that duplicates the handle out of the bounded
    // sequence is an escape, not a preserving observation.
    let mut attempts = canonical_attempts();
    attempts[2].logical_handle_output = Some(crate::FilesystemLogicalHandleOutput {
        kind: FilesystemLogicalHandleKind::Native,
        identity: identity(9),
        source: FilesystemLogicalHandleOutputSource::Duplicated(identity(7)),
    });
    assert_observation_rejected(attempts);
}

#[test]
fn query_chain_rejects_substituted_and_cross_domain_inputs() {
    let mut attempts = canonical_attempts();
    attempts[2].logical_handle_inputs[0].resolution =
        FilesystemLogicalHandleInputResolution::Resolved(identity(9));
    assert_observation_rejected(attempts);

    let mut attempts = canonical_attempts();
    attempts[2].logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Descriptor;
    assert_observation_rejected(attempts);

    let mut attempts = canonical_attempts();
    attempts[4].logical_handle_inputs[0].resolution =
        FilesystemLogicalHandleInputResolution::Resolved(identity(9));
    assert_observation_rejected(attempts);
}

#[test]
fn query_chain_rejects_missing_ambiguous_and_late_release() {
    // No release at all.
    let mut attempts = canonical_attempts();
    attempts.pop();
    assert_observation_rejected(attempts);

    // A use after the release is a second event start the grammar cannot
    // attribute to the bounded occurrence.
    let mut attempts = canonical_attempts();
    let mut late = attempts[2].clone();
    late.operation_tag = 31;
    attempts.push(late);
    assert_observation_rejected(attempts);

    // A second release is likewise a distinct event start.
    let mut attempts = canonical_attempts();
    attempts.push(attempts[4].clone());
    assert_observation_rejected(attempts);

    // An early retirement invalidates the later close.
    let mut attempts = canonical_attempts();
    attempts[2] = attempts[4].clone();
    assert_observation_rejected(attempts);

    // A failed close is not the proven release.
    let mut attempts = canonical_attempts();
    attempts[4].outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: 6,
    });
    assert_observation_rejected(attempts);

    // A close that does not retire the identity is not the proven release.
    let mut attempts = canonical_attempts();
    attempts[4].retired_logical_handles.clear();
    assert_observation_rejected(attempts);
}

#[test]
fn query_chain_rejects_tampered_query_lanes() {
    let mut attempts = canonical_attempts();
    attempts[2].operation_tag = 32;
    assert_observation_rejected(attempts);

    let mut attempts = canonical_attempts();
    attempts[2].scalar_operands[0].operand_ordinal = 1;
    assert_observation_rejected(attempts);

    let mut attempts = canonical_attempts();
    attempts[2].mutable_byte_operands[0].operand_ordinal = 2;
    assert_observation_rejected(attempts);

    let mut attempts = canonical_attempts();
    attempts[2].mutable_byte_operands[0].post_bytes[14] = 1;
    assert_observation_rejected(attempts);

    let mut attempts = canonical_attempts();
    attempts[2].mutable_byte_operand_resolutions[0].bytes[1] ^= 1;
    assert_observation_rejected(attempts);

    let mut attempts = canonical_attempts();
    attempts[2].returned_paths[0].bytes[1] ^= 1;
    assert_observation_rejected(attempts);

    let mut attempts = canonical_attempts();
    attempts[2].returned_paths[0].operand_ordinal = 2;
    assert_observation_rejected(attempts);

    // The error-slot read must return the slot it observes.
    let mut attempts = canonical_attempts();
    attempts[1].outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(5),
        post_error: 0,
    });
    assert_observation_rejected(attempts);

    // A chain without a final-path query is not this family.
    let mut attempts = canonical_attempts();
    attempts.remove(2);
    assert_observation_rejected(attempts);
}

#[test]
fn query_chain_records_reject_inconsistent_construction() {
    assert!(
        FilesystemSourceNativeHandleQueryChainReplayRecord::new(
            root(1),
            b"pkg/main.omg".to_vec(),
            0,
            0,
            vec![final_path_query()],
            1,
            0,
        )
        .is_err()
    );
    assert!(
        FilesystemSourceNativeHandleQueryChainReplayRecord::new(
            root(1),
            b"../escape".to_vec(),
            7,
            0,
            vec![final_path_query()],
            1,
            0,
        )
        .is_err()
    );
    assert!(
        FilesystemSourceNativeHandleQueryChainReplayRecord::new(
            root(1),
            b"pkg/main.omg".to_vec(),
            7,
            0,
            Vec::new(),
            1,
            0,
        )
        .is_err()
    );
    assert!(
        FilesystemSourceNativeHandleQueryChainReplayRecord::new(
            root(1),
            b"pkg/main.omg".to_vec(),
            7,
            0,
            vec![last_error(0)],
            1,
            0,
        )
        .is_err()
    );
    assert!(
        FilesystemSourceNativeHandleQueryChainReplayRecord::new(
            root(1),
            b"pkg/main.omg".to_vec(),
            7,
            0,
            vec![final_path_query()],
            0,
            0,
        )
        .is_err()
    );

    let path = b"C:\\pkg\\main.omg".to_vec();
    let mut post_bytes = path.clone();
    post_bytes.push(0);
    post_bytes.resize(260, 0);
    // Resolution must equal pre-state.
    assert!(
        FilesystemNativeHandleFinalPathQueryReplayRecord::new(
            260,
            0,
            15,
            0,
            vec![1; 260],
            vec![0; 260],
            post_bytes.clone(),
            path.clone(),
        )
        .is_err()
    );
    // Post-state must NUL-terminate and leave the tail unchanged.
    let mut bad_post = post_bytes.clone();
    bad_post[14] = 1;
    assert!(
        FilesystemNativeHandleFinalPathQueryReplayRecord::new(
            260,
            0,
            15,
            0,
            vec![0; 260],
            vec![0; 260],
            bad_post,
            path.clone(),
        )
        .is_err()
    );
    // Result must equal the returned path length.
    assert!(
        FilesystemNativeHandleFinalPathQueryReplayRecord::new(
            260,
            0,
            16,
            0,
            vec![0; 260],
            vec![0; 260],
            post_bytes.clone(),
            path.clone(),
        )
        .is_err()
    );
    // Capacity must hold the path plus its terminator.
    assert!(
        FilesystemNativeHandleFinalPathQueryReplayRecord::new(
            15,
            0,
            15,
            0,
            vec![0; 15],
            vec![0; 15],
            vec![0; 15],
            path.clone(),
        )
        .is_err()
    );
}

#[test]
fn query_chain_identity_joins_source_event_distinctness() {
    let query_chain = FilesystemSourceInputReplayEventRecord::NativeHandleQueryChain(chain());
    let duplicate = FilesystemSourceInputReplayEventRecord::NativeHandleQueryChain(
        FilesystemSourceNativeHandleQueryChainReplayRecord::new(
            root(1),
            b"pkg/other.omg".to_vec(),
            7,
            0,
            vec![final_path_query()],
            1,
            0,
        )
        .unwrap(),
    );
    assert!(FilesystemSourceInputReplayRecord::new(vec![query_chain, duplicate]).is_err());
}
