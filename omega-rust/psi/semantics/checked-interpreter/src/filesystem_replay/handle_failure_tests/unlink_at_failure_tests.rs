use crate::filesystem_replay::{
    FilesystemInputUnknownDescriptorUnlinkAtReplayRecord as UnlinkAtRecord,
    unknown_descriptor_unlink_at_attempt, unknown_descriptor_unlink_at_attempt_is_exact,
    unknown_descriptor_unlink_at_from_exact_attempt,
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

fn checked_unknown_descriptor_unlink_at() -> checked_trees::CheckedTrees {
    const SOURCE: &str = r#"
data Main { filesystem: FilesystemHost; result: i32; }

machine Main::unlink_unknown(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.unlink_at(-1, "entry.bin", 0);
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
            PathBuf::from("tests/unknown_descriptor_unlink_at.omg"),
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
        .expect("tokenize unlink_at replay fixture");
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
        .expect("parse unlink_at replay fixture");
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve unlink_at replay fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type unlink_at replay fixture");
    lower_typed_trees(typed).expect("check unlink_at replay fixture")
}

#[test]
fn record_retains_exact_component_flags_and_optional_source() {
    let source = source_input();
    let record = UnlinkAtRecord::new(Some(source.clone()), b"entry.bin".to_vec(), i32::MIN)
        .expect("safe relative component is admitted");
    assert_eq!(record.source_input(), Some(&source));
    assert_eq!(record.relative_component(), b"entry.bin");
    assert_eq!(record.flags(), i32::MIN);

    let replay = FilesystemReplay::from_input_unknown_descriptor_unlink_at_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 15]
    );
    assert!((0..3).all(|index| !replay.executes_replay_attempt(index)));
    assert!(replay.executes_replay_attempt(3));
    assert!(!replay.has_output_attempts());
    assert!(replay.expected_included_sources().is_empty());
    let attempt = replay.attempts().last().unwrap();
    assert!(unknown_descriptor_unlink_at_attempt_is_exact(attempt));
    assert_eq!(
        unknown_descriptor_unlink_at_from_exact_attempt(attempt),
        Some((&b"entry.bin"[..], i32::MIN))
    );
    assert_eq!(
        attempt.result(),
        Some(FilesystemOperationResult::Scalar(-1))
    );
    assert_eq!(attempt.post_error(), Some(9));
    assert!(attempt.logical_handle_output().is_none());

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    let observed =
        FilesystemReplay::from_input_unknown_descriptor_unlink_at_observations(&observations)
            .unwrap();
    assert_eq!(observed.attempts(), replay.attempts());
}

#[test]
fn record_rejects_unsafe_relative_components() {
    for rejected in [
        &b""[..],
        &b"."[..],
        &b".."[..],
        &b"nested/name"[..],
        &b"nested\\name"[..],
        &b"nul\0name"[..],
    ] {
        assert!(
            UnlinkAtRecord::new(None, rejected.to_vec(), 0).is_err(),
            "unexpected accepted relative component: {rejected:?}"
        );
    }
}

#[test]
fn observations_reject_shape_and_input_drift() {
    let exact = unknown_descriptor_unlink_at_attempt(b"entry.bin".to_vec(), -47);
    let identity = FilesystemLogicalHandleIdentity::new(9).unwrap();

    assert_mutation_rejected(exact.clone(), |attempt| attempt.operation_tag = 14);
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
        attempt.scalar_operands[0].value = FilesystemScalarOperandValue::U32(47);
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.scalar_operands.push(FilesystemScalarOperand {
            operand_ordinal: 3,
            value: FilesystemScalarOperandValue::I32(0),
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.byte_operands[0].operand_ordinal = 0;
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.byte_operands[0].bytes = b"nested/name".to_vec();
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.logical_handle_inputs.clear();
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
fn observations_reject_every_forbidden_side_lane() {
    let exact = unknown_descriptor_unlink_at_attempt(b"entry.bin".to_vec(), 0);
    let root = FilesystemGrantRootIdentity::new(1).unwrap();
    let identity = FilesystemLogicalHandleIdentity::new(9).unwrap();

    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.byte_operands.push(FilesystemByteOperand {
            operand_ordinal: 3,
            bytes: vec![1],
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.path_like_operands.push(FilesystemPathLikeOperand {
            operand_ordinal: 3,
            bytes: b"name".to_vec(),
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .rooted_path_operand_resolutions
            .push(FilesystemRootedPathOperandResolution {
                operand_ordinal: 3,
                root,
                relative_path: b"name".to_vec(),
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.returned_paths.push(FilesystemReturnedPath {
            operand_ordinal: 3,
            kind: FilesystemReturnedPathKind::FinalPath,
            completeness: FilesystemReturnedPathCompleteness::Complete,
            bytes: b"name".to_vec(),
        });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .observed_byte_regions
            .push(FilesystemObservedByteRegion {
                output_operand_ordinal: 3,
                kind: FilesystemObservedByteRegionKind::SequentialFileRead,
                offset: 0,
                length: 1,
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .metadata_observations
            .push(FilesystemMetadataObservation::new(
                3,
                FilesystemMetadataObservationKind::OpenDescriptor,
                0,
                0,
                0,
            ));
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .mutable_byte_operand_resolutions
            .push(FilesystemMutableByteOperandResolution {
                operand_ordinal: 3,
                bytes: vec![1],
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .mutable_i64_operand_resolutions
            .push(FilesystemMutableI64OperandResolution {
                operand_ordinal: 3,
                value: 1,
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .mutable_byte_operands
            .push(FilesystemMutableByteOperand {
                operand_ordinal: 3,
                pre_bytes: vec![1],
                post_bytes: vec![1],
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt
            .mutable_i64_operands
            .push(FilesystemMutableI64Operand {
                operand_ordinal: 3,
                pre_value: 1,
                post_value: 1,
            });
    });
    assert_mutation_rejected(exact.clone(), |attempt| {
        attempt.authorized_paths.push(FilesystemAuthorizedPath {
            operand_ordinal: 3,
            access: FilesystemGrantAccess::Write,
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
            operand_ordinal: 3,
            access: FilesystemGrantAccess::Write,
            reason: FilesystemGrantRefusalReason::OutsideGrantedRoots,
        });
    });
}

#[test]
fn rejects_handoff_and_extra_operation() {
    let exact = unknown_descriptor_unlink_at_attempt(b"entry.bin".to_vec(), 0);
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
        FilesystemReplay::from_input_unknown_descriptor_unlink_at_observations(&observations)
            .is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![exact.clone(), exact],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_unlink_at_observations(&observations)
            .is_err()
    );
}

#[test]
fn executes_exact_replay_provider_free() {
    let replay = FilesystemReplay::from_input_unknown_descriptor_unlink_at_record(
        UnlinkAtRecord::new(None, b"entry.bin".to_vec(), 0).unwrap(),
    )
    .unwrap();
    let checked = checked_unknown_descriptor_unlink_at();
    let outcome = interpret_entry_with_options(
        &checked,
        "Main::unlink_unknown",
        &[],
        InterpretOptions {
            filesystem: FilesystemAccess::ReplayFilesystem(replay),
            ..InterpretOptions::default()
        },
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 0);

    for record in [
        UnlinkAtRecord::new(None, b"changed.bin".to_vec(), 0).unwrap(),
        UnlinkAtRecord::new(None, b"entry.bin".to_vec(), 128).unwrap(),
    ] {
        let replay =
            FilesystemReplay::from_input_unknown_descriptor_unlink_at_record(record).unwrap();
        let changed = interpret_entry_with_options(
            &checked,
            "Main::unlink_unknown",
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
        FilesystemReplay::from_input_unknown_descriptor_unlink_at_observations(&observations)
            .is_err()
    );
}
