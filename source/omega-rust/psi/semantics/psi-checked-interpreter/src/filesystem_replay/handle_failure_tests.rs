use super::{
    FilesystemInputUnknownDescriptorCloseReplayRecord, unknown_descriptor_close_attempt,
    unknown_descriptor_close_attempt_is_exact,
};
use crate::{
    EvaluationObservations, FilesystemLogicalHandleIdentity,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource,
    FilesystemObservationProvider, FilesystemOperationAttemptOutcome, FilesystemOperationResult,
    FilesystemReplay, FilesystemReplayReadKind, FilesystemReplayReadRecord,
    FilesystemScalarOperand, FilesystemScalarOperandValue, FilesystemSourceInputReplayEventRecord,
    FilesystemSourceInputReplayRecord, FilesystemSourceReadChainReplayRecord,
};

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

#[test]
fn unknown_descriptor_close_record_composes_after_optional_source_input() {
    let without_source = FilesystemReplay::from_input_unknown_descriptor_close_record(
        FilesystemInputUnknownDescriptorCloseReplayRecord::new(None),
    )
    .unwrap();
    assert_eq!(without_source.attempts().len(), 1);
    assert!(without_source.executes_replay_attempt(0));
    assert!(!without_source.has_output_attempts());
    assert!(unknown_descriptor_close_attempt_is_exact(
        &without_source.attempts()[0]
    ));

    let with_source = FilesystemReplay::from_input_unknown_descriptor_close_record(
        FilesystemInputUnknownDescriptorCloseReplayRecord::new(Some(source_input())),
    )
    .unwrap();
    assert_eq!(
        with_source
            .attempts()
            .iter()
            .map(crate::FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 8]
    );
    assert!(unknown_descriptor_close_attempt_is_exact(
        with_source.attempts().last().unwrap()
    ));
    assert!((0..3).all(|index| !with_source.executes_replay_attempt(index)));
    assert!(with_source.executes_replay_attempt(3));
}

#[test]
fn unknown_descriptor_close_observations_accept_only_the_closed_shape() {
    let exact = unknown_descriptor_close_attempt();
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![exact.clone()], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_close_observations(&observations).is_ok()
    );

    let mut changed = exact.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_tampered_close_rejected(changed);

    let mut changed = exact.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(-1),
        post_error: 13,
    });
    assert_tampered_close_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Native;
    assert_tampered_close_rejected(changed);

    let mut changed = exact.clone();
    changed.logical_handle_inputs[0].resolution = FilesystemLogicalHandleInputResolution::Null;
    assert_tampered_close_rejected(changed);

    let mut changed = exact.clone();
    changed.scalar_operands.push(FilesystemScalarOperand {
        operand_ordinal: 0,
        value: FilesystemScalarOperandValue::I32(-1),
    });
    assert_tampered_close_rejected(changed);

    let identity = FilesystemLogicalHandleIdentity::new(9).unwrap();
    let mut changed = exact.clone();
    changed.logical_handle_output = Some(FilesystemLogicalHandleOutput {
        kind: FilesystemLogicalHandleKind::Descriptor,
        identity,
        source: FilesystemLogicalHandleOutputSource::Created,
    });
    assert_tampered_close_rejected(changed);

    let mut changed = exact;
    changed.retired_logical_handles.push(identity);
    assert_tampered_close_rejected(changed);
}

fn assert_tampered_close_rejected(attempt: crate::FilesystemOperationAttempt) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(vec![attempt], Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_close_observations(&observations).is_err()
    );
}
