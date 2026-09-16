//! Fixtures shared by the handle failure tests: the filesystem host, the
//! kind and tag tables, source inputs and the tamper assertions.

mod descriptor_and_native_handle_records;
mod open_at_failure_tests;
mod read_and_metadata_observations;
mod seek_write_and_file_time_observations;
mod unlink_at_failure_tests;

use super::{
    FilesystemInputUnknownDescriptorOperationReplayKind as Kind,
    FilesystemInputUnknownDescriptorWriteOperationReplayKind as WriteKind,
    unknown_descriptor_seek_attempt,
};
use crate::{
    BuildIncludedSource, EvaluationObservations, FilesystemAccess, FilesystemAuthorizedPath,
    FilesystemByteOperand, FilesystemGrantAccess, FilesystemGrantRefusal,
    FilesystemGrantRefusalReason, FilesystemGrantRootIdentity, FilesystemLogicalHandleIdentity,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource,
    FilesystemMetadataObservation, FilesystemMetadataObservationKind, FilesystemMutableByteOperand,
    FilesystemMutableByteOperandResolution, FilesystemMutableI64Operand,
    FilesystemMutableI64OperandResolution, FilesystemObservationProvider,
    FilesystemObservedByteRegion, FilesystemObservedByteRegionKind, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemPathLikeOperand,
    FilesystemReplay, FilesystemReplayReadKind, FilesystemReplayReadRecord, FilesystemReturnedPath,
    FilesystemReturnedPathCompleteness, FilesystemReturnedPathKind,
    FilesystemRootedPathOperandResolution, FilesystemScalarOperandValue,
    FilesystemSourceInputReplayEventRecord, FilesystemSourceInputReplayRecord,
    FilesystemSourceReadChainReplayRecord, InterpretOptions, interpret_entry,
};
use source::{SourceMap, SourceOrigin};
use source_files_to_tokens::Lexer;
use std::{path::PathBuf, sync::Arc};
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
use typed_trees_to_checked_trees::lower_typed_trees;

const FILESYSTEM_HOST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/std/filesystem_host.omg"
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

fn checked_unknown_descriptor_read_file_metadata() -> checked_trees::CheckedTrees {
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
    let resolved = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve read_file_metadata replay fixture");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type read_file_metadata replay fixture");
    lower_typed_trees(typed).expect("check read_file_metadata replay fixture")
}

fn checked_unknown_descriptor_get_osfhandle() -> checked_trees::CheckedTrees {
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
    let resolved = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve get_osfhandle replay fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type get_osfhandle replay fixture");
    lower_typed_trees(typed).expect("check get_osfhandle replay fixture")
}

fn checked_unknown_native_handle_close_handle() -> checked_trees::CheckedTrees {
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
    let resolved = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve close_handle replay fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type close_handle replay fixture");
    lower_typed_trees(typed).expect("check close_handle replay fixture")
}

fn checked_unknown_native_handle_final_path_name_by_handle() -> checked_trees::CheckedTrees {
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
    let resolved = resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve final_path_name_by_handle replay fixture");
    let typed = lower_symbol_resolved_trees(&resolved)
        .expect("type final_path_name_by_handle replay fixture");
    lower_typed_trees(typed).expect("check final_path_name_by_handle replay fixture")
}

fn assert_tampered_operation_rejected(attempt: crate::FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_operation_observations(&observations)
            .is_err()
    );
}

fn assert_tampered_get_osfhandle_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_observations(&observations)
            .is_err()
    );
}

fn assert_tampered_close_handle_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_close_handle_observations(&observations)
            .is_err()
    );
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

fn assert_tampered_seek_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_seek_observations(&observations).is_err()
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

fn assert_tampered_set_file_times_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_set_file_times_observations(&observations)
            .is_err()
    );
}

fn assert_tampered_read_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_read_observations(&observations).is_err()
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

fn assert_tampered_payload_write_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_write_observations(&observations).is_err()
    );
}
