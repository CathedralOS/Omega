use super::{
    FilesystemInputUnknownDescriptorOperationReplayKind as Kind,
    FilesystemInputUnknownDescriptorOperationReplayRecord as Record,
    unknown_descriptor_operation_attempt, unknown_descriptor_operation_from_exact_attempt,
};
use crate::{
    EvaluationObservations, FilesystemLogicalHandleIdentity,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource,
    FilesystemObservationProvider, FilesystemOperationAttemptOutcome, FilesystemOperationResult,
    FilesystemReplay, FilesystemReplayReadKind, FilesystemReplayReadRecord,
    FilesystemScalarOperand, FilesystemScalarOperandValue, FilesystemSourceInputReplayEventRecord,
    FilesystemSourceInputReplayRecord, FilesystemSourceReadChainReplayRecord,
};

const KINDS_AND_TAGS: [(Kind, u16); 4] = [
    (Kind::Close, 8),
    (Kind::Sync, 43),
    (Kind::SyncData, 44),
    (Kind::Duplicate, 45),
];

fn source_input() -> FilesystemSourceInputReplayRecord {
    let read = FilesystemReplayReadRecord::new(
        FilesystemReplayReadKind::Sequential,
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let chain = FilesystemSourceReadChainReplayRecord::new(
        crate::FilesystemGrantRootIdentity::new(1).unwrap(),
        b"input.omg".to_vec(),
        7,
        0,
        vec![read],
        0,
    )
    .unwrap();
    FilesystemSourceInputReplayRecord::new(vec![FilesystemSourceInputReplayEventRecord::ReadChain(
        chain,
    )])
    .unwrap()
}

#[test]
fn unknown_descriptor_operation_records_compose_after_optional_source_input() {
    for (kind, tag) in KINDS_AND_TAGS {
        let without_source = FilesystemReplay::from_input_unknown_descriptor_operation_record(
            Record::new(None, kind),
        )
        .unwrap();
        assert_eq!(without_source.attempts().len(), 1);
        assert_eq!(without_source.attempts()[0].operation_tag(), tag);
        assert!(without_source.executes_replay_attempt(0));
        assert!(!without_source.has_output_attempts());
        assert_eq!(
            unknown_descriptor_operation_from_exact_attempt(&without_source.attempts()[0]),
            Some(kind)
        );

        let with_source = FilesystemReplay::from_input_unknown_descriptor_operation_record(
            Record::new(Some(source_input()), kind),
        )
        .unwrap();
        assert_eq!(
            with_source
                .attempts()
                .iter()
                .map(crate::FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, tag]
        );
        assert!((0..3).all(|index| !with_source.executes_replay_attempt(index)));
        assert!(with_source.executes_replay_attempt(3));
    }
}

#[test]
fn unknown_descriptor_operation_observations_accept_each_closed_shape() {
    for (kind, _) in KINDS_AND_TAGS {
        let exact = unknown_descriptor_operation_attempt(kind);
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            vec![exact.clone()],
            Vec::new(),
        );
        let replay =
            FilesystemReplay::from_input_unknown_descriptor_operation_observations(&observations)
                .unwrap();
        assert_eq!(
            unknown_descriptor_operation_from_exact_attempt(&replay.attempts()[0]),
            Some(kind)
        );
    }
}

#[test]
fn unknown_descriptor_operation_observations_reject_lane_drift() {
    let exact = unknown_descriptor_operation_attempt(Kind::Duplicate);

    let mut changed = exact.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_tampered_operation_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 13,
    });
    assert_tampered_operation_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Native;
    assert_tampered_operation_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
    assert_tampered_operation_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands.push(FilesystemScalarOperand {
        operand_ordinal: 0,
        value: FilesystemScalarOperandValue::I32(-1),
    });
    assert_tampered_operation_rejected(changed);

    let identity = FilesystemLogicalHandleIdentity::new(9).unwrap();
    let mut changed = exact.clone();
    changed.logical_handle_output = Some(FilesystemLogicalHandleOutput {
        kind: FilesystemLogicalHandleKind::Descriptor,
        identity,
        source: FilesystemLogicalHandleOutputSource::Created,
    });
    assert_tampered_operation_rejected(changed);

    let mut changed = exact.clone();
    changed.retired_logical_handles.push(identity);
    assert_tampered_operation_rejected(changed);

    let mut changed = exact;
    changed.operation_tag = 42;
    assert_tampered_operation_rejected(changed);
}

fn assert_tampered_operation_rejected(attempt: crate::FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_operation_observations(&observations)
            .is_err()
    );
}
