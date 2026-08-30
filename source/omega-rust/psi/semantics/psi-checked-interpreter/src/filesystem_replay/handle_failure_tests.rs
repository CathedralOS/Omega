mod open_at_failure_tests;
mod unlink_at_failure_tests;

use super::{
    FilesystemInputUnknownDescriptorGetOsfHandleReplayRecord as GetOsfHandleRecord,
    FilesystemInputUnknownDescriptorOperationReplayKind as Kind,
    FilesystemInputUnknownDescriptorOperationReplayRecord as Record,
    FilesystemInputUnknownDescriptorReadFileMetadataReplayRecord as ReadFileMetadataRecord,
    FilesystemInputUnknownDescriptorReadReplayKind as ReadKind,
    FilesystemInputUnknownDescriptorReadReplayRecord as ReadRecord,
    FilesystemInputUnknownDescriptorSeekReplayRecord as SeekRecord,
    FilesystemInputUnknownDescriptorSetFileTimesReplayRecord as SetFileTimesRecord,
    FilesystemInputUnknownDescriptorWriteOperationReplayKind as WriteKind,
    FilesystemInputUnknownDescriptorWriteOperationReplayRecord as WriteRecord,
    FilesystemInputUnknownDescriptorWriteReplayKind as PayloadWriteKind,
    FilesystemInputUnknownDescriptorWriteReplayRecord as PayloadWriteRecord,
    FilesystemInputUnknownNativeHandleCloseHandleReplayRecord as CloseHandleRecord,
    FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord as FinalPathRecord,
    unknown_descriptor_get_osfhandle_attempt, unknown_descriptor_get_osfhandle_attempt_is_exact,
    unknown_descriptor_operation_attempt, unknown_descriptor_operation_from_exact_attempt,
    unknown_descriptor_read_attempt, unknown_descriptor_read_file_metadata_attempt,
    unknown_descriptor_read_file_metadata_from_exact_attempt,
    unknown_descriptor_read_from_exact_attempt, unknown_descriptor_seek_attempt,
    unknown_descriptor_seek_from_exact_attempt, unknown_descriptor_set_file_times_attempt,
    unknown_descriptor_set_file_times_from_exact_attempt, unknown_descriptor_write_attempt,
    unknown_descriptor_write_from_exact_attempt, unknown_descriptor_write_operation_attempt,
    unknown_descriptor_write_operation_from_exact_attempt,
    unknown_native_handle_close_handle_attempt,
    unknown_native_handle_close_handle_attempt_is_exact,
    unknown_native_handle_final_path_name_by_handle_attempt,
    unknown_native_handle_final_path_name_by_handle_attempt_is_exact,
    unknown_native_handle_final_path_name_by_handle_from_exact_attempt,
};
use crate::{
    BuildIncludedSource, EvaluationObservations, FILESYSTEM_METADATA_API_CARRIER_BYTES,
    FilesystemAccess, FilesystemAuthorizedPath, FilesystemByteOperand,
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
    FilesystemSourceReadChainReplayRecord, InterpretOptions, MAX_FILESYSTEM_REPLAY_RETAINED_BYTES,
    interpret_entry_with_options,
};
use psi_source::{SourceMap, SourceOrigin};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources;
use psi_tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
use psi_typed_trees_to_checked_trees::lower_typed_trees;
use std::{path::PathBuf, sync::Arc};

const FILESYSTEM_HOST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/std/filesystem_host.omg"
));

pub(super) const KINDS_AND_TAGS: [(Kind, u16); 4] = [
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

pub(super) fn source_input() -> FilesystemSourceInputReplayRecord {
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

fn checked_unknown_descriptor_read_file_metadata() -> psi_checked_trees::CheckedTrees {
    const SOURCE: &str = r#"
data Main { filesystem: FilesystemHost; result: i32; buffer: [u8; 144]; }

machine Main::read_unknown(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.read_file_metadata(-1, &mut self.buffer);
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
            PathBuf::from("tests/unknown_descriptor_read_file_metadata.omg"),
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
        .expect("tokenize read_file_metadata replay fixture");
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
        .expect("parse read_file_metadata replay fixture");
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve read_file_metadata replay fixture");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type read_file_metadata replay fixture");
    lower_typed_trees(typed).expect("check read_file_metadata replay fixture")
}

fn checked_unknown_descriptor_get_osfhandle() -> psi_checked_trees::CheckedTrees {
    const SOURCE: &str = r#"
data Main { filesystem: FilesystemHost; result: i64; }

machine Main::get_unknown(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.get_osfhandle(-1);
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
            PathBuf::from("tests/unknown_descriptor_get_osfhandle.omg"),
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
        .expect("tokenize get_osfhandle replay fixture");
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
        .expect("parse get_osfhandle replay fixture");
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve get_osfhandle replay fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type get_osfhandle replay fixture");
    lower_typed_trees(typed).expect("check get_osfhandle replay fixture")
}

fn checked_unknown_native_handle_close_handle() -> psi_checked_trees::CheckedTrees {
    const SOURCE: &str = r#"
data Main { filesystem: FilesystemHost; result: i32; }

machine Main::close_unknown(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.close_handle(-1);
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
            PathBuf::from("tests/unknown_native_handle_close_handle.omg"),
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
        .expect("tokenize close_handle replay fixture");
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
        .expect("parse close_handle replay fixture");
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve close_handle replay fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type close_handle replay fixture");
    lower_typed_trees(typed).expect("check close_handle replay fixture")
}

fn checked_unknown_native_handle_final_path_name_by_handle() -> psi_checked_trees::CheckedTrees {
    const SOURCE: &str = r#"
data Main { filesystem: FilesystemHost; result: i64; buffer: [u8; 4]; }

machine Main::query_unknown(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.final_path_name_by_handle(-1, &mut self.buffer, 4, 0);
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
            PathBuf::from("tests/unknown_native_handle_final_path_name_by_handle.omg"),
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
        .expect("tokenize final_path_name_by_handle replay fixture");
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
        .expect("parse final_path_name_by_handle replay fixture");
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve final_path_name_by_handle replay fixture");
    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("type final_path_name_by_handle replay fixture");
    lower_typed_trees(typed).expect("check final_path_name_by_handle replay fixture")
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
    let outcome = interpret_entry_with_options(
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

fn assert_tampered_get_osfhandle_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_observations(&observations)
            .is_err()
    );
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
    let outcome = interpret_entry_with_options(
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

fn assert_tampered_close_handle_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_close_handle_observations(&observations)
            .is_err()
    );
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
    let outcome = interpret_entry_with_options(
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

fn assert_tampered_final_path_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_observations(
            &observations,
        )
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

fn assert_tampered_set_file_times_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_set_file_times_observations(&observations)
            .is_err()
    );
}

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

fn assert_tampered_read_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
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

    let outcome = interpret_entry_with_options(
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

fn assert_tampered_read_file_metadata_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
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

fn assert_tampered_payload_write_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_write_observations(&observations).is_err()
    );
}
