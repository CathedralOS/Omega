use super::{
    WRITE_KINDS_AND_TAGS, assert_tampered_seek_rejected, assert_tampered_set_file_times_rejected,
    assert_tampered_write_operation_rejected, nonempty_side_lane_attempts, source_input,
};
use crate::filesystem_replay::FilesystemInputUnknownDescriptorSeekReplayRecord as SeekRecord;
use crate::filesystem_replay::FilesystemInputUnknownDescriptorSetFileTimesReplayRecord as SetFileTimesRecord;
use crate::filesystem_replay::FilesystemInputUnknownDescriptorWriteOperationReplayKind as WriteKind;
use crate::filesystem_replay::FilesystemInputUnknownDescriptorWriteOperationReplayRecord as WriteRecord;
use crate::filesystem_replay::{
    unknown_descriptor_seek_attempt, unknown_descriptor_seek_from_exact_attempt,
    unknown_descriptor_set_file_times_attempt,
    unknown_descriptor_set_file_times_from_exact_attempt,
    unknown_descriptor_write_operation_attempt,
    unknown_descriptor_write_operation_from_exact_attempt,
};
use crate::{
    BuildIncludedSource, EvaluationObservations, FilesystemEvaluationHaltKind,
    FilesystemGrantRootIdentity, FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemReplay, FilesystemScalarOperand,
    FilesystemScalarOperandValue, MAX_FILESYSTEM_REPLAY_RETAINED_BYTES,
};

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

#[test]
fn unknown_descriptor_set_file_times_record_round_trips_with_optional_source_prefix() {
    let times = (0_u8..40).collect::<Vec<_>>();
    let record = SetFileTimesRecord::new(None, times.clone()).unwrap();
    assert!(record.source_input().is_none());
    assert_eq!(record.times(), times);

    let without_source =
        FilesystemReplay::from_input_unknown_descriptor_set_file_times_record(record).unwrap();
    assert_eq!(without_source.attempts().len(), 1);
    assert_eq!(without_source.attempts()[0].operation_tag(), 42);
    assert_eq!(
        unknown_descriptor_set_file_times_from_exact_attempt(&without_source.attempts()[0]),
        Some(times.as_slice())
    );
    assert!(without_source.executes_replay_attempt(0));
    assert!(!without_source.has_output_attempts());

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        without_source.attempts().to_vec(),
        Vec::new(),
    );
    let observed =
        FilesystemReplay::from_input_unknown_descriptor_set_file_times_observations(&observations)
            .unwrap();
    assert_eq!(observed.attempts(), without_source.attempts());

    let with_source = FilesystemReplay::from_input_unknown_descriptor_set_file_times_record(
        SetFileTimesRecord::new(Some(source_input()), times.clone()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        with_source
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 42]
    );
    assert!((0..3).all(|index| !with_source.executes_replay_attempt(index)));
    assert!(with_source.executes_replay_attempt(3));
    assert!(!with_source.has_output_attempts());

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        with_source.attempts().to_vec(),
        Vec::new(),
    );
    let observed =
        FilesystemReplay::from_input_unknown_descriptor_set_file_times_observations(&observations)
            .unwrap();
    assert_eq!(observed.attempts(), with_source.attempts());
}

#[test]
fn unknown_descriptor_set_file_times_record_rejects_short_and_oversized_carriers() {
    assert!(SetFileTimesRecord::new(None, vec![0; 31]).is_err());

    let oversized = vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES / 3 + 1];
    let record = SetFileTimesRecord::new(None, oversized.clone()).unwrap();
    assert!(FilesystemReplay::from_input_unknown_descriptor_set_file_times_record(record).is_err());

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_set_file_times_attempt(oversized)],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_set_file_times_observations(&observations)
            .is_err()
    );
}

#[test]
fn unknown_descriptor_set_file_times_observations_reject_failure_and_carrier_drift() {
    let exact = unknown_descriptor_set_file_times_attempt(vec![7; 40]);

    let mut changed = exact.clone();
    changed.operation_tag = 41;
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: 9,
    });
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 13,
    });
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands.push(FilesystemScalarOperand {
        operand_ordinal: 2,
        value: FilesystemScalarOperandValue::U64(40),
    });
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions[0].operand_ordinal = 2;
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands[0].operand_ordinal = 2;
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions.clear();
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands.clear();
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions[0].bytes[0] ^= 1;
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands[0].pre_bytes[0] ^= 1;
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands[0].post_bytes[0] ^= 1;
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions[0]
        .bytes
        .truncate(31);
    changed.mutable_byte_operands[0].pre_bytes.truncate(31);
    changed.mutable_byte_operands[0].post_bytes.truncate(31);
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].operand_ordinal = 1;
    assert_tampered_set_file_times_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
    assert_tampered_set_file_times_rejected(changed);

    for changed in nonempty_side_lane_attempts(exact) {
        assert_tampered_set_file_times_rejected(changed);
    }
}

#[test]
fn unknown_descriptor_set_file_times_observations_reject_handoff_and_non_source_prefix() {
    let exact = unknown_descriptor_set_file_times_attempt(vec![3; 32]);
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![exact.clone()],
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
        FilesystemReplay::from_input_unknown_descriptor_set_file_times_observations(&observations)
            .is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_seek_attempt(0, 0), exact],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_set_file_times_observations(&observations)
            .is_err()
    );
}
