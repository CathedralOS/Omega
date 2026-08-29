use super::{FilesystemSourceReadLinkReplayRecord, source_read_link_attempt_is_exact};
use crate::{
    EvaluationObservations, FilesystemByteOperand, FilesystemGrantAccess,
    FilesystemGrantRootIdentity, FilesystemInputOutputReplayRecord, FilesystemObservationProvider,
    FilesystemOperationAttempt, FilesystemOperationAttemptOutcome, FilesystemOperationResult,
    FilesystemOutputFileReplayRecord, FilesystemReplay, FilesystemReturnedPathCompleteness,
    FilesystemReturnedPathKind, FilesystemScalarOperandValue,
    FilesystemSourceInputReplayEventRecord, FilesystemSourceInputReplayRecord,
};

fn root(value: u32) -> FilesystemGrantRootIdentity {
    FilesystemGrantRootIdentity::new(value).expect("test root is nonzero")
}

fn complete_record() -> FilesystemSourceReadLinkReplayRecord {
    FilesystemSourceReadLinkReplayRecord::new(
        root(1),
        b"links/tool".to_vec(),
        root(1),
        b"resolved-parent/tool".to_vec(),
        5,
        3,
        17,
        vec![1, 2, 3, 4, 5, 6],
        vec![10, 11, 12, 13, 14, 15],
        vec![b'l', b'i', b'b', 13, 14, 15],
        FilesystemReturnedPathCompleteness::Complete,
        b"lib".to_vec(),
    )
    .expect("complete read-link payload is internally exact")
}

fn limited_record() -> FilesystemSourceReadLinkReplayRecord {
    FilesystemSourceReadLinkReplayRecord::new(
        root(1),
        b"links/long".to_vec(),
        root(1),
        b"links/long".to_vec(),
        4,
        4,
        0,
        vec![0; 6],
        vec![20, 21, 22, 23, 24, 25],
        vec![b'v', b'e', b'r', b'y', 24, 25],
        FilesystemReturnedPathCompleteness::LimitReached,
        b"very".to_vec(),
    )
    .expect("limited read-link payload retains only its authoritative prefix")
}

fn zero_count_limited_record() -> FilesystemSourceReadLinkReplayRecord {
    FilesystemSourceReadLinkReplayRecord::new(
        root(1),
        b"links/zero".to_vec(),
        root(1),
        b"links/zero".to_vec(),
        0,
        0,
        0,
        vec![1, 2, 3],
        vec![4, 5, 6],
        vec![4, 5, 6],
        FilesystemReturnedPathCompleteness::LimitReached,
        Vec::new(),
    )
    .expect("zero-count truncation has an empty authoritative prefix")
}

fn source_input(
    records: Vec<FilesystemSourceReadLinkReplayRecord>,
) -> FilesystemSourceInputReplayRecord {
    FilesystemSourceInputReplayRecord::new(
        records
            .into_iter()
            .map(FilesystemSourceInputReplayEventRecord::ReadLink)
            .collect(),
    )
    .expect("read-link events are typed Source inputs")
}

fn observation_accepts(attempt: FilesystemOperationAttempt) -> bool {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    FilesystemReplay::from_source_input_observations(&observations).is_ok()
}

#[test]
fn complete_limited_and_zero_count_events_reconstruct_exact_attempts() {
    let complete = complete_record();
    assert_eq!(complete.source_root(), root(1));
    assert_eq!(complete.source_relative_path(), b"links/tool");
    assert_eq!(complete.authorized_root(), root(1));
    assert_eq!(complete.authorized_relative_path(), b"resolved-parent/tool");
    assert_eq!(complete.requested_count(), 5);
    assert_eq!(complete.result(), 3);
    assert_eq!(complete.post_error(), 17);
    assert_eq!(complete.mutable_resolution(), &[1, 2, 3, 4, 5, 6]);
    assert_eq!(complete.mutable_pre_state(), &[10, 11, 12, 13, 14, 15]);
    assert_eq!(
        complete.mutable_post_state(),
        &[b'l', b'i', b'b', 13, 14, 15]
    );
    assert_eq!(
        complete.completeness(),
        FilesystemReturnedPathCompleteness::Complete
    );
    assert_eq!(complete.returned_bytes(), b"lib");

    let replay = FilesystemReplay::from_source_input_record(source_input(vec![
        complete,
        limited_record(),
        zero_count_limited_record(),
    ]))
    .expect("typed read-link events fit replay custody");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![21, 21, 21]
    );
    assert!(
        replay
            .attempts()
            .iter()
            .all(source_read_link_attempt_is_exact)
    );

    let [complete_attempt, limited_attempt, zero_attempt] = replay.attempts() else {
        panic!("each read-link event contributes exactly one attempt")
    };
    assert_eq!(
        complete_attempt.provider(),
        FilesystemObservationProvider::RealScoped
    );
    assert_eq!(complete_attempt.scalar_operands()[0].operand_ordinal(), 2);
    assert_eq!(
        complete_attempt.scalar_operands()[0].value(),
        FilesystemScalarOperandValue::U64(5)
    );
    assert_eq!(
        complete_attempt.rooted_path_operand_resolutions()[0].operand_ordinal(),
        0
    );
    assert_eq!(complete_attempt.authorized_paths()[0].operand_ordinal(), 0);
    assert_eq!(
        complete_attempt.mutable_byte_operand_resolutions()[0].operand_ordinal(),
        1
    );
    assert_eq!(
        complete_attempt.mutable_byte_operands()[0].operand_ordinal(),
        1
    );
    assert_eq!(complete_attempt.returned_paths()[0].operand_ordinal(), 1);
    assert_eq!(
        complete_attempt.returned_paths()[0].kind(),
        FilesystemReturnedPathKind::ReadLinkPayload
    );
    assert_eq!(limited_attempt.returned_paths()[0].bytes(), b"very");
    assert_eq!(
        limited_attempt.returned_paths()[0].completeness(),
        FilesystemReturnedPathCompleteness::LimitReached
    );
    assert!(zero_attempt.returned_paths()[0].bytes().is_empty());
    assert_eq!(
        zero_attempt.result(),
        Some(FilesystemOperationResult::Scalar(0))
    );

    for attempt in replay.attempts() {
        assert!(observation_accepts(attempt.clone()));
    }
}

#[test]
fn typed_record_rejects_invented_or_inconsistent_payload_state() {
    let base = complete_record();
    assert!(
        FilesystemSourceReadLinkReplayRecord::new(
            root(1),
            b"../link".to_vec(),
            root(1),
            b"link".to_vec(),
            1,
            1,
            0,
            vec![0],
            vec![0],
            vec![b'x'],
            FilesystemReturnedPathCompleteness::Complete,
            vec![b'x'],
        )
        .is_err()
    );
    assert!(
        FilesystemSourceReadLinkReplayRecord::new(
            root(1),
            b"link".to_vec(),
            root(2),
            b"link".to_vec(),
            1,
            1,
            0,
            vec![0],
            vec![0],
            vec![b'x'],
            FilesystemReturnedPathCompleteness::Complete,
            vec![b'x'],
        )
        .is_err()
    );
    assert!(
        FilesystemSourceReadLinkReplayRecord::new(
            base.source_root(),
            base.source_relative_path().to_vec(),
            base.authorized_root(),
            base.authorized_relative_path().to_vec(),
            5,
            -1,
            0,
            base.mutable_resolution().to_vec(),
            base.mutable_pre_state().to_vec(),
            base.mutable_post_state().to_vec(),
            base.completeness(),
            base.returned_bytes().to_vec(),
        )
        .is_err()
    );
    assert!(
        FilesystemSourceReadLinkReplayRecord::new(
            base.source_root(),
            base.source_relative_path().to_vec(),
            base.authorized_root(),
            base.authorized_relative_path().to_vec(),
            2,
            3,
            0,
            base.mutable_resolution().to_vec(),
            base.mutable_pre_state().to_vec(),
            base.mutable_post_state().to_vec(),
            base.completeness(),
            base.returned_bytes().to_vec(),
        )
        .is_err()
    );
    assert!(
        FilesystemSourceReadLinkReplayRecord::new(
            base.source_root(),
            base.source_relative_path().to_vec(),
            base.authorized_root(),
            base.authorized_relative_path().to_vec(),
            5,
            3,
            0,
            base.mutable_resolution().to_vec(),
            base.mutable_pre_state().to_vec(),
            base.mutable_post_state().to_vec(),
            FilesystemReturnedPathCompleteness::LimitReached,
            base.returned_bytes().to_vec(),
        )
        .is_err()
    );
}

#[test]
fn observed_read_link_shape_rejects_tampered_evidence_lanes() {
    let canonical =
        FilesystemReplay::from_source_input_record(source_input(vec![complete_record()]))
            .unwrap()
            .attempts()[0]
            .clone();

    let mut tampered = Vec::new();
    let mut attempt = canonical.clone();
    attempt.operation_tag = 20;
    tampered.push(attempt);
    let mut attempt = canonical.clone();
    attempt.provider = FilesystemObservationProvider::Virtual;
    tampered.push(attempt);
    let mut attempt = canonical.clone();
    attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(2),
        post_error: 17,
    });
    tampered.push(attempt);
    let mut attempt = canonical.clone();
    attempt.scalar_operands[0].value = FilesystemScalarOperandValue::U64(2);
    tampered.push(attempt);
    let mut attempt = canonical.clone();
    attempt.rooted_path_operand_resolutions[0].relative_path = b"../escape".to_vec();
    tampered.push(attempt);
    let mut attempt = canonical.clone();
    attempt.authorized_paths[0].access = FilesystemGrantAccess::Write;
    tampered.push(attempt);
    let mut attempt = canonical.clone();
    attempt.authorized_paths[0].root = root(2);
    tampered.push(attempt);
    let mut attempt = canonical.clone();
    attempt.returned_paths[0].kind = FilesystemReturnedPathKind::CanonicalPath;
    tampered.push(attempt);
    let mut attempt = canonical.clone();
    attempt.returned_paths[0].completeness = FilesystemReturnedPathCompleteness::LimitReached;
    tampered.push(attempt);
    let mut attempt = canonical.clone();
    attempt.returned_paths[0].bytes = b"lie".to_vec();
    tampered.push(attempt);
    let mut attempt = canonical.clone();
    attempt.mutable_byte_operand_resolutions[0].bytes.pop();
    tampered.push(attempt);
    let mut attempt = canonical.clone();
    attempt.mutable_byte_operands[0].post_bytes[5] ^= 1;
    tampered.push(attempt);
    let mut attempt = canonical;
    attempt.byte_operands.push(FilesystemByteOperand {
        operand_ordinal: 9,
        bytes: vec![1],
    });
    tampered.push(attempt);

    assert!(
        tampered
            .into_iter()
            .all(|attempt| !observation_accepts(attempt))
    );
}

#[test]
fn read_link_source_root_cannot_be_reused_as_output_root() {
    let source = source_input(vec![complete_record()]);
    let output =
        FilesystemOutputFileReplayRecord::empty(root(1), b"generated.bin".to_vec(), 9, 0, 0)
            .expect("output fixture is otherwise canonical");
    assert!(FilesystemInputOutputReplayRecord::new(source, vec![output], Vec::new()).is_err());
}
