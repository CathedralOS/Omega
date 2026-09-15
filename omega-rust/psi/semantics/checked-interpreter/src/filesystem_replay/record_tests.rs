use super::{
    BuildIncludedSource, EvaluationObservations, FILESYSTEM_METADATA_API_CARRIER_BYTES,
    FILESYSTEM_REPLAY_ATTEMPT_RETENTION_WEIGHT, FILESYSTEM_REPLAY_BYTE_OPERAND_RETENTION_WEIGHT,
    FilesystemByteOperand, FilesystemGrantAccess, FilesystemGrantRootIdentity,
    FilesystemInputOutputAbsentRemovesReplayRecord, FilesystemInputOutputReplayRecord,
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleOutputSource, FilesystemMetadataObservation,
    FilesystemMetadataObservationKind, FilesystemObservationProvider, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemOutputAbsentRemoveKind,
    FilesystemOutputAbsentRemoveReplayRecord, FilesystemOutputDuplicateReplayRecord,
    FilesystemOutputFileOperationReplayRecord, FilesystemOutputFileReplayRecord,
    FilesystemOutputWriteReplayKind, FilesystemOutputWriteReplayRecord, FilesystemReplay,
    FilesystemReplayReadKind, FilesystemReplayReadRecord, FilesystemScalarOperand,
    FilesystemScalarOperandValue, FilesystemSourceDescriptorMetadataReplayRecord,
    FilesystemSourceInputReplayEventRecord, FilesystemSourceInputReplayRecord,
    FilesystemSourceReadChainReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES,
    MAX_FILESYSTEM_REPLAY_RETAINED_BYTES, MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT,
    output_absent_remove_attempt_is_exact, validate_filesystem_replay_size,
};
use crate::{FilesystemGrantRefusal, FilesystemGrantRefusalReason};

fn root(value: u32) -> FilesystemGrantRootIdentity {
    FilesystemGrantRootIdentity::new(value).expect("test root is nonzero")
}

fn source_input(identity: u64) -> FilesystemSourceInputReplayRecord {
    let read = FilesystemReplayReadRecord::new(
        FilesystemReplayReadKind::Sequential,
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("empty successful read is canonical");
    let chain = FilesystemSourceReadChainReplayRecord::new(
        root(1),
        b"inputs/table.txt".to_vec(),
        identity,
        0,
        vec![read],
        0,
    )
    .expect("source chain is canonical");
    FilesystemSourceInputReplayRecord::new(vec![FilesystemSourceInputReplayEventRecord::ReadChain(
        chain,
    )])
    .expect("source input is nonempty")
}

fn output_file(identity: u64) -> FilesystemOutputFileReplayRecord {
    let bytes = b"pub data Generated {}\n".to_vec();
    FilesystemOutputFileReplayRecord::new(
        root(2),
        b"table.generated.omg".to_vec(),
        identity,
        0,
        bytes.clone(),
        i64::try_from(bytes.len()).unwrap(),
        0,
        0,
    )
    .expect("full output write is canonical")
}

fn empty_output_file(identity: u64) -> FilesystemOutputFileReplayRecord {
    FilesystemOutputFileReplayRecord::empty(root(2), b"empty.bin".to_vec(), identity, 0, 0)
        .expect("freshly created and closed empty Output file is canonical")
}

#[test]
fn typed_absent_output_removes_form_a_closed_failure_only_replay() {
    let first = FilesystemOutputAbsentRemoveReplayRecord::new(
        FilesystemOutputAbsentRemoveKind::File,
        root(2),
        b"missing-first.bin".to_vec(),
    )
    .unwrap();
    let second = FilesystemOutputAbsentRemoveReplayRecord::new(
        FilesystemOutputAbsentRemoveKind::Directory,
        root(2),
        b"nested/missing-second".to_vec(),
    )
    .unwrap();
    let record = FilesystemInputOutputAbsentRemovesReplayRecord::new(
        Some(source_input(7)),
        vec![first, second],
    )
    .unwrap();
    let replay = FilesystemReplay::from_input_output_absent_removes_record(record).unwrap();

    assert!(replay.has_output_attempts());
    assert!(replay.output_entries().is_empty());
    assert!((0..3).all(|index| !replay.executes_replay_attempt(index)));
    assert!((3..5).all(|index| replay.executes_replay_attempt(index)));
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 9, 12]
    );
    for attempt in &replay.attempts()[3..] {
        assert_eq!(
            attempt.result(),
            Some(FilesystemOperationResult::Scalar(-1))
        );
        assert_eq!(attempt.post_error(), Some(2));
        assert!(output_absent_remove_attempt_is_exact(attempt));
    }
}

fn multiwrite_output_file(identity: u64) -> FilesystemOutputFileReplayRecord {
    FilesystemOutputFileReplayRecord::with_writes(
        root(2),
        b"multi.generated.omg".to_vec(),
        identity,
        0,
        vec![
            FilesystemOutputWriteReplayRecord::new(b"first".to_vec(), 5, 0).unwrap(),
            FilesystemOutputWriteReplayRecord::new(Vec::new(), 0, 0).unwrap(),
            FilesystemOutputWriteReplayRecord::new(b"second".to_vec(), 6, 0).unwrap(),
        ],
        0,
    )
    .expect("multiple complete sequential writes are canonical")
}

fn positioned_output_file(identity: u64) -> FilesystemOutputFileReplayRecord {
    FilesystemOutputFileReplayRecord::with_writes(
        root(2),
        b"positioned.generated.omg".to_vec(),
        identity,
        0,
        vec![
            FilesystemOutputWriteReplayRecord::new(b"head".to_vec(), 4, 0).unwrap(),
            FilesystemOutputWriteReplayRecord::positioned(8, b"tail".to_vec(), 4, 0).unwrap(),
            FilesystemOutputWriteReplayRecord::new(b"-cur".to_vec(), 4, 0).unwrap(),
            FilesystemOutputWriteReplayRecord::positioned(0, Vec::new(), 0, 0).unwrap(),
        ],
        0,
    )
    .expect("mixed complete sequential and positioned writes are canonical")
}

fn descriptor_metadata_input(identity: u64) -> FilesystemSourceInputReplayRecord {
    let carrier = vec![0; FILESYSTEM_METADATA_API_CARRIER_BYTES];
    let metadata = FilesystemMetadataObservation::new(
        1,
        FilesystemMetadataObservationKind::OpenDescriptor,
        0o100444,
        23,
        1_000_000_000,
    );
    let event = FilesystemSourceDescriptorMetadataReplayRecord::new(
        root(1),
        b"main.omg".to_vec(),
        identity,
        0,
        0,
        carrier.clone(),
        carrier.clone(),
        carrier,
        metadata,
        0,
    )
    .expect("descriptor metadata event is canonical");
    FilesystemSourceInputReplayRecord::new(vec![
        FilesystemSourceInputReplayEventRecord::DescriptorMetadata(event),
    ])
    .expect("descriptor metadata is a source event")
}

fn typed_replay() -> FilesystemReplay {
    let output = output_file(2);
    let included = BuildIncludedSource::from_coordinate(
        output.output_root(),
        output.output_relative_path().to_vec(),
        6,
    )
    .expect("handoff path is canonical");
    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], vec![included])
            .expect("Source and Output coordinates are distinct");
    FilesystemReplay::from_input_output_record(record).expect("typed replay fits policy")
}

#[test]
fn typed_input_output_record_emits_one_exact_chain_and_handoff() {
    let source_only = FilesystemReplay::from_source_input_record(source_input(1)).unwrap();
    assert!(source_only.output_files().is_empty());
    assert!(source_only.expected_included_sources().is_empty());

    let replay = typed_replay();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 8]
    );
    let outputs = replay.output_files();
    let [output] = outputs.as_slice() else {
        panic!("one Output file is typed")
    };
    assert_eq!(output.output_root(), root(2));
    assert_eq!(output.output_relative_path(), b"table.generated.omg");
    assert_eq!(output.logical_handle_identity().get(), 2);
    assert_eq!(output.create_mode(), 438);
    let [FilesystemOutputFileOperationReplayRecord::Write(write)] = output.operations() else {
        panic!("singleton Output file retains one write")
    };
    assert_eq!(write.result(), write.bytes().len() as i64);
    let [included] = replay.expected_included_sources() else {
        panic!("one handoff coordinate is retained")
    };
    assert_eq!(included.root(), output.output_root());
    assert_eq!(included.relative_path(), output.output_relative_path());
}

#[test]
fn typed_input_output_record_retains_an_ordinary_file_without_handoff() {
    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output_file(2)], Vec::new())
            .expect("ordinary output needs no generated-source handoff");
    let replay = FilesystemReplay::from_input_output_record(record)
        .expect("ordinary output replay fits policy");
    assert_eq!(replay.output_files().len(), 1);
    assert!(replay.expected_included_sources().is_empty());

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    let decoded = FilesystemReplay::from_input_output_observations(&observations)
        .expect("observed ordinary output is accepted");
    assert!(decoded.expected_included_sources().is_empty());
}

#[test]
fn typed_input_output_record_retains_multiple_ordinary_files_without_handoff() {
    let mut second = output_file(3);
    second.output_relative_path = b"metadata.bin".to_vec();
    let record = FilesystemInputOutputReplayRecord::new(
        source_input(1),
        vec![output_file(2), second],
        Vec::new(),
    )
    .expect("distinct ordinary outputs need no generated-source handoff");
    let replay = FilesystemReplay::from_input_output_record(record)
        .expect("multiple ordinary output replay fits policy");
    assert_eq!(replay.output_files().len(), 2);
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 8, 1, 5, 8]
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    let decoded = FilesystemReplay::from_input_output_observations(&observations)
        .expect("observed ordinary outputs are accepted");
    assert_eq!(decoded.output_files().len(), 2);
    assert!(decoded.expected_included_sources().is_empty());
}

#[test]
fn typed_output_file_retains_one_or_more_full_sequential_writes() {
    let output = multiwrite_output_file(2);
    let handoff = BuildIncludedSource::from_coordinate(
        output.output_root(),
        output.output_relative_path().to_vec(),
        8,
    )
    .unwrap();
    let record = FilesystemInputOutputReplayRecord::new(
        source_input(1),
        vec![output],
        vec![handoff.clone()],
    )
    .expect("handoff follows the variable-length output close");
    let replay = FilesystemReplay::from_input_output_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 5, 5, 8]
    );
    let decoded_chains = replay.output_files();
    let [decoded] = decoded_chains.as_slice() else {
        panic!("one Output file is retained")
    };
    assert_eq!(
        decoded
            .operations()
            .iter()
            .filter_map(|operation| match operation {
                FilesystemOutputFileOperationReplayRecord::Write(write) => Some(write),
                _ => None,
            })
            .flat_map(FilesystemOutputWriteReplayRecord::bytes)
            .copied()
            .collect::<Vec<_>>(),
        b"firstsecond"
    );
    assert_eq!(
        replay.expected_included_sources(),
        std::slice::from_ref(&handoff)
    );

    assert!(FilesystemOutputWriteReplayRecord::new(vec![1, 2], 1, 0).is_err());
    let mut no_writes = replay.attempts().to_vec();
    no_writes.drain(4..7);
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(no_writes, Vec::new());
    let empty = FilesystemReplay::from_input_output_observations(&observations)
        .expect("create-close is an exact empty ordinary Output file");
    assert!(empty.output_files()[0].replayed_bytes().unwrap().is_empty());

    let mut partial_write = replay.attempts().to_vec();
    partial_write[4].outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(4),
        post_error: 0,
    });
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        partial_write,
        vec![handoff.clone()],
    );
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

    let mut wrong_descriptor = replay.attempts().to_vec();
    wrong_descriptor[6].logical_handle_inputs[0].resolution =
        FilesystemLogicalHandleInputResolution::Resolved(
            FilesystemLogicalHandleIdentity::new(99).unwrap(),
        );
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(wrong_descriptor, vec![handoff]);
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
}

#[test]
fn typed_output_file_retains_full_positioned_writes_and_cursor_semantics() {
    let output = positioned_output_file(2);
    assert_eq!(output.replayed_bytes().unwrap(), b"head-curtail");
    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new()).unwrap();
    let replay = FilesystemReplay::from_input_output_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 7, 5, 7, 8]
    );
    let decoded = FilesystemReplay::from_input_output_observations(
        &EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        ),
    )
    .expect("positioned output observations retain exact operation shape");
    let output_files = decoded.output_files();
    let [decoded] = output_files.as_slice() else {
        panic!("one positioned Output file is retained")
    };
    assert_eq!(decoded.replayed_bytes().unwrap(), b"head-curtail");
    let FilesystemOutputFileOperationReplayRecord::Write(positioned) = &decoded.operations()[1]
    else {
        panic!("second Output operation is a positioned write")
    };
    assert_eq!(
        positioned.kind(),
        FilesystemOutputWriteReplayKind::Positioned { offset: 8 }
    );

    assert!(FilesystemOutputWriteReplayRecord::positioned(-1, vec![1], 1, 0).is_err());
    let zero_length_beyond_extent = FilesystemOutputFileReplayRecord::with_writes(
        root(2),
        b"empty.bin".to_vec(),
        3,
        0,
        vec![FilesystemOutputWriteReplayRecord::positioned(1, Vec::new(), 0, 0).unwrap()],
        0,
    )
    .expect("zero-length positioned writes do not extend Output");
    assert!(
        zero_length_beyond_extent
            .replayed_bytes()
            .unwrap()
            .is_empty()
    );

    let sparse_over_ceiling = FilesystemOutputFileReplayRecord::with_writes(
        root(2),
        b"sparse.bin".to_vec(),
        3,
        0,
        vec![
            FilesystemOutputWriteReplayRecord::positioned(
                i64::try_from(MAX_FILESYSTEM_REPLAY_RETAINED_BYTES).unwrap(),
                vec![1],
                1,
                0,
            )
            .unwrap(),
        ],
        0,
    )
    .expect("typed positioned chain is valid before replay allocation policy");
    let sparse_record = FilesystemInputOutputReplayRecord::new(
        source_input(1),
        vec![sparse_over_ceiling],
        Vec::new(),
    )
    .unwrap();
    assert!(FilesystemReplay::from_input_output_record(sparse_record).is_err());

    let mut wrong_offset_lane = replay.attempts().to_vec();
    wrong_offset_lane[5].scalar_operands[0].operand_ordinal = 1;
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(wrong_offset_lane, Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
}

#[test]
fn typed_output_file_retains_create_close_without_synthetic_write() {
    let output = empty_output_file(2);
    assert!(output.operations().is_empty());
    assert!(output.replayed_bytes().unwrap().is_empty());
    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new()).unwrap();
    let replay = FilesystemReplay::from_input_output_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 8]
    );
    let decoded = FilesystemReplay::from_input_output_observations(
        &EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        ),
    )
    .expect("create-close Output file observations are exact");
    let output_files = decoded.output_files();
    let [decoded] = output_files.as_slice() else {
        panic!("one empty Output file is retained")
    };
    assert!(decoded.operations().is_empty());
    assert!(decoded.replayed_bytes().unwrap().is_empty());

    let mut incomplete = replay.attempts().to_vec();
    incomplete.pop();
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(incomplete, Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
}

#[test]
fn typed_output_file_retains_sync_operations_in_authored_order() {
    let output = FilesystemOutputFileReplayRecord::with_operations(
        root(2),
        b"synced.bin".to_vec(),
        2,
        0,
        vec![
            FilesystemOutputFileOperationReplayRecord::Sync,
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"first".to_vec(), 5, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::SyncData,
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"second".to_vec(), 6, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::Sync,
        ],
        0,
    )
    .unwrap();
    assert_eq!(output.replayed_bytes().unwrap(), b"firstsecond");
    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new()).unwrap();
    let replay = FilesystemReplay::from_input_output_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 43, 5, 44, 5, 43, 8]
    );
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    let decoded = FilesystemReplay::from_input_output_observations(&observations)
        .expect("successful sync operations are exact Output operations");
    assert_eq!(decoded.output_files()[0].operations().len(), 5);
    assert_eq!(
        decoded.output_files()[0].replayed_bytes().unwrap(),
        b"firstsecond"
    );

    let mut malformed = replay.attempts().to_vec();
    malformed[4].scalar_operands.push(FilesystemScalarOperand {
        operand_ordinal: 1,
        value: FilesystemScalarOperandValue::I32(0),
    });
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(malformed, Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
}

#[test]
fn typed_output_file_replays_exact_duplicate_and_immediate_retirement() {
    let output = FilesystemOutputFileReplayRecord::with_operations(
        root(2),
        b"duplicated.bin".to_vec(),
        2,
        0,
        vec![
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"before".to_vec(), 6, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(
                FilesystemOutputDuplicateReplayRecord::new(3).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"after".to_vec(), 5, 0).unwrap(),
            ),
        ],
        0,
    )
    .unwrap();
    assert_eq!(output.replayed_bytes().unwrap(), b"beforeafter");
    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new()).unwrap();
    let replay = FilesystemReplay::from_input_output_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 45, 8, 5, 8]
    );
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    let decoded = FilesystemReplay::from_input_output_observations(&observations)
        .expect("successful duplicate and immediate close are exact Output operations");
    assert!(matches!(
        decoded.output_files()[0].operations()[1],
        FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(duplicate)
            if duplicate.logical_handle_identity().get() == 3
    ));

    let mut wrong_lineage = replay.attempts().to_vec();
    wrong_lineage[5]
        .logical_handle_output
        .as_mut()
        .unwrap()
        .source = FilesystemLogicalHandleOutputSource::Created;
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(wrong_lineage, Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

    let mut failed_duplicate = replay.attempts().to_vec();
    failed_duplicate[5].outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 9,
    });
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(failed_duplicate, Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

    let mut wrong_close = replay.attempts().to_vec();
    wrong_close[6].logical_handle_inputs[0].resolution =
        FilesystemLogicalHandleInputResolution::Resolved(
            FilesystemLogicalHandleIdentity::new(2).unwrap(),
        );
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(wrong_close, Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

    let source_colliding_output = FilesystemOutputFileReplayRecord::with_operations(
        root(2),
        b"source-collision.bin".to_vec(),
        2,
        0,
        vec![
            FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(
                FilesystemOutputDuplicateReplayRecord::new(1).unwrap(),
            ),
        ],
        0,
    )
    .unwrap();
    assert!(
        FilesystemInputOutputReplayRecord::new(
            source_input(1),
            vec![source_colliding_output],
            Vec::new(),
        )
        .is_err()
    );

    let over_quota = (0..=MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES)
        .map(|index| {
            FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(
                FilesystemOutputDuplicateReplayRecord::new(u64::try_from(index).unwrap() + 3)
                    .unwrap(),
            )
        })
        .collect();
    assert!(
        FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"too-many-duplicates.bin".to_vec(),
            2,
            0,
            over_quota,
            0,
        )
        .is_err()
    );
}

#[test]
fn typed_output_file_retains_exact_descriptor_permission_changes() {
    let output = FilesystemOutputFileReplayRecord::with_operations(
        root(2),
        b"tool.bin".to_vec(),
        2,
        0,
        vec![
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"tool".to_vec(), 4, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::SetFilePermissions { mode: 0o755 },
        ],
        0,
    )
    .unwrap();
    assert_eq!(output.replayed_file_permissions(), Some(0o755));
    assert!(output.replayed_executable());
    assert_eq!(output.replayed_bytes().unwrap(), b"tool");

    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new()).unwrap();
    let replay = FilesystemReplay::from_input_output_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 17, 8]
    );
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    let decoded = FilesystemReplay::from_input_output_observations(&observations)
        .expect("successful descriptor permission changes are exact Output operations");
    assert_eq!(
        decoded.output_files()[0].replayed_file_permissions(),
        Some(0o755)
    );

    let mut failed = replay.attempts().to_vec();
    failed[5].outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 1,
    });
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(failed, Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

    let mut wrong_descriptor = replay.attempts().to_vec();
    wrong_descriptor[5].logical_handle_inputs[0].resolution =
        FilesystemLogicalHandleInputResolution::Unknown;
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(wrong_descriptor, Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
}

#[test]
fn typed_output_file_retains_exact_descriptor_time_carrier() {
    let mut times = vec![0; 32];
    times[0..8].copy_from_slice(&11i64.to_le_bytes());
    times[16..24].copy_from_slice(&29i64.to_le_bytes());
    let output = FilesystemOutputFileReplayRecord::with_operations(
        root(2),
        b"dated.bin".to_vec(),
        2,
        0,
        vec![
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"dated".to_vec(), 5, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::SetFileTimes {
                times: times.clone(),
            },
        ],
        0,
    )
    .unwrap();
    assert_eq!(output.replayed_bytes().unwrap(), b"dated");

    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new()).unwrap();
    let replay = FilesystemReplay::from_input_output_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 42, 8]
    );
    let time_attempt = &replay.attempts()[5];
    assert_eq!(
        time_attempt.mutable_byte_operand_resolutions[0].bytes,
        times
    );
    assert_eq!(time_attempt.mutable_byte_operands[0].pre_bytes, times);
    assert_eq!(time_attempt.mutable_byte_operands[0].post_bytes, times);

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    let decoded = FilesystemReplay::from_input_output_observations(&observations)
        .expect("successful descriptor time changes are exact Output operations");
    assert!(matches!(
        &decoded.output_files()[0].operations()[1],
        FilesystemOutputFileOperationReplayRecord::SetFileTimes { times: retained }
            if retained == &times
    ));

    let mut changed_post_state = replay.attempts().to_vec();
    changed_post_state[5].mutable_byte_operands[0].post_bytes[0] ^= 1;
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(changed_post_state, Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

    assert!(
        FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"short-times.bin".to_vec(),
            2,
            0,
            vec![FilesystemOutputFileOperationReplayRecord::SetFileTimes { times: vec![0; 31] }],
            0,
        )
        .is_err()
    );

    let over_retention = FilesystemOutputFileReplayRecord::with_operations(
        root(2),
        b"large-times.bin".to_vec(),
        2,
        0,
        vec![FilesystemOutputFileOperationReplayRecord::SetFileTimes {
            times: vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES / 3 + 1],
        }],
        0,
    )
    .unwrap();
    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![over_retention], Vec::new())
            .unwrap();
    assert!(FilesystemReplay::from_input_output_record(record).is_err());
}

#[test]
fn typed_output_file_replays_set_length_without_moving_cursor() {
    let output = FilesystemOutputFileReplayRecord::with_operations(
        root(2),
        b"resized.bin".to_vec(),
        2,
        0,
        vec![
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"abcdef".to_vec(), 6, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::SetLength { length: 3 },
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"XY".to_vec(), 2, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::SetLength { length: 5 },
        ],
        0,
    )
    .unwrap();
    assert_eq!(output.replayed_bytes().unwrap(), b"abc\0\0");
    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new()).unwrap();
    let replay = FilesystemReplay::from_input_output_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 41, 5, 41, 8]
    );
    let decoded = FilesystemReplay::from_input_output_observations(
        &EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        ),
    )
    .expect("successful set_len operations are exact Output operations");
    assert_eq!(
        decoded.output_files()[0].replayed_bytes().unwrap(),
        b"abc\0\0"
    );

    assert!(
        FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"negative.bin".to_vec(),
            3,
            0,
            vec![FilesystemOutputFileOperationReplayRecord::SetLength { length: -1 }],
            0,
        )
        .is_err()
    );
    let mut malformed = replay.attempts().to_vec();
    malformed[5].scalar_operands[0].operand_ordinal = 0;
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(malformed, Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
}

#[test]
fn typed_output_file_replays_exact_seek_cursor_transitions() {
    let output = FilesystemOutputFileReplayRecord::with_operations(
        root(2),
        b"seeked.bin".to_vec(),
        2,
        0,
        vec![
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"abcdef".to_vec(), 6, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::Seek {
                offset: 2,
                whence: 0,
                result: 2,
            },
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"XY".to_vec(), 2, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::Seek {
                offset: -1,
                whence: 2,
                result: 5,
            },
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"Z".to_vec(), 1, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::Seek {
                offset: -3,
                whence: 1,
                result: 3,
            },
            FilesystemOutputFileOperationReplayRecord::Write(
                FilesystemOutputWriteReplayRecord::new(b"Q".to_vec(), 1, 0).unwrap(),
            ),
        ],
        0,
    )
    .unwrap();
    assert_eq!(output.replayed_bytes().unwrap(), b"abXQeZ");
    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], Vec::new()).unwrap();
    let replay = FilesystemReplay::from_input_output_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 10, 5, 10, 5, 10, 5, 8]
    );
    let decoded = FilesystemReplay::from_input_output_observations(
        &EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        ),
    )
    .expect("successful canonical seeks are exact Output operations");
    assert_eq!(
        decoded.output_files()[0].replayed_bytes().unwrap(),
        b"abXQeZ"
    );

    assert!(
        FilesystemOutputFileReplayRecord::with_operations(
            root(2),
            b"bad-seek.bin".to_vec(),
            3,
            0,
            vec![FilesystemOutputFileOperationReplayRecord::Seek {
                offset: 1,
                whence: 0,
                result: 2,
            }],
            0,
        )
        .is_err()
    );
    let mut malformed = replay.attempts().to_vec();
    malformed[5].scalar_operands[1].value = FilesystemScalarOperandValue::I32(9);
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(malformed, Vec::new());
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
}

#[test]
fn typed_input_output_record_retains_ordered_multiple_source_handoffs() {
    let first = output_file(2);
    let mut second = output_file(3);
    second.output_relative_path = b"other.generated.omg".to_vec();
    let handoffs = vec![
        BuildIncludedSource::from_coordinate(
            second.output_root(),
            second.output_relative_path().to_vec(),
            9,
        )
        .unwrap(),
        BuildIncludedSource::from_coordinate(
            first.output_root(),
            first.output_relative_path().to_vec(),
            9,
        )
        .unwrap(),
    ];
    let record = FilesystemInputOutputReplayRecord::new(
        source_input(1),
        vec![first, second],
        handoffs.clone(),
    )
    .expect("handoff order may differ from output-chain order after both closes");
    let replay = FilesystemReplay::from_input_output_record(record).unwrap();
    assert_eq!(replay.expected_included_sources(), handoffs);

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        handoffs.clone(),
    );
    let decoded = FilesystemReplay::from_input_output_observations(&observations).unwrap();
    assert_eq!(decoded.expected_included_sources(), handoffs);
}

#[test]
fn typed_descriptor_metadata_record_emits_one_closed_exact_event() {
    let replay = FilesystemReplay::from_source_input_record(descriptor_metadata_input(7)).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 39, 8]
    );
    let metadata = &replay.attempts()[1];
    assert_eq!(
        metadata.logical_handle_inputs[0].resolution,
        FilesystemLogicalHandleInputResolution::Resolved(
            FilesystemLogicalHandleIdentity::new(7).unwrap()
        )
    );
    assert_eq!(
        metadata.metadata_observations[0].kind(),
        FilesystemMetadataObservationKind::OpenDescriptor
    );

    let mut events = source_input(7).events;
    events.extend(descriptor_metadata_input(7).events);
    assert!(FilesystemSourceInputReplayRecord::new(events).is_err());
}

#[test]
fn typed_output_rejects_partial_noncanonical_and_overlapping_records() {
    assert!(
        FilesystemInputOutputReplayRecord::new(source_input(1), Vec::new(), Vec::new()).is_err()
    );
    assert!(
        FilesystemOutputFileReplayRecord::new(
            root(2),
            b"../escape.omg".to_vec(),
            2,
            0,
            vec![1],
            1,
            0,
            0,
        )
        .is_err()
    );
    assert!(
        FilesystemOutputFileReplayRecord::new(
            root(2),
            b"generated.omg".to_vec(),
            2,
            0,
            vec![1, 2],
            1,
            0,
            0,
        )
        .is_err()
    );
    let output = output_file(1);
    let included = BuildIncludedSource::from_coordinate(
        output.output_root(),
        output.output_relative_path().to_vec(),
        6,
    )
    .unwrap();
    assert!(
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], vec![included])
            .is_err()
    );

    let output = output_file(2);
    let duplicate_handoff = BuildIncludedSource::from_coordinate(
        output.output_root(),
        output.output_relative_path().to_vec(),
        6,
    )
    .unwrap();
    assert!(
        FilesystemInputOutputReplayRecord::new(
            source_input(1),
            vec![output],
            vec![duplicate_handoff.clone(), duplicate_handoff],
        )
        .is_err()
    );

    let first = output_file(2);
    let duplicate_path = output_file(3);
    assert!(
        FilesystemInputOutputReplayRecord::new(
            source_input(1),
            vec![first, duplicate_path],
            Vec::new(),
        )
        .is_err()
    );

    let first = output_file(2);
    let mut duplicate_descriptor = output_file(2);
    duplicate_descriptor.output_relative_path = b"other.bin".to_vec();
    assert!(
        FilesystemInputOutputReplayRecord::new(
            source_input(1),
            vec![first, duplicate_descriptor],
            Vec::new(),
        )
        .is_err()
    );

    let output = output_file(2);
    let wrong_handoff =
        BuildIncludedSource::from_coordinate(root(2), b"other.omg".to_vec(), 6).unwrap();
    assert!(
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], vec![wrong_handoff],)
            .is_err()
    );

    let output = output_file(2);
    let early_handoff = BuildIncludedSource::from_coordinate(
        output.output_root(),
        output.output_relative_path().to_vec(),
        5,
    )
    .unwrap();
    assert!(
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], vec![early_handoff],)
            .is_err()
    );

    let first = output_file(2);
    let mut second = output_file(3);
    second.output_relative_path = b"other.generated.omg".to_vec();
    let early_second = BuildIncludedSource::from_coordinate(
        second.output_root(),
        second.output_relative_path().to_vec(),
        6,
    )
    .unwrap();
    assert!(
        FilesystemInputOutputReplayRecord::new(
            source_input(1),
            vec![first, second],
            vec![early_second],
        )
        .is_err()
    );
}

#[test]
fn observed_input_output_replay_rejects_nonexact_output_lanes() {
    let replay = typed_replay();
    let included = replay.expected_included_sources()[0].clone();
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        vec![included.clone()],
    );
    let decoded = FilesystemReplay::from_input_output_observations(&observations)
        .expect("exact observed chain is accepted");
    assert_eq!(decoded.expected_included_sources(), &[included]);

    let mut attempts = replay.attempts().to_vec();
    let output_start = attempts.len() - 3;
    attempts[output_start].scalar_operands[0].value = FilesystemScalarOperandValue::I32(511);
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        attempts,
        replay.expected_included_sources().to_vec(),
    );
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

    let early_handoff = BuildIncludedSource::from_coordinate(
        root(2),
        b"table.generated.omg".to_vec(),
        output_start + 2,
    )
    .unwrap();
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        vec![early_handoff],
    );
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

    let mut attempts = replay.attempts().to_vec();
    attempts[output_start + 1].grant_refusals = vec![FilesystemGrantRefusal {
        operand_ordinal: 0,
        access: FilesystemGrantAccess::Write,
        reason: FilesystemGrantRefusalReason::OutsideGrantedRoots,
    }];
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        attempts,
        replay.expected_included_sources().to_vec(),
    );
    assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
}

#[test]
fn replay_retention_has_a_lower_aggregate_clone_ceiling() {
    let bytes = vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES + 1];
    let output = FilesystemOutputFileReplayRecord::new(
        root(2),
        b"large.generated.omg".to_vec(),
        2,
        0,
        bytes,
        i64::try_from(MAX_FILESYSTEM_REPLAY_RETAINED_BYTES + 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    let included = BuildIncludedSource::from_coordinate(
        output.output_root(),
        output.output_relative_path().to_vec(),
        6,
    )
    .unwrap();
    let record =
        FilesystemInputOutputReplayRecord::new(source_input(1), vec![output], vec![included])
            .expect("large typed record is valid before replay-retention policy");
    assert!(FilesystemReplay::from_input_output_record(record).is_err());
}

#[test]
fn replay_retention_weight_accepts_the_exact_limit_and_rejects_one_more_unit() {
    let payload_length = MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT
        - FILESYSTEM_REPLAY_ATTEMPT_RETENTION_WEIGHT
        - FILESYSTEM_REPLAY_BYTE_OPERAND_RETENTION_WEIGHT;
    let attempt = FilesystemOperationAttempt {
        operation_tag: 0,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: 0,
        }),
        scalar_operands: Vec::new(),
        byte_operands: vec![FilesystemByteOperand {
            operand_ordinal: 0,
            bytes: vec![0; payload_length],
        }],
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
    };
    let mut attempts = vec![attempt];
    assert!(validate_filesystem_replay_size(&attempts).is_ok());
    attempts[0].byte_operands[0].bytes.push(0);
    assert!(validate_filesystem_replay_size(&attempts).is_err());
}
