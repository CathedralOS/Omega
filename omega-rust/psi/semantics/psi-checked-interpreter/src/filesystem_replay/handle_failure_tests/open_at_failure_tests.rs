use super::*;
use crate::filesystem_replay::{
    FilesystemInputUnknownDescriptorOpenAtReplayRecord as OpenAtRecord,
    unknown_descriptor_open_at_attempt, unknown_descriptor_open_at_attempt_is_exact,
    unknown_descriptor_open_at_from_exact_attempt,
};

fn checked_unknown_descriptor_open_at() -> psi_checked_trees::CheckedTrees {
    const SOURCE: &str = r#"
data Main { filesystem: FilesystemHost; result: i32; }

machine Main::open_unknown(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.open_at(-1, "entry.bin", 0);
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
            PathBuf::from("tests/unknown_descriptor_open_at.omg"),
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
        .expect("tokenize open_at replay fixture");
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
        .expect("parse open_at replay fixture");
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve open_at replay fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type open_at replay fixture");
    lower_typed_trees(typed).expect("check open_at replay fixture")
}

#[test]
fn record_retains_exact_component_flags_and_optional_source() {
    let source = source_input();
    let record = OpenAtRecord::new(Some(source.clone()), b"entry.bin".to_vec(), i32::MIN)
        .expect("safe relative component is admitted");
    assert_eq!(record.source_input(), Some(&source));
    assert_eq!(record.relative_component(), b"entry.bin");
    assert_eq!(record.flags(), i32::MIN);

    let replay = FilesystemReplay::from_input_unknown_descriptor_open_at_record(record).unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 14]
    );
    assert!((0..3).all(|index| !replay.executes_replay_attempt(index)));
    assert!(replay.executes_replay_attempt(3));
    assert!(!replay.has_output_attempts());
    assert!(replay.expected_included_sources().is_empty());
    let attempt = replay.attempts().last().unwrap();
    assert!(unknown_descriptor_open_at_attempt_is_exact(attempt));
    assert_eq!(
        unknown_descriptor_open_at_from_exact_attempt(attempt),
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
        FilesystemReplay::from_input_unknown_descriptor_open_at_observations(&observations)
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
            OpenAtRecord::new(None, rejected.to_vec(), 0).is_err(),
            "unexpected accepted relative component: {rejected:?}"
        );
    }
}

#[test]
fn observations_reject_shape_and_input_drift() {
    let exact = unknown_descriptor_open_at_attempt(b"entry.bin".to_vec(), -47);
    let identity = FilesystemLogicalHandleIdentity::new(9).unwrap();

    let mut changed = exact.clone();
    changed.operation_tag = 15;
    assert_tampered_open_at_rejected(changed);

    let mut changed = exact.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_tampered_open_at_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: 9,
    });
    assert_tampered_open_at_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 13,
    });
    assert_tampered_open_at_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[0].operand_ordinal = 1;
    assert_tampered_open_at_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands[0].value = FilesystemScalarOperandValue::U32(47);
    assert_tampered_open_at_rejected(changed);

    let mut changed = exact.clone();
    changed.byte_operands[0].operand_ordinal = 0;
    assert_tampered_open_at_rejected(changed);

    let mut changed = exact.clone();
    changed.byte_operands[0].bytes = b"nested/name".to_vec();
    assert_tampered_open_at_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Native;
    assert_tampered_open_at_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
    assert_tampered_open_at_rejected(changed);

    let mut changed = exact;
    changed.logical_handle_inputs[0].resolution =
        FilesystemLogicalHandleInputResolution::Resolved(identity);
    assert_tampered_open_at_rejected(changed);
}

#[test]
fn observations_reject_every_forbidden_side_lane() {
    for changed in
        nonempty_side_lane_attempts(unknown_descriptor_open_at_attempt(b"entry.bin".to_vec(), 0))
    {
        assert_tampered_open_at_rejected(changed);
    }
}

#[test]
fn rejects_handoff_and_extra_operation() {
    let exact = unknown_descriptor_open_at_attempt(b"entry.bin".to_vec(), 0);
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
        FilesystemReplay::from_input_unknown_descriptor_open_at_observations(&observations)
            .is_err()
    );

    let observations = EvaluationObservations::from_filesystem_operation_attempts(
        vec![unknown_descriptor_seek_attempt(0, 0), exact],
        Vec::new(),
    );
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_open_at_observations(&observations)
            .is_err()
    );
}

#[test]
fn executes_exact_replay_provider_free() {
    let replay = FilesystemReplay::from_input_unknown_descriptor_open_at_record(
        OpenAtRecord::new(None, b"entry.bin".to_vec(), 0).unwrap(),
    )
    .unwrap();
    let checked = checked_unknown_descriptor_open_at();
    let outcome = interpret_entry_with_options(
        &checked,
        "Main::open_unknown",
        &[],
        InterpretOptions {
            filesystem: FilesystemAccess::ReplayFilesystem(replay),
            ..InterpretOptions::default()
        },
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 0);

    for record in [
        OpenAtRecord::new(None, b"changed.bin".to_vec(), 0).unwrap(),
        OpenAtRecord::new(None, b"entry.bin".to_vec(), 1).unwrap(),
    ] {
        let replay =
            FilesystemReplay::from_input_unknown_descriptor_open_at_record(record).unwrap();
        let changed = interpret_entry_with_options(
            &checked,
            "Main::open_unknown",
            &[],
            InterpretOptions {
                filesystem: FilesystemAccess::ReplayFilesystem(replay),
                ..InterpretOptions::default()
            },
        );
        assert!(changed.error.is_some());
    }
}

fn assert_tampered_open_at_rejected(attempt: FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_open_at_observations(&observations)
            .is_err()
    );
}
