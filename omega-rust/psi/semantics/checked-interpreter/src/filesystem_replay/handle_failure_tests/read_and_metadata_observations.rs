use super::{
    assert_tampered_payload_write_rejected, assert_tampered_read_file_metadata_rejected,
    assert_tampered_read_rejected, checked_unknown_descriptor_read_file_metadata,
    nonempty_side_lane_attempts, source_input,
};
use crate::filesystem_replay::FilesystemInputUnknownDescriptorReadFileMetadataReplayRecord as ReadFileMetadataRecord;
use crate::filesystem_replay::FilesystemInputUnknownDescriptorReadReplayKind as ReadKind;
use crate::filesystem_replay::FilesystemInputUnknownDescriptorReadReplayRecord as ReadRecord;
use crate::filesystem_replay::FilesystemInputUnknownDescriptorWriteReplayKind as PayloadWriteKind;
use crate::filesystem_replay::FilesystemInputUnknownDescriptorWriteReplayRecord as PayloadWriteRecord;
use crate::filesystem_replay::{
    unknown_descriptor_read_attempt, unknown_descriptor_read_file_metadata_attempt,
    unknown_descriptor_read_file_metadata_from_exact_attempt,
    unknown_descriptor_read_from_exact_attempt, unknown_descriptor_seek_attempt,
    unknown_descriptor_write_attempt, unknown_descriptor_write_from_exact_attempt,
};
use crate::{
    BuildIncludedSource, EvaluationObservations, FILESYSTEM_METADATA_API_CARRIER_BYTES,
    FilesystemAccess, FilesystemGrantRootIdentity, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemObservationProvider, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemReplay,
    FilesystemScalarOperand, FilesystemScalarOperandValue, InterpretOptions,
    MAX_FILESYSTEM_REPLAY_RETAINED_BYTES, interpret_entry,
};

#[test]
fn unknown_descriptor_read_records_round_trip_exact_carriers_with_optional_source_prefix() {
    let cases = [
        (ReadKind::Sequential { count: 3 }, 4),
        (
            ReadKind::Positioned {
                count: 4,
                offset: -47,
            },
            6,
        ),
    ];
    for (kind, tag) in cases {
        let buffer = vec![11, 29, 47, 83, 101];
        let record = ReadRecord::new(None, kind, buffer.clone()).unwrap();
        assert!(record.source_input().is_none());
        assert_eq!(record.kind(), kind);
        assert_eq!(record.buffer(), buffer);

        let replay = FilesystemReplay::from_input_unknown_descriptor_read_record(record).unwrap();
        assert_eq!(replay.attempts().len(), 1);
        assert_eq!(replay.attempts()[0].operation_tag(), tag);
        assert_eq!(
            unknown_descriptor_read_from_exact_attempt(&replay.attempts()[0]),
            Some((kind, buffer.as_slice()))
        );
        assert!(replay.executes_replay_attempt(0));
        assert!(!replay.has_output_attempts());

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let observed =
            FilesystemReplay::from_input_unknown_descriptor_read_observations(&observations)
                .unwrap();
        assert_eq!(observed.attempts(), replay.attempts());

        let with_source = FilesystemReplay::from_input_unknown_descriptor_read_record(
            ReadRecord::new(Some(source_input()), kind, buffer.clone()).unwrap(),
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
        assert!(!with_source.has_output_attempts());

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            with_source.attempts().to_vec(),
            Vec::new(),
        );
        let observed =
            FilesystemReplay::from_input_unknown_descriptor_read_observations(&observations)
                .unwrap();
        assert_eq!(observed.attempts(), with_source.attempts());
    }
}

#[test]
fn unknown_descriptor_read_records_reject_count_capacity_and_aggregate_size_drift() {
    let empty = FilesystemReplay::from_input_unknown_descriptor_read_record(
        ReadRecord::new(None, ReadKind::Sequential { count: 0 }, Vec::new()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        unknown_descriptor_read_from_exact_attempt(&empty.attempts()[0]),
        Some((ReadKind::Sequential { count: 0 }, &[][..]))
    );

    assert!(ReadRecord::new(None, ReadKind::Sequential { count: 4 }, vec![0; 3]).is_err());
    assert!(
        ReadRecord::new(
            None,
            ReadKind::Positioned {
                count: u64::MAX,
                offset: 0,
            },
            Vec::new(),
        )
        .is_err()
    );

    let oversized = vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES / 3 + 1];
    let record = ReadRecord::new(None, ReadKind::Sequential { count: 0 }, oversized).unwrap();
    assert!(FilesystemReplay::from_input_unknown_descriptor_read_record(record).is_err());

    let oversized = vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES / 3 + 1];
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_read_attempt(
            ReadKind::Sequential { count: 0 },
            oversized,
        )],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_read_observations(&observations).is_err()
    );
}

#[test]
fn unknown_descriptor_read_observations_reject_operation_scalar_and_failure_drift() {
    let exact = unknown_descriptor_read_attempt(
        ReadKind::Positioned {
            count: 3,
            offset: -47,
        },
        vec![1, 2, 3, 4],
    );

    let mut changed = exact.clone();
    changed.operation_tag = 4;
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: 9,
    });
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 13,
    });
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[0].operand_ordinal = 1;
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[0].value = FilesystemScalarOperandValue::I64(3);
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[1].operand_ordinal = 2;
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].operand_ordinal = 1;
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Find;
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
    assert_tampered_read_rejected(changed);

    let mut changed = exact;
    changed.scalar_operands.pop();
    assert_tampered_read_rejected(changed);
}

#[test]
fn unknown_descriptor_read_observations_reject_carrier_and_count_drift() {
    let exact =
        unknown_descriptor_read_attempt(ReadKind::Sequential { count: 3 }, vec![7, 11, 13, 17]);

    let mut changed = exact.clone();
    changed.scalar_operands[0].value = FilesystemScalarOperandValue::U64(5);
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions[0].operand_ordinal = 2;
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands[0].operand_ordinal = 2;
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions.clear();
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands.clear();
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions[0].bytes[0] ^= 1;
    assert_tampered_read_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands[0].pre_bytes[0] ^= 1;
    assert_tampered_read_rejected(changed);

    let mut changed = exact;
    changed.mutable_byte_operands[0].post_bytes[0] ^= 1;
    assert_tampered_read_rejected(changed);
}

#[test]
fn unknown_descriptor_read_observations_reject_side_lanes_handoff_and_non_source_prefix() {
    let exact = unknown_descriptor_read_attempt(
        ReadKind::Positioned {
            count: 2,
            offset: i64::MIN,
        },
        vec![3, 5, 7],
    );
    for changed in nonempty_side_lane_attempts(exact.clone()) {
        assert_tampered_read_rejected(changed);
    }

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
        FilesystemReplay::from_input_unknown_descriptor_read_observations(&observations).is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_seek_attempt(0, 0), exact],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_read_observations(&observations).is_err()
    );
}

#[test]
fn unknown_descriptor_read_file_metadata_records_preserve_boundary_carriers_and_source_prefixes() {
    let carrier = (0..FILESYSTEM_METADATA_API_CARRIER_BYTES)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let record = ReadFileMetadataRecord::new(None, carrier.clone());
    assert!(record.source_input().is_none());
    assert_eq!(record.carrier(), carrier);

    let without_source =
        FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_record(record).unwrap();
    assert_eq!(without_source.attempts().len(), 1);
    assert_eq!(without_source.attempts()[0].operation_tag(), 39);
    assert_eq!(
        unknown_descriptor_read_file_metadata_from_exact_attempt(&without_source.attempts()[0]),
        Some(carrier.as_slice())
    );
    assert!(without_source.executes_replay_attempt(0));
    assert!(!without_source.has_output_attempts());
    assert!(without_source.expected_included_sources().is_empty());

    let operation = &without_source.attempts()[0];
    assert_eq!(
        operation.provider,
        FilesystemObservationProvider::RealScoped
    );
    assert_eq!(
        operation.outcome,
        Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(-1),
            post_error: 9,
        })
    );
    assert!(operation.logical_handle_output.is_none());
    assert!(operation.retired_logical_handles.is_empty());

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        without_source.attempts().to_vec(),
        Vec::new(),
    );
    let observed = FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_observations(
        &observations,
    )
    .unwrap();
    assert_eq!(observed.attempts(), without_source.attempts());

    let with_source = FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_record(
        ReadFileMetadataRecord::new(Some(source_input()), carrier.clone()),
    )
    .unwrap();
    assert_eq!(
        with_source
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 39]
    );
    assert!((0..3).all(|index| !with_source.executes_replay_attempt(index)));
    assert!(with_source.executes_replay_attempt(3));
    assert!(!with_source.has_output_attempts());
    assert_eq!(
        unknown_descriptor_read_file_metadata_from_exact_attempt(&with_source.attempts()[3]),
        Some(carrier.as_slice())
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        with_source.attempts().to_vec(),
        Vec::new(),
    );
    let observed = FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_observations(
        &observations,
    )
    .unwrap();
    assert_eq!(observed.attempts(), with_source.attempts());
}

#[test]
fn unknown_descriptor_read_file_metadata_executes_provider_free_and_tears_down_empty() {
    let checked = checked_unknown_descriptor_read_file_metadata();
    let replay = FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_record(
        ReadFileMetadataRecord::new(None, vec![0; FILESYSTEM_METADATA_API_CARRIER_BYTES]),
    )
    .unwrap();

    let outcome = interpret_entry(
        &checked,
        "Main::read_unknown",
        &[],
        InterpretOptions {
            filesystem: FilesystemAccess::ReplayFilesystem(replay),
            ..InterpretOptions::default()
        },
    );

    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 0);
    assert!(outcome.stdout.is_empty());
    assert!(outcome.stderr.is_empty());
}

#[test]
fn unknown_descriptor_read_file_metadata_rejects_below_preparation_capacity() {
    let short = vec![7; FILESYSTEM_METADATA_API_CARRIER_BYTES - 1];
    let record = ReadFileMetadataRecord::new(None, short.clone());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_record(record).is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_read_file_metadata_attempt(short)],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_observations(
            &observations
        )
        .is_err()
    );
}

#[test]
fn unknown_descriptor_read_file_metadata_observations_retain_complete_carrier() {
    let mut carrier = vec![0; FILESYSTEM_METADATA_API_CARRIER_BYTES + 19];
    carrier[0] = 11;
    carrier[73] = 29;
    *carrier.last_mut().unwrap() = 47;
    let exact = unknown_descriptor_read_file_metadata_attempt(carrier.clone());
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![exact.clone()], Vec::new());
    let replay = FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_observations(
        &observations,
    )
    .unwrap();
    assert_eq!(replay.attempts(), &[exact]);
    assert_eq!(
        unknown_descriptor_read_file_metadata_from_exact_attempt(&replay.attempts()[0]),
        Some(carrier.as_slice())
    );
}

#[test]
fn unknown_descriptor_read_file_metadata_rejects_failure_and_handle_drift() {
    let exact = unknown_descriptor_read_file_metadata_attempt(vec![
        3;
        FILESYSTEM_METADATA_API_CARRIER_BYTES
    ]);

    let mut changed = exact.clone();
    changed.operation_tag = 38;
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = None;
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: 9,
    });
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 13,
    });
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands.push(FilesystemScalarOperand {
        operand_ordinal: 2,
        value: FilesystemScalarOperandValue::U64(0),
    });
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs.clear();
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].operand_ordinal = 1;
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Find;
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact;
    changed.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
    assert_tampered_read_file_metadata_rejected(changed);
}

#[test]
fn unknown_descriptor_read_file_metadata_rejects_carrier_drift() {
    let exact = unknown_descriptor_read_file_metadata_attempt(vec![
        5;
        FILESYSTEM_METADATA_API_CARRIER_BYTES
    ]);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions.clear();
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands.clear();
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions[0].operand_ordinal = 2;
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands[0].operand_ordinal = 2;
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions[0].bytes[0] ^= 1;
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands[0].pre_bytes[0] ^= 1;
    assert_tampered_read_file_metadata_rejected(changed);

    let mut changed = exact;
    changed.mutable_byte_operands[0].post_bytes[0] ^= 1;
    assert_tampered_read_file_metadata_rejected(changed);
}

#[test]
fn unknown_descriptor_read_file_metadata_rejects_side_lanes_handoff_and_invalid_prefix() {
    let exact = unknown_descriptor_read_file_metadata_attempt(vec![
        7;
        FILESYSTEM_METADATA_API_CARRIER_BYTES
    ]);
    for changed in nonempty_side_lane_attempts(exact.clone()) {
        assert_tampered_read_file_metadata_rejected(changed);
    }

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
        FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_observations(
            &observations
        )
        .is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_seek_attempt(0, 0), exact],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_observations(
            &observations
        )
        .is_err()
    );
}

#[test]
fn unknown_descriptor_read_file_metadata_enforces_aggregate_replay_limit() {
    let oversized = vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES / 3 + 1];
    let record = ReadFileMetadataRecord::new(None, oversized.clone());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_record(record).is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_read_file_metadata_attempt(oversized)],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_observations(
            &observations
        )
        .is_err()
    );
}

#[test]
fn unknown_descriptor_writes_round_trip_exact_payloads_with_optional_source_prefix() {
    let cases = [
        (PayloadWriteKind::Sequential, 5),
        (PayloadWriteKind::Positioned { offset: -47 }, 7),
    ];
    for (kind, tag) in cases {
        let payload = vec![11, 29, 47, 83, 101];
        let record = PayloadWriteRecord::new(None, kind, payload.clone());
        assert!(record.source_input().is_none());
        assert_eq!(record.kind(), kind);
        assert_eq!(record.payload(), payload);

        let replay = FilesystemReplay::from_input_unknown_descriptor_write_record(record).unwrap();
        assert_eq!(replay.attempts().len(), 1);
        assert_eq!(replay.attempts()[0].operation_tag(), tag);
        assert_eq!(
            unknown_descriptor_write_from_exact_attempt(&replay.attempts()[0]),
            Some((kind, payload.as_slice()))
        );
        assert!(replay.executes_replay_attempt(0));
        assert!(!replay.has_output_attempts());

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let observed =
            FilesystemReplay::from_input_unknown_descriptor_write_observations(&observations)
                .unwrap();
        assert_eq!(observed.attempts(), replay.attempts());

        let with_source = FilesystemReplay::from_input_unknown_descriptor_write_record(
            PayloadWriteRecord::new(Some(source_input()), kind, payload.clone()),
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
        assert!(!with_source.has_output_attempts());

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            with_source.attempts().to_vec(),
            Vec::new(),
        );
        let observed =
            FilesystemReplay::from_input_unknown_descriptor_write_observations(&observations)
                .unwrap();
        assert_eq!(observed.attempts(), with_source.attempts());
    }

    let empty = FilesystemReplay::from_input_unknown_descriptor_write_record(
        PayloadWriteRecord::new(None, PayloadWriteKind::Sequential, Vec::new()),
    )
    .unwrap();
    assert_eq!(
        unknown_descriptor_write_from_exact_attempt(&empty.attempts()[0]),
        Some((PayloadWriteKind::Sequential, &[][..]))
    );
}

#[test]
fn unknown_descriptor_write_records_enforce_aggregate_replay_limit() {
    let oversized = vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES];
    let record = PayloadWriteRecord::new(None, PayloadWriteKind::Sequential, oversized);
    assert!(FilesystemReplay::from_input_unknown_descriptor_write_record(record).is_err());

    let oversized = vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES];
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_write_attempt(
            PayloadWriteKind::Sequential,
            oversized,
        )],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_write_observations(&observations).is_err()
    );
}

#[test]
fn unknown_descriptor_write_observations_retain_payload_and_reject_shape_drift() {
    let exact = unknown_descriptor_write_attempt(
        PayloadWriteKind::Positioned { offset: -47 },
        vec![1, 2, 3, 4],
    );

    let mut changed = exact.clone();
    changed.byte_operands[0].bytes[0] ^= 1;
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![changed.clone()],
        Vec::new(),
    );
    let changed_replay =
        FilesystemReplay::from_input_unknown_descriptor_write_observations(&observations).unwrap();
    assert_eq!(
        unknown_descriptor_write_from_exact_attempt(&changed_replay.attempts()[0]),
        Some((
            PayloadWriteKind::Positioned { offset: -47 },
            changed.byte_operands[0].bytes.as_slice(),
        ))
    );

    let mut changed = exact.clone();
    changed.byte_operands[0].operand_ordinal = 2;
    assert_tampered_payload_write_rejected(changed);

    let mut changed = exact.clone();
    changed.byte_operands.clear();
    assert_tampered_payload_write_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[0].operand_ordinal = 1;
    assert_tampered_payload_write_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[0].value = FilesystemScalarOperandValue::U64(47);
    assert_tampered_payload_write_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands.clear();
    assert_tampered_payload_write_rejected(changed);

    let mut changed = exact.clone();
    changed.operation_tag = 5;
    assert_tampered_payload_write_rejected(changed);

    let mut changed = exact.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_tampered_payload_write_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: 9,
    });
    assert_tampered_payload_write_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 13,
    });
    assert_tampered_payload_write_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].operand_ordinal = 1;
    assert_tampered_payload_write_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Find;
    assert_tampered_payload_write_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
    assert_tampered_payload_write_rejected(changed);

    let mut sequential =
        unknown_descriptor_write_attempt(PayloadWriteKind::Sequential, vec![1, 2, 3, 4]);
    sequential.scalar_operands.push(FilesystemScalarOperand {
        operand_ordinal: 2,
        value: FilesystemScalarOperandValue::I64(0),
    });
    assert_tampered_payload_write_rejected(sequential);
}

#[test]
fn unknown_descriptor_write_observations_reject_side_lanes_handoff_and_non_source_prefix() {
    let exact = unknown_descriptor_write_attempt(
        PayloadWriteKind::Positioned { offset: i64::MIN },
        vec![3, 5, 7],
    );
    for changed in nonempty_side_lane_attempts(exact.clone()) {
        assert_tampered_payload_write_rejected(changed);
    }

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
        FilesystemReplay::from_input_unknown_descriptor_write_observations(&observations).is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_seek_attempt(0, 0), exact],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_write_observations(&observations).is_err()
    );
}
