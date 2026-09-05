use super::{
    FilesystemInputUnknownDescriptorReadDirReplayRecord as ReadDirRecord,
    unknown_descriptor_read_dir_attempt, unknown_descriptor_read_dir_attempt_is_exact,
    unknown_descriptor_read_dir_from_exact_attempt,
};
use crate::{
    BuildIncludedSource, EvaluationObservations, FilesystemAccess, FilesystemAuthorizedPath,
    FilesystemByteOperand, FilesystemGrantAccess, FilesystemGrantRefusal,
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
    FilesystemSourceReadChainReplayRecord, InterpretOptions, interpret_entry_with_options,
};
use source::{SourceMap, SourceOrigin};
use source_files_to_tokens::Lexer;
use std::{path::PathBuf, sync::Arc};
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources;
use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
use typed_trees_to_checked_trees::lower_typed_trees;

const FILESYSTEM_HOST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/std/filesystem_host.omg"
));

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
        FilesystemGrantRootIdentity::new(1).unwrap(),
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

fn checked_unknown_descriptor_read_dir() -> checked_trees::CheckedTrees {
    const SOURCE: &str = r#"
data Main {
    filesystem: FilesystemHost;
    result: i64;
    buffer: [u8; 4];
    position: i64;
}

machine Main::read_unknown(&mut self)
reaches FilesystemHost
{
    self.position = -47;
    self.result = self.filesystem.read_dir(
        -1,
        &mut self.buffer,
        3,
        &mut self.position
    );
}
"#;
    let mut sources = SourceMap::default();
    let filesystem_host_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/std/filesystem_host.omg"),
            FILESYSTEM_HOST.to_owned(),
            PathBuf::from("source/library/std"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("tests/unknown_descriptor_read_dir.omg"),
            SOURCE.to_owned(),
            PathBuf::from("tests"),
            None,
            SourceOrigin::User,
        )
        .source_id;
    let filesystem_host_tokens = Lexer::new(FILESYSTEM_HOST)
        .tokenize()
        .expect("tokenize canonical filesystem host");
    let mut syntax = parse_syntax_trees_with_id(filesystem_host_source_id, &filesystem_host_tokens)
        .expect("parse canonical filesystem host");
    let tokens = Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize read_dir replay fixture");
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
        .expect("parse read_dir replay fixture");
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve read_dir replay fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type read_dir replay fixture");
    lower_typed_trees(typed).expect("check read_dir replay fixture")
}

#[test]
fn record_retains_exact_authored_inputs_and_optional_source() {
    let source = source_input();
    let authored_buffer = vec![3, 1, 4, 1, 5];
    let record = ReadDirRecord::new(Some(source.clone()), 3, authored_buffer.clone(), i64::MIN)
        .expect("bounded read_dir inputs are admitted");
    assert_eq!(record.source_input(), Some(&source));
    assert_eq!(record.requested_count(), 3);
    assert_eq!(record.buffer(), authored_buffer);
    assert_eq!(record.position(), i64::MIN);

    let replay = FilesystemReplay::from_input_unknown_descriptor_read_dir_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 23]
    );
    assert!((0..3).all(|index| !replay.executes_replay_attempt(index)));
    assert!(replay.executes_replay_attempt(3));
    assert!(!replay.has_output_attempts());
    assert!(replay.expected_included_sources().is_empty());

    let attempt = replay.attempts().last().unwrap();
    assert!(unknown_descriptor_read_dir_attempt_is_exact(attempt));
    assert_eq!(
        unknown_descriptor_read_dir_from_exact_attempt(attempt),
        Some((3, authored_buffer.as_slice(), i64::MIN))
    );
    assert_eq!(
        attempt.result(),
        Some(FilesystemOperationResult::Scalar(-1))
    );
    assert_eq!(attempt.post_error(), Some(9));
    assert_eq!(
        attempt.mutable_byte_operand_resolutions()[0].bytes(),
        authored_buffer
    );
    assert_eq!(
        attempt.mutable_byte_operands()[0].pre_bytes(),
        authored_buffer
    );
    assert_eq!(
        attempt.mutable_byte_operands()[0].post_bytes(),
        authored_buffer
    );
    assert_eq!(
        attempt.mutable_i64_operand_resolutions()[0].value(),
        i64::MIN
    );
    assert_eq!(attempt.mutable_i64_operands()[0].pre_value(), i64::MIN);
    assert_eq!(attempt.mutable_i64_operands()[0].post_value(), i64::MIN);

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    let observed =
        FilesystemReplay::from_input_unknown_descriptor_read_dir_observations(&observations)
            .unwrap();
    assert_eq!(observed.attempts(), replay.attempts());
}

#[test]
fn constructor_rejects_count_that_cannot_address_the_carrier() {
    assert!(ReadDirRecord::new(None, 5, vec![0; 4], 0).is_err());
    assert!(ReadDirRecord::new(None, u64::MAX, Vec::new(), 0).is_err());
    assert!(ReadDirRecord::new(None, 0, Vec::new(), 0).is_ok());
}

#[test]
fn observations_reject_fixed_shape_and_authored_lane_drift() {
    let exact = unknown_descriptor_read_dir_attempt(3, vec![3, 1, 4, 1, 5], -47);
    let identity = FilesystemLogicalHandleIdentity::new(9).unwrap();

    assert_mutation_rejected(exact.clone(), |attempt| attempt.operation_tag = 24);
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.provider = FilesystemObservationProvider::Virtual;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: 9,
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(-1),
            post_error: 13,
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.scalar_operands[0].operand_ordinal = 1;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.scalar_operands[0].value = FilesystemScalarOperandValue::I64(3);
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.scalar_operands[0].value = FilesystemScalarOperandValue::U64(6);
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.mutable_byte_operand_resolutions[0].operand_ordinal = 2;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.mutable_byte_operand_resolutions[0].bytes[0] = 8;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.mutable_byte_operands[0].operand_ordinal = 2;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.mutable_byte_operands[0].pre_bytes[0] = 8;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.mutable_byte_operands[0].post_bytes[0] = 8;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.mutable_i64_operand_resolutions[0].operand_ordinal = 2;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.mutable_i64_operand_resolutions[0].value = -46;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.mutable_i64_operands[0].operand_ordinal = 2;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.mutable_i64_operands[0].pre_value = -46;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.mutable_i64_operands[0].post_value = -46;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.logical_handle_inputs.clear();
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.logical_handle_inputs[0].operand_ordinal = 1;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Native;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
    });
    assert_mutation_rejected(exact, |attempt| {
        attempt.logical_handle_inputs[0].resolution =
            FilesystemLogicalHandleInputResolution::Resolved(identity);
    });
}

#[test]
fn observations_reject_extra_required_rows_and_every_forbidden_side_lane() {
    let exact = unknown_descriptor_read_dir_attempt(3, vec![3, 1, 4, 1, 5], -47);
    let root = FilesystemGrantRootIdentity::new(1).unwrap();
    let identity = FilesystemLogicalHandleIdentity::new(9).unwrap();

    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.scalar_operands.push(FilesystemScalarOperand {
            operand_ordinal: 4,
            value: FilesystemScalarOperandValue::U64(0),
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .mutable_byte_operand_resolutions
            .push(FilesystemMutableByteOperandResolution {
                operand_ordinal: 4,
                bytes: vec![1],
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .mutable_i64_operand_resolutions
            .push(FilesystemMutableI64OperandResolution {
                operand_ordinal: 4,
                value: 1,
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .mutable_byte_operands
            .push(FilesystemMutableByteOperand {
                operand_ordinal: 4,
                pre_bytes: vec![1],
                post_bytes: vec![1],
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .mutable_i64_operands
            .push(FilesystemMutableI64Operand {
                operand_ordinal: 4,
                pre_value: 1,
                post_value: 1,
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .logical_handle_inputs
            .push(FilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Unknown,
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.byte_operands.push(FilesystemByteOperand {
            operand_ordinal: 4,
            bytes: vec![1],
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.path_like_operands.push(FilesystemPathLikeOperand {
            operand_ordinal: 4,
            bytes: b"name".to_vec(),
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .rooted_path_operand_resolutions
            .push(FilesystemRootedPathOperandResolution {
                operand_ordinal: 4,
                root,
                relative_path: b"name".to_vec(),
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.returned_paths.push(FilesystemReturnedPath {
            operand_ordinal: 4,
            kind: FilesystemReturnedPathKind::FinalPath,
            completeness: FilesystemReturnedPathCompleteness::Complete,
            bytes: b"name".to_vec(),
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .observed_byte_regions
            .push(FilesystemObservedByteRegion {
                output_operand_ordinal: 1,
                kind: FilesystemObservedByteRegionKind::DirectoryRecords,
                offset: 0,
                length: 0,
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .metadata_observations
            .push(FilesystemMetadataObservation::new(
                1,
                FilesystemMetadataObservationKind::OpenDescriptor,
                0,
                0,
                0,
            ));
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.authorized_paths.push(FilesystemAuthorizedPath {
            operand_ordinal: 4,
            access: FilesystemGrantAccess::Read,
            root,
            relative_path: b"name".to_vec(),
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.logical_handle_output = Some(FilesystemLogicalHandleOutput {
            kind: FilesystemLogicalHandleKind::Descriptor,
            identity,
            source: FilesystemLogicalHandleOutputSource::Created,
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.retired_logical_handles.push(identity);
    });
    assert_mutation_rejected(exact, |attempt| {
        attempt.grant_refusals.push(FilesystemGrantRefusal {
            operand_ordinal: 4,
            access: FilesystemGrantAccess::Read,
            reason: FilesystemGrantRefusalReason::OutsideGrantedRoots,
        });
    });
}

#[test]
fn observations_accept_exact_prefix_and_reject_handoff_or_extra_operation() {
    let source = source_input();
    let replay = FilesystemReplay::from_input_unknown_descriptor_read_dir_record(
        ReadDirRecord::new(Some(source), 3, vec![0; 4], -47).unwrap(),
    )
    .unwrap();
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_read_dir_observations(&observations)
            .is_ok()
    );

    let exact = unknown_descriptor_read_dir_attempt(3, vec![0; 4], -47);
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
        FilesystemReplay::from_input_unknown_descriptor_read_dir_observations(&observations)
            .is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![exact.clone(), exact],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_read_dir_observations(&observations)
            .is_err()
    );
}

#[test]
fn executes_exact_replay_provider_free_with_empty_teardown() {
    let replay = FilesystemReplay::from_input_unknown_descriptor_read_dir_record(
        ReadDirRecord::new(None, 3, vec![0; 4], -47).unwrap(),
    )
    .unwrap();
    let checked = checked_unknown_descriptor_read_dir();
    let outcome = interpret_entry_with_options(
        &checked,
        "Main::read_unknown",
        &[],
        InterpretOptions {
            filesystem: FilesystemAccess::ReplayFilesystem(replay),
            ..InterpretOptions::default()
        },
    );
    assert_eq!(
        outcome.error, None,
        "clean completion includes successful replay-exhaustion teardown"
    );
    assert_eq!(outcome.exit_code, 0);

    for record in [
        ReadDirRecord::new(None, 2, vec![0; 4], -47).unwrap(),
        ReadDirRecord::new(None, 3, vec![1, 0, 0, 0], -47).unwrap(),
        ReadDirRecord::new(None, 3, vec![0; 4], -46).unwrap(),
    ] {
        let replay =
            FilesystemReplay::from_input_unknown_descriptor_read_dir_record(record).unwrap();
        let changed = interpret_entry_with_options(
            &checked,
            "Main::read_unknown",
            &[],
            InterpretOptions {
                filesystem: FilesystemAccess::ReplayFilesystem(replay),
                ..InterpretOptions::default()
            },
        );
        assert!(changed.error.is_some());
    }
}

fn assert_mutation_rejected(
    mut attempt: FilesystemOperationAttempt,
    mutate: impl FnOnce(&mut FilesystemOperationAttempt),
) {
    mutate(&mut attempt);
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_read_dir_observations(&observations)
            .is_err()
    );
}
