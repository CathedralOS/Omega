use super::{
    FilesystemInputUnknownDescriptorOperationReplayKind as Kind,
    FilesystemInputUnknownDescriptorOperationReplayRecord as Record,
    FilesystemInputUnknownDescriptorSeekReplayRecord as SeekRecord,
    FilesystemInputUnknownDescriptorWriteOperationReplayKind as WriteKind,
    FilesystemInputUnknownDescriptorWriteOperationReplayRecord as WriteRecord,
    unknown_descriptor_operation_attempt, unknown_descriptor_operation_from_exact_attempt,
    unknown_descriptor_seek_attempt, unknown_descriptor_seek_from_exact_attempt,
    unknown_descriptor_write_operation_attempt,
    unknown_descriptor_write_operation_from_exact_attempt,
};
use crate::{
    BuildIncludedSource, EvaluationObservations, FilesystemAuthorizedPath, FilesystemByteOperand,
    FilesystemEvaluationHaltKind, FilesystemGrantAccess, FilesystemGrantRefusal,
    FilesystemGrantRefusalReason, FilesystemGrantRootIdentity, FilesystemLogicalHandleIdentity,
    FilesystemLogicalHandleInput, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemLogicalHandleOutput,
    FilesystemLogicalHandleOutputSource, FilesystemMetadataObservation,
    FilesystemMetadataObservationKind, FilesystemMutableByteOperand,
    FilesystemMutableByteOperandResolution, FilesystemMutableI64Operand,
    FilesystemMutableI64OperandResolution, FilesystemObservationProvider,
    FilesystemObservedByteRegion, FilesystemObservedByteRegionKind, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemPathLikeOperand,
    FilesystemReplay, FilesystemReplayReadKind, FilesystemReplayReadRecord, FilesystemReturnedPath,
    FilesystemReturnedPathCompleteness, FilesystemReturnedPathKind,
    FilesystemRootedPathOperandResolution, FilesystemScalarOperand, FilesystemScalarOperandValue,
    FilesystemSourceInputReplayEventRecord, FilesystemSourceInputReplayRecord,
    FilesystemSourceReadChainReplayRecord,
};

const KINDS_AND_TAGS: [(Kind, u16); 4] = [
    (Kind::Close, 8),
    (Kind::Sync, 43),
    (Kind::SyncData, 44),
    (Kind::Duplicate, 45),
];

const WRITE_KINDS_AND_TAGS: [(WriteKind, u16); 4] = [
    (WriteKind::SetFilePermissions { mode: 0o640 }, 17),
    (WriteKind::SetLength { length: -47 }, 41),
    (WriteKind::LockFile { operation: 3 }, 46),
    (WriteKind::ChangeFileOwner { uid: -1, gid: 501 }, 49),
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

#[test]
fn unknown_descriptor_seek_record_without_source_reconstructs_exact_attempt() {
    let record = SeekRecord::new(None, i64::MIN, i32::MAX);
    assert!(record.source_input().is_none());
    assert_eq!(record.offset(), i64::MIN);
    assert_eq!(record.whence(), i32::MAX);

    let replay = FilesystemReplay::from_input_unknown_descriptor_seek_record(record).unwrap();
    assert_eq!(replay.attempts().len(), 1);
    assert_eq!(
        unknown_descriptor_seek_from_exact_attempt(&replay.attempts()[0]),
        Some((i64::MIN, i32::MAX))
    );
    assert!(replay.executes_replay_attempt(0));
    assert!(!replay.has_output_attempts());
}

#[test]
fn unknown_descriptor_seek_observations_without_source_preserve_authored_values() {
    for (offset, whence) in [(0, 0), (-47, 1), (i64::MAX, i32::MIN)] {
        let exact = unknown_descriptor_seek_attempt(offset, whence);
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            vec![exact.clone()],
            Vec::new(),
        );
        let replay =
            FilesystemReplay::from_input_unknown_descriptor_seek_observations(&observations)
                .unwrap();
        assert_eq!(replay.attempts(), &[exact]);
        assert_eq!(
            unknown_descriptor_seek_from_exact_attempt(&replay.attempts()[0]),
            Some((offset, whence))
        );
    }
}

#[test]
fn unknown_descriptor_seek_record_and_observations_accept_exact_source_prefix() {
    let record = SeekRecord::new(Some(source_input()), 91, 2);
    assert!(record.source_input().is_some());
    let replay = FilesystemReplay::from_input_unknown_descriptor_seek_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 10]
    );
    assert!((0..3).all(|index| !replay.executes_replay_attempt(index)));
    assert!(replay.executes_replay_attempt(3));

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    let observed =
        FilesystemReplay::from_input_unknown_descriptor_seek_observations(&observations).unwrap();
    assert_eq!(observed.attempts(), replay.attempts());
}

#[test]
fn unknown_descriptor_seek_observations_reject_operation_shape_drift() {
    let exact = unknown_descriptor_seek_attempt(-47, 2);
    let identity = FilesystemLogicalHandleIdentity::new(9).unwrap();

    let mut changed = exact.clone();
    changed.operation_tag = 11;
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = None;
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::EvaluationHalted(
        FilesystemEvaluationHaltKind::Trap,
    ));
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: 9,
    });
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::LogicalHandle(identity),
        post_error: 9,
    });
    assert_tampered_seek_rejected(changed);

    let mut changed = exact;
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 13,
    });
    assert_tampered_seek_rejected(changed);
}

#[test]
fn unknown_descriptor_seek_observations_reject_scalar_shape_drift() {
    let exact = unknown_descriptor_seek_attempt(-47, 2);

    let mut changed = exact.clone();
    changed.scalar_operands.clear();
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands.remove(0);
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands.pop();
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands.push(FilesystemScalarOperand {
        operand_ordinal: 3,
        value: FilesystemScalarOperandValue::I32(0),
    });
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[0].operand_ordinal = 0;
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[1].operand_ordinal = 1;
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[0].value = FilesystemScalarOperandValue::U64((-47_i64) as u64);
    assert_tampered_seek_rejected(changed);

    let mut changed = exact;
    changed.scalar_operands[1].value = FilesystemScalarOperandValue::I64(2);
    assert_tampered_seek_rejected(changed);
}

#[test]
fn unknown_descriptor_seek_observations_reject_logical_handle_drift() {
    let exact = unknown_descriptor_seek_attempt(-47, 2);
    let identity = FilesystemLogicalHandleIdentity::new(9).unwrap();

    let mut changed = exact.clone();
    changed.logical_handle_inputs.clear();
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed
        .logical_handle_inputs
        .push(FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            resolution: FilesystemLogicalHandleInputResolution::Unknown,
        });
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].operand_ordinal = 1;
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Native;
    assert_tampered_seek_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
    assert_tampered_seek_rejected(changed);

    let mut changed = exact;
    changed.logical_handle_inputs[0].resolution =
        FilesystemLogicalHandleInputResolution::Resolved(identity);
    assert_tampered_seek_rejected(changed);
}

#[test]
fn unknown_descriptor_seek_observations_reject_every_nonempty_side_lane() {
    for changed in nonempty_side_lane_attempts(unknown_descriptor_seek_attempt(-47, 2)) {
        assert_tampered_seek_rejected(changed);
    }
}

fn nonempty_side_lane_attempts(
    exact: FilesystemOperationAttempt,
) -> Vec<FilesystemOperationAttempt> {
    let root = FilesystemGrantRootIdentity::new(1).unwrap();
    let identity = FilesystemLogicalHandleIdentity::new(9).unwrap();
    let mut changed_attempts = Vec::new();

    let mut changed = exact.clone();
    changed.byte_operands.push(FilesystemByteOperand {
        operand_ordinal: 3,
        bytes: vec![1],
    });
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed.path_like_operands.push(FilesystemPathLikeOperand {
        operand_ordinal: 3,
        bytes: b"name".to_vec(),
    });
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed
        .rooted_path_operand_resolutions
        .push(FilesystemRootedPathOperandResolution {
            operand_ordinal: 3,
            root,
            relative_path: b"name".to_vec(),
        });
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed.returned_paths.push(FilesystemReturnedPath {
        operand_ordinal: 3,
        kind: FilesystemReturnedPathKind::FinalPath,
        completeness: FilesystemReturnedPathCompleteness::Complete,
        bytes: b"name".to_vec(),
    });
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed
        .observed_byte_regions
        .push(FilesystemObservedByteRegion {
            output_operand_ordinal: 3,
            kind: FilesystemObservedByteRegionKind::SequentialFileRead,
            offset: 0,
            length: 1,
        });
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed
        .metadata_observations
        .push(FilesystemMetadataObservation::new(
            3,
            FilesystemMetadataObservationKind::OpenDescriptor,
            0,
            0,
            0,
        ));
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed
        .mutable_byte_operand_resolutions
        .push(FilesystemMutableByteOperandResolution {
            operand_ordinal: 3,
            bytes: vec![1],
        });
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed
        .mutable_i64_operand_resolutions
        .push(FilesystemMutableI64OperandResolution {
            operand_ordinal: 3,
            value: 1,
        });
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed
        .mutable_byte_operands
        .push(FilesystemMutableByteOperand {
            operand_ordinal: 3,
            pre_bytes: vec![1],
            post_bytes: vec![1],
        });
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed
        .mutable_i64_operands
        .push(FilesystemMutableI64Operand {
            operand_ordinal: 3,
            pre_value: 1,
            post_value: 1,
        });
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed.authorized_paths.push(FilesystemAuthorizedPath {
        operand_ordinal: 3,
        access: FilesystemGrantAccess::Read,
        root,
        relative_path: b"name".to_vec(),
    });
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed.logical_handle_output = Some(FilesystemLogicalHandleOutput {
        kind: FilesystemLogicalHandleKind::Descriptor,
        identity,
        source: FilesystemLogicalHandleOutputSource::Created,
    });
    changed_attempts.push(changed);

    let mut changed = exact.clone();
    changed.retired_logical_handles.push(identity);
    changed_attempts.push(changed);

    let mut changed = exact;
    changed.grant_refusals.push(FilesystemGrantRefusal {
        operand_ordinal: 3,
        access: FilesystemGrantAccess::Read,
        reason: FilesystemGrantRefusalReason::OutsideGrantedRoots,
    });
    changed_attempts.push(changed);

    changed_attempts
}

#[test]
fn unknown_descriptor_seek_observations_reject_generated_source_handoff() {
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_seek_attempt(-47, 2)],
        vec![
            BuildIncludedSource::from_coordinate(
                FilesystemGrantRootIdentity::new(2).unwrap(),
                b"generated.omg".to_vec(),
                1,
            )
            .unwrap(),
        ],
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_seek_observations(&observations).is_err()
    );
}

fn assert_tampered_seek_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_seek_observations(&observations).is_err()
    );
}

#[test]
fn unknown_descriptor_write_operations_round_trip_with_optional_source_prefix() {
    for (kind, tag) in WRITE_KINDS_AND_TAGS {
        let record = WriteRecord::new(None, kind);
        assert!(record.source_input().is_none());
        assert_eq!(record.kind(), kind);
        let without_source =
            FilesystemReplay::from_input_unknown_descriptor_write_operation_record(record).unwrap();
        assert_eq!(without_source.attempts().len(), 1);
        assert_eq!(without_source.attempts()[0].operation_tag(), tag);
        assert_eq!(
            unknown_descriptor_write_operation_from_exact_attempt(&without_source.attempts()[0]),
            Some(kind)
        );
        assert!(without_source.executes_replay_attempt(0));
        assert!(!without_source.has_output_attempts());

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            without_source.attempts().to_vec(),
            Vec::new(),
        );
        assert_eq!(
            FilesystemReplay::from_input_unknown_descriptor_write_operation_observations(
                &observations
            )
            .unwrap()
            .attempts(),
            without_source.attempts()
        );

        let with_source = FilesystemReplay::from_input_unknown_descriptor_write_operation_record(
            WriteRecord::new(Some(source_input()), kind),
        )
        .unwrap();
        assert_eq!(
            with_source
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![2, 4, 8, tag]
        );
        assert!((0..3).all(|index| !with_source.executes_replay_attempt(index)));
        assert!(with_source.executes_replay_attempt(3));
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            with_source.attempts().to_vec(),
            Vec::new(),
        );
        assert!(
            FilesystemReplay::from_input_unknown_descriptor_write_operation_observations(
                &observations
            )
            .is_ok()
        );
    }
}

#[test]
fn unknown_descriptor_write_operations_reject_kind_and_scalar_drift() {
    for (kind, tag) in WRITE_KINDS_AND_TAGS {
        let exact = unknown_descriptor_write_operation_attempt(kind);

        let mut changed = exact.clone();
        changed.operation_tag = tag + 1;
        assert_tampered_write_operation_rejected(changed);

        let mut changed = exact.clone();
        changed.scalar_operands.clear();
        assert_tampered_write_operation_rejected(changed);

        let mut changed = exact.clone();
        changed.scalar_operands.push(FilesystemScalarOperand {
            operand_ordinal: 3,
            value: FilesystemScalarOperandValue::I32(0),
        });
        assert_tampered_write_operation_rejected(changed);

        let mut changed = exact.clone();
        changed.scalar_operands[0].operand_ordinal = 0;
        assert_tampered_write_operation_rejected(changed);

        let mut changed = exact.clone();
        changed.scalar_operands[0].value = FilesystemScalarOperandValue::U64(0);
        assert_tampered_write_operation_rejected(changed);

        if exact.scalar_operands.len() == 2 {
            let mut changed = exact.clone();
            changed.scalar_operands[1].operand_ordinal = 1;
            assert_tampered_write_operation_rejected(changed);

            let mut changed = exact.clone();
            changed.scalar_operands[1].value = FilesystemScalarOperandValue::U32(501);
            assert_tampered_write_operation_rejected(changed);
        }

        let mut changed = exact.clone();
        changed.provider = FilesystemObservationProvider::Virtual;
        assert_tampered_write_operation_rejected(changed);

        let mut changed = exact.clone();
        changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: 9,
        });
        assert_tampered_write_operation_rejected(changed);

        let mut changed = exact;
        changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(-1),
            post_error: 13,
        });
        assert_tampered_write_operation_rejected(changed);
    }
}

#[test]
fn unknown_descriptor_write_operations_reject_side_lanes_and_handoffs() {
    let exact = unknown_descriptor_write_operation_attempt(WriteKind::SetLength { length: 47 });
    for changed in nonempty_side_lane_attempts(exact.clone()) {
        assert_tampered_write_operation_rejected(changed);
    }

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![exact],
        vec![
            BuildIncludedSource::from_coordinate(
                FilesystemGrantRootIdentity::new(2).unwrap(),
                b"generated.omg".to_vec(),
                1,
            )
            .unwrap(),
        ],
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_write_operation_observations(&observations)
            .is_err()
    );
}

fn assert_tampered_write_operation_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_write_operation_observations(&observations)
            .is_err()
    );
}
