use super::native_mutation_failures::{
    FilesystemInputUnknownNativeHandleMutationReplayKind as Kind,
    FilesystemInputUnknownNativeHandleMutationReplayRecord as Record,
    unknown_native_handle_mutation_attempt, unknown_native_handle_mutation_attempt_is_exact,
    unknown_native_handle_mutation_from_exact_attempt,
};
use crate::{
    BuildIncludedSource, EvaluationObservations, FilesystemAccess, FilesystemByteOperand,
    FilesystemGrantRootIdentity, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemObservationProvider, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemReplay,
    FilesystemReplayReadKind, FilesystemReplayReadRecord, FilesystemScalarOperandValue,
    FilesystemSourceInputReplayEventRecord, FilesystemSourceInputReplayRecord,
    FilesystemSourceReadChainReplayRecord, FilesystemSponsor, FsGrants, InterpretOptions,
    MAX_FILESYSTEM_REPLAY_RETAINED_BYTES, interpret_entry_with_options,
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

const NATIVE_MUTATION_FIXTURE: &str = r#"
data SetFileTimeMain {
    filesystem: FilesystemHost;
    result: i32;
    last_access: [u8; 8];
    last_write: [u8; 8];
}

machine SetFileTimeMain::replay(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.set_file_time(-1, -7, &self.last_access, &self.last_write);
}

machine SetFileTimeMain::probe(&mut self) -> i32
reaches FilesystemHost
{
    self.result = self.filesystem.set_file_time(-1, -7, &self.last_access, &self.last_write);
    let error: i32 = self.filesystem.get_last_error();
    transition { _ -> (error) }
}

data LockFileExMain {
    filesystem: FilesystemHost;
    result: i32;
    overlapped: [u8; 32];
}

machine LockFileExMain::replay(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.lock_file_ex(-1, 3, 0, 47, 11, &mut self.overlapped);
}

machine LockFileExMain::probe(&mut self) -> i32
reaches FilesystemHost
{
    self.result = self.filesystem.lock_file_ex(-1, 3, 0, 47, 11, &mut self.overlapped);
    let error: i32 = self.filesystem.get_last_error();
    transition { _ -> (error) }
}

data UnlockFileMain { filesystem: FilesystemHost; result: i32; }

machine UnlockFileMain::replay(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.unlock_file(-1, 5, 7, 13, 17);
}

machine UnlockFileMain::probe(&mut self) -> i32
reaches FilesystemHost
{
    self.result = self.filesystem.unlock_file(-1, 5, 7, 13, 17);
    let error: i32 = self.filesystem.get_last_error();
    transition { _ -> (error) }
}
"#;

pub(super) fn kinds() -> [(Kind, u16); 3] {
    [
        (
            Kind::SetFileTime {
                creation: -7,
                last_access: vec![0; 8],
                last_write: vec![0; 8],
            },
            32,
        ),
        (
            Kind::LockFileEx {
                flags: 3,
                reserved: 0,
                length_low: 47,
                length_high: 11,
                overlapped: vec![0; 32],
            },
            33,
        ),
        (
            Kind::UnlockFile {
                offset_low: 5,
                offset_high: 7,
                length_low: 13,
                length_high: 17,
            },
            34,
        ),
    ]
}

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

pub(super) fn checked_fixture() -> psi_checked_trees::CheckedTrees {
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
    let fixture_source_id = sources
        .add_with_metadata(
            PathBuf::from("tests/native_mutation_failures.omg"),
            NATIVE_MUTATION_FIXTURE.to_owned(),
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
    let fixture_tokens = Lexer::new(NATIVE_MUTATION_FIXTURE)
        .tokenize()
        .expect("tokenize native mutation fixture");
    parse_syntax_trees_into_with_id(&mut syntax, fixture_source_id, &fixture_tokens)
        .expect("parse native mutation fixture");
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve native mutation fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type native mutation fixture");
    lower_typed_trees(typed).expect("check native mutation fixture")
}

#[test]
fn typed_records_reconstruct_each_exact_closed_variant() {
    for (kind, tag) in kinds() {
        let record = Record::new(None, kind.clone()).unwrap();
        assert!(record.source_input().is_none());
        assert_eq!(record.kind(), &kind);

        let replay = FilesystemReplay::from_input_unknown_native_handle_mutation_record(record)
            .expect("typed native mutation record is exact");
        let [attempt] = replay.attempts() else {
            panic!("record without Source retains one attempt")
        };
        assert_eq!(attempt.operation_tag(), tag);
        assert_eq!(attempt.result(), Some(FilesystemOperationResult::Scalar(0)));
        assert_eq!(attempt.post_error(), Some(6));
        assert!(attempt.logical_handle_output().is_none());
        assert!(attempt.retired_logical_handles().is_empty());
        assert!(unknown_native_handle_mutation_attempt_is_exact(attempt));
        assert_eq!(
            unknown_native_handle_mutation_from_exact_attempt(attempt),
            Some(kind)
        );
        assert!(replay.executes_replay_attempt(0));
        assert!(!replay.has_output_attempts());
        assert!(replay.expected_included_sources().is_empty());

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let observed =
            FilesystemReplay::from_input_unknown_native_handle_mutation_observations(&observations)
                .expect("exact observed native mutation is admitted");
        assert_eq!(observed.attempts(), replay.attempts());
    }
}

#[test]
fn native_mutation_records_compose_after_only_an_exact_source_prefix() {
    let source = source_input();
    let record = Record::new(Some(source.clone()), kinds()[1].0.clone()).unwrap();
    assert_eq!(record.source_input(), Some(&source));
    let replay =
        FilesystemReplay::from_input_unknown_native_handle_mutation_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 33]
    );
    assert!((0..3).all(|index| !replay.executes_replay_attempt(index)));
    assert!(replay.executes_replay_attempt(3));

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        replay.attempts().to_vec(),
        Vec::new(),
    );
    assert_eq!(
        FilesystemReplay::from_input_unknown_native_handle_mutation_observations(&observations)
            .unwrap()
            .attempts(),
        replay.attempts()
    );
}

#[test]
fn typed_records_reject_short_native_carriers() {
    assert!(
        Record::new(
            None,
            Kind::SetFileTime {
                creation: 0,
                last_access: vec![0; 7],
                last_write: vec![0; 8],
            },
        )
        .is_err()
    );
    assert!(
        Record::new(
            None,
            Kind::SetFileTime {
                creation: 0,
                last_access: vec![0; 8],
                last_write: vec![0; 7],
            },
        )
        .is_err()
    );
    assert!(
        Record::new(
            None,
            Kind::LockFileEx {
                flags: 0,
                reserved: 0,
                length_low: 0,
                length_high: 0,
                overlapped: vec![0; 31],
            },
        )
        .is_err()
    );
}

#[test]
fn observed_mutations_reject_common_and_variant_specific_drift() {
    for (kind, _) in kinds() {
        let exact = unknown_native_handle_mutation_attempt(kind);

        let mut changed = exact.clone();
        changed.provider = FilesystemObservationProvider::Virtual;
        assert_rejected(changed);

        let mut changed = exact.clone();
        changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(1),
            post_error: 6,
        });
        assert_rejected(changed);

        let mut changed = exact.clone();
        changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: 5,
        });
        assert_rejected(changed);

        let mut changed = exact.clone();
        changed.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Descriptor;
        assert_rejected(changed);

        let mut changed = exact.clone();
        changed.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
        assert_rejected(changed);

        let mut changed = exact;
        changed.byte_operands.push(FilesystemByteOperand {
            operand_ordinal: 9,
            bytes: b"invented".to_vec(),
        });
        assert_rejected(changed);
    }

    let mut set_file_time = unknown_native_handle_mutation_attempt(kinds()[0].0.clone());
    set_file_time.scalar_operands[0].value = FilesystemScalarOperandValue::U64(7);
    assert_rejected(set_file_time);
    let mut set_file_time = unknown_native_handle_mutation_attempt(kinds()[0].0.clone());
    set_file_time.byte_operands[0].operand_ordinal = 3;
    assert_rejected(set_file_time);
    let mut set_file_time = unknown_native_handle_mutation_attempt(kinds()[0].0.clone());
    set_file_time.byte_operands[1].bytes.truncate(7);
    assert_rejected(set_file_time);

    let mut lock = unknown_native_handle_mutation_attempt(kinds()[1].0.clone());
    lock.scalar_operands[2].operand_ordinal = 4;
    assert_rejected(lock);
    let mut lock = unknown_native_handle_mutation_attempt(kinds()[1].0.clone());
    lock.mutable_byte_operand_resolutions[0].operand_ordinal = 4;
    assert_rejected(lock);
    let mut lock = unknown_native_handle_mutation_attempt(kinds()[1].0.clone());
    lock.mutable_byte_operands[0].pre_bytes[0] ^= 1;
    assert_rejected(lock);
    let mut lock = unknown_native_handle_mutation_attempt(kinds()[1].0.clone());
    lock.mutable_byte_operands[0].post_bytes.truncate(31);
    assert_rejected(lock);

    let mut unlock = unknown_native_handle_mutation_attempt(kinds()[2].0.clone());
    unlock.scalar_operands[3].value = FilesystemScalarOperandValue::I32(17);
    assert_rejected(unlock);
}

#[test]
fn observed_mutations_reject_handoff_extra_operation_and_missing_operation() {
    let exact = unknown_native_handle_mutation_attempt(kinds()[2].0.clone());
    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![exact.clone()],
        vec![
            BuildIncludedSource::from_coordinate(
                FilesystemGrantRootIdentity::new(2).unwrap(),
                b"generated.omg".to_vec(),
                0,
            )
            .unwrap(),
        ],
    );
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_mutation_observations(&observations)
            .is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![
            unknown_native_handle_mutation_attempt(kinds()[0].0.clone()),
            exact,
        ],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_mutation_observations(&observations)
            .is_err()
    );

    assert!(
        FilesystemReplay::from_input_unknown_native_handle_mutation_observations(
            &EvaluationObservations::default()
        )
        .is_err()
    );
}

#[test]
fn native_mutation_records_enforce_aggregate_replay_size() {
    let oversized_set_file_time = Record::new(
        None,
        Kind::SetFileTime {
            creation: 0,
            last_access: vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES + 1],
            last_write: vec![0; 8],
        },
    )
    .unwrap();
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_mutation_record(oversized_set_file_time)
            .is_err()
    );

    let oversized_lock = Record::new(
        None,
        Kind::LockFileEx {
            flags: 0,
            reserved: 0,
            length_low: 0,
            length_high: 0,
            overlapped: vec![0; MAX_FILESYSTEM_REPLAY_RETAINED_BYTES / 3 + 1],
        },
    )
    .unwrap();
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_mutation_record(oversized_lock).is_err()
    );
}

#[test]
fn each_native_mutation_executes_replay_without_a_provider() {
    let checked = checked_fixture();
    for (kind, entry) in [
        (kinds()[0].0.clone(), "SetFileTimeMain::replay"),
        (kinds()[1].0.clone(), "LockFileExMain::replay"),
        (kinds()[2].0.clone(), "UnlockFileMain::replay"),
    ] {
        let replay = FilesystemReplay::from_input_unknown_native_handle_mutation_record(
            Record::new(None, kind).unwrap(),
        )
        .unwrap();
        let outcome = interpret_entry_with_options(
            &checked,
            entry,
            &[],
            InterpretOptions {
                filesystem: FilesystemAccess::ReplayFilesystem(replay),
                ..InterpretOptions::default()
            },
        );
        assert_eq!(outcome.error, None, "{entry}");
        assert_eq!(outcome.exit_code, 0, "{entry}");
        assert!(outcome.stdout.is_empty());
        assert!(outcome.stderr.is_empty());
    }
}

#[test]
fn virtual_and_sponsored_real_evaluators_fail_unknown_handles_before_mutation() {
    let checked = checked_fixture();
    let entries = [
        "SetFileTimeMain::probe",
        "LockFileExMain::probe",
        "UnlockFileMain::probe",
    ];
    for entry in entries {
        let outcome =
            interpret_entry_with_options(&checked, entry, &[], InterpretOptions::default());
        assert_eq!(outcome.error, None, "virtual {entry}");
        assert_eq!(outcome.exit_code, 6, "virtual {entry}");
    }

    let sponsor = FilesystemSponsor::new(std::env::temp_dir()).unwrap();
    let before = sponsor.snapshot().unwrap();
    for entry in entries {
        let outcome = interpret_entry_with_options(
            &checked,
            entry,
            &[],
            InterpretOptions {
                filesystem: FilesystemAccess::RealScopedSponsored {
                    grants: FsGrants::default(),
                    sponsor: sponsor.clone(),
                },
                ..InterpretOptions::default()
            },
        );
        assert_eq!(outcome.error, None, "sponsored real {entry}");
        assert_eq!(outcome.exit_code, 6, "sponsored real {entry}");
        assert_eq!(
            sponsor.snapshot().unwrap(),
            before,
            "sponsored real {entry}"
        );
    }
}

fn assert_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_mutation_observations(&observations)
            .is_err()
    );
}
