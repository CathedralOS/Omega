use super::{
    KINDS_AND_TAGS, assert_tampered_close_handle_rejected, assert_tampered_final_path_rejected,
    assert_tampered_get_osfhandle_rejected, assert_tampered_operation_rejected,
    checked_unknown_descriptor_get_osfhandle, checked_unknown_native_handle_close_handle,
    checked_unknown_native_handle_final_path_name_by_handle, nonempty_side_lane_attempts,
    source_input,
};
use crate::filesystem_replay::FilesystemInputUnknownDescriptorGetOsfHandleReplayRecord as GetOsfHandleRecord;
use crate::filesystem_replay::FilesystemInputUnknownDescriptorOperationReplayKind as Kind;
use crate::filesystem_replay::FilesystemInputUnknownDescriptorOperationReplayRecord as Record;
use crate::filesystem_replay::FilesystemInputUnknownNativeHandleCloseHandleReplayRecord as CloseHandleRecord;
use crate::filesystem_replay::FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord as FinalPathRecord;
use crate::filesystem_replay::{
    unknown_descriptor_get_osfhandle_attempt, unknown_descriptor_get_osfhandle_attempt_is_exact,
    unknown_descriptor_operation_attempt, unknown_descriptor_operation_from_exact_attempt,
    unknown_descriptor_seek_attempt, unknown_native_handle_close_handle_attempt,
    unknown_native_handle_close_handle_attempt_is_exact,
    unknown_native_handle_final_path_name_by_handle_attempt,
    unknown_native_handle_final_path_name_by_handle_attempt_is_exact,
    unknown_native_handle_final_path_name_by_handle_from_exact_attempt,
};
use crate::{
    BuildIncludedSource, EvaluationObservations, FilesystemAccess, FilesystemGrantRootIdentity,
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemLogicalHandleOutput,
    FilesystemLogicalHandleOutputSource, FilesystemObservationProvider, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemReplay,
    FilesystemScalarOperand, FilesystemScalarOperandValue, InterpretOptions, interpret_entry,
};

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

#[test]
fn unknown_descriptor_get_osfhandle_records_compose_after_optional_source_input() {
    let record = GetOsfHandleRecord::new(None);
    assert!(record.source_input().is_none());
    let without_source =
        FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_record(record).unwrap();
    assert_eq!(without_source.attempts().len(), 1);
    let attempt = &without_source.attempts()[0];
    assert!(unknown_descriptor_get_osfhandle_attempt_is_exact(attempt));
    assert_eq!(attempt.operation_tag(), 30);
    assert_eq!(
        attempt.result(),
        Some(FilesystemOperationResult::Scalar(-2))
    );
    assert_eq!(attempt.post_error(), Some(0));
    assert!(attempt.logical_handle_output().is_none());
    assert!(without_source.executes_replay_attempt(0));
    assert!(!without_source.has_output_attempts());
    assert!(without_source.expected_included_sources().is_empty());

    let source = source_input();
    let record = GetOsfHandleRecord::new(Some(source.clone()));
    assert_eq!(record.source_input(), Some(&source));
    let with_source =
        FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_record(record).unwrap();
    assert_eq!(
        with_source
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 30]
    );
    assert!((0..3).all(|index| !with_source.executes_replay_attempt(index)));
    assert!(with_source.executes_replay_attempt(3));
    assert!(!with_source.has_output_attempts());

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        with_source.attempts().to_vec(),
        Vec::new(),
    );
    let observed =
        FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_observations(&observations)
            .unwrap();
    assert_eq!(observed.attempts(), with_source.attempts());
}

#[test]
fn unknown_descriptor_get_osfhandle_rejects_model_and_lane_drift() {
    let exact = unknown_descriptor_get_osfhandle_attempt();

    let mut changed = exact.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_tampered_get_osfhandle_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 0,
    });
    assert_tampered_get_osfhandle_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-2),
        post_error: 9,
    });
    assert_tampered_get_osfhandle_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Native;
    assert_tampered_get_osfhandle_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands.push(FilesystemScalarOperand {
        operand_ordinal: 1,
        value: FilesystemScalarOperandValue::I32(0),
    });
    assert_tampered_get_osfhandle_rejected(changed);

    for changed in nonempty_side_lane_attempts(exact) {
        assert_tampered_get_osfhandle_rejected(changed);
    }
}

#[test]
fn unknown_descriptor_get_osfhandle_rejects_handoff_and_invalid_prefix() {
    let exact = unknown_descriptor_get_osfhandle_attempt();
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
        FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_observations(&observations)
            .is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_seek_attempt(0, 0), exact],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_observations(&observations)
            .is_err()
    );
}

#[test]
fn unknown_descriptor_get_osfhandle_executes_exact_replay_provider_free() {
    let replay = FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_record(
        GetOsfHandleRecord::new(None),
    )
    .unwrap();
    let outcome = interpret_entry(
        &checked_unknown_descriptor_get_osfhandle(),
        "Main::get_unknown",
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
fn unknown_native_handle_close_handle_records_compose_after_optional_source_input() {
    let record = CloseHandleRecord::new(None);
    assert!(record.source_input().is_none());
    let without_source =
        FilesystemReplay::from_input_unknown_native_handle_close_handle_record(record).unwrap();
    assert_eq!(without_source.attempts().len(), 1);
    let attempt = &without_source.attempts()[0];
    assert!(unknown_native_handle_close_handle_attempt_is_exact(attempt));
    assert_eq!(attempt.operation_tag(), 29);
    assert_eq!(attempt.result(), Some(FilesystemOperationResult::Scalar(0)));
    assert_eq!(attempt.post_error(), Some(6));
    assert!(attempt.logical_handle_output().is_none());
    assert!(attempt.retired_logical_handles().is_empty());
    assert!(without_source.executes_replay_attempt(0));
    assert!(!without_source.has_output_attempts());
    assert!(without_source.expected_included_sources().is_empty());

    let source = source_input();
    let record = CloseHandleRecord::new(Some(source.clone()));
    assert_eq!(record.source_input(), Some(&source));
    let with_source =
        FilesystemReplay::from_input_unknown_native_handle_close_handle_record(record).unwrap();
    assert_eq!(
        with_source
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 29]
    );
    assert!((0..3).all(|index| !with_source.executes_replay_attempt(index)));
    assert!(with_source.executes_replay_attempt(3));
    assert!(!with_source.has_output_attempts());

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        with_source.attempts().to_vec(),
        Vec::new(),
    );
    let observed =
        FilesystemReplay::from_input_unknown_native_handle_close_handle_observations(&observations)
            .unwrap();
    assert_eq!(observed.attempts(), with_source.attempts());
}

#[test]
fn unknown_native_handle_close_handle_rejects_shape_drift() {
    let exact = unknown_native_handle_close_handle_attempt();

    let mut changed = exact.clone();
    changed.operation_tag = 30;
    assert_tampered_close_handle_rejected(changed);

    let mut changed = exact.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_tampered_close_handle_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(1),
        post_error: 6,
    });
    assert_tampered_close_handle_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: 9,
    });
    assert_tampered_close_handle_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Descriptor;
    assert_tampered_close_handle_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].operand_ordinal = 1;
    assert_tampered_close_handle_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
    assert_tampered_close_handle_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands.push(FilesystemScalarOperand {
        operand_ordinal: 1,
        value: FilesystemScalarOperandValue::I64(-1),
    });
    assert_tampered_close_handle_rejected(changed);

    for changed in nonempty_side_lane_attempts(exact) {
        assert_tampered_close_handle_rejected(changed);
    }
}

#[test]
fn unknown_native_handle_close_handle_rejects_handoff_and_invalid_prefix() {
    let exact = unknown_native_handle_close_handle_attempt();
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
        FilesystemReplay::from_input_unknown_native_handle_close_handle_observations(&observations)
            .is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_seek_attempt(0, 0), exact],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_close_handle_observations(&observations)
            .is_err()
    );
}

#[test]
fn unknown_native_handle_close_handle_executes_exact_replay_provider_free() {
    let replay = FilesystemReplay::from_input_unknown_native_handle_close_handle_record(
        CloseHandleRecord::new(None),
    )
    .unwrap();
    let outcome = interpret_entry(
        &checked_unknown_native_handle_close_handle(),
        "Main::close_unknown",
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
fn unknown_native_handle_final_path_records_preserve_exact_input_and_optional_source() {
    let buffer = vec![3, 5, 7, 11, 13];
    let record = FinalPathRecord::new(None, buffer.clone(), 4, 2).unwrap();
    assert!(record.source_input().is_none());
    assert_eq!(record.buffer(), buffer);
    assert_eq!(record.capacity(), 4);
    assert_eq!(record.flags(), 2);

    let without_source =
        FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_record(record)
            .unwrap();
    assert_eq!(without_source.attempts().len(), 1);
    let attempt = &without_source.attempts()[0];
    assert_eq!(attempt.operation_tag(), 31);
    assert!(unknown_native_handle_final_path_name_by_handle_attempt_is_exact(attempt));
    assert_eq!(
        unknown_native_handle_final_path_name_by_handle_from_exact_attempt(attempt),
        Some((buffer.as_slice(), 4, 2))
    );
    assert_eq!(attempt.result(), Some(FilesystemOperationResult::Scalar(0)));
    assert_eq!(attempt.post_error(), Some(6));
    assert!(without_source.executes_replay_attempt(0));
    assert!(!without_source.has_output_attempts());

    let with_source =
        FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_record(
            FinalPathRecord::new(Some(source_input()), buffer, 5, u32::MAX).unwrap(),
        )
        .unwrap();
    assert_eq!(
        with_source
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 31]
    );
    assert!((0..3).all(|index| !with_source.executes_replay_attempt(index)));
    assert!(with_source.executes_replay_attempt(3));
    assert!(!with_source.has_output_attempts());

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        with_source.attempts().to_vec(),
        Vec::new(),
    );
    let observed =
        FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_observations(
            &observations,
        )
        .unwrap();
    assert_eq!(observed.attempts(), with_source.attempts());
}

#[test]
fn unknown_native_handle_final_path_rejects_capacity_and_shape_drift() {
    assert!(FinalPathRecord::new(None, vec![0; 3], 4, 0).is_err());
    assert!(FinalPathRecord::new(None, Vec::new(), u64::MAX, 0).is_err());

    let exact = unknown_native_handle_final_path_name_by_handle_attempt(vec![1, 2, 3, 4], 3, 9);

    let mut changed = exact.clone();
    changed.operation_tag = 30;
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 6,
    });
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: 9,
    });
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Descriptor;
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[0].operand_ordinal = 1;
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[0].value = FilesystemScalarOperandValue::U64(5);
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[1].value = FilesystemScalarOperandValue::I32(9);
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions[0].operand_ordinal = 2;
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands[0].operand_ordinal = 2;
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operand_resolutions[0].bytes[0] ^= 1;
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands[0].pre_bytes[0] ^= 1;
    assert_tampered_final_path_rejected(changed);

    let mut changed = exact.clone();
    changed.mutable_byte_operands[0].post_bytes[0] ^= 1;
    assert_tampered_final_path_rejected(changed);

    for changed in nonempty_side_lane_attempts(exact) {
        assert_tampered_final_path_rejected(changed);
    }
}

#[test]
fn unknown_native_handle_final_path_rejects_handoff_and_non_source_prefix() {
    let exact = unknown_native_handle_final_path_name_by_handle_attempt(vec![0; 4], 4, 0);
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
        FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_observations(
            &observations,
        )
        .is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_seek_attempt(0, 0), exact],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_observations(
            &observations,
        )
        .is_err()
    );
}

#[test]
fn unknown_native_handle_final_path_executes_exact_replay_provider_free() {
    let replay =
        FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_record(
            FinalPathRecord::new(None, vec![0; 4], 4, 0).unwrap(),
        )
        .unwrap();
    let outcome = interpret_entry(
        &checked_unknown_native_handle_final_path_name_by_handle(),
        "Main::query_unknown",
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
