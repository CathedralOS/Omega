use super::{
    FilesystemInputUnknownDescriptorOperationReplayRecord as OperationRecord,
    FilesystemInputUnknownDescriptorOperationWithErrnoReplayRecord as PairRecord,
    descriptor_error_state_failures::{
        errno_after_bad_descriptor_attempt, errno_after_bad_descriptor_attempt_is_exact,
    },
    handle_failure_tests::{KINDS_AND_TAGS, source_input},
    handle_failures::unknown_descriptor_operation_attempt,
};
use crate::{
    EvaluationObservations, FilesystemObservationProvider, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemReplay,
};

#[test]
fn typed_records_reconstruct_each_exact_descriptor_failure_and_errno_pair() {
    for (kind, operation_tag) in KINDS_AND_TAGS {
        let record = PairRecord::new(OperationRecord::new(None, kind));
        assert!(record.operation().source_input().is_none());
        let replay =
            FilesystemReplay::from_input_unknown_descriptor_operation_with_errno_record(record)
                .unwrap();
        let [operation, errno] = replay.attempts() else {
            panic!("ordered record retains exactly two attempts")
        };
        assert_eq!(operation.operation_tag(), operation_tag);
        assert_eq!(errno.operation_tag(), 50);
        assert_eq!(errno.result(), Some(FilesystemOperationResult::Scalar(9)));
        assert_eq!(errno.post_error(), Some(9));
        assert!(errno_after_bad_descriptor_attempt_is_exact(errno));
        assert!((0..2).all(|index| replay.executes_replay_attempt(index)));
        assert!(!replay.has_output_attempts());

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        assert_eq!(
            FilesystemReplay::from_input_unknown_descriptor_operation_with_errno_observations(
                &observations,
            )
            .unwrap()
            .attempts(),
            replay.attempts()
        );
    }
}

#[test]
fn ordered_errno_pair_accepts_only_an_exact_source_prefix() {
    let record = PairRecord::new(OperationRecord::new(
        Some(source_input()),
        KINDS_AND_TAGS[1].0,
    ));
    let replay =
        FilesystemReplay::from_input_unknown_descriptor_operation_with_errno_record(record)
            .unwrap();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(crate::FilesystemOperationAttempt::operation_tag)
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 43, 50]
    );
    assert!((0..3).all(|index| !replay.executes_replay_attempt(index)));
    assert!((3..5).all(|index| replay.executes_replay_attempt(index)));
}

#[test]
fn ordered_errno_pair_rejects_standalone_reordered_and_drifting_reads() {
    let operation = unknown_descriptor_operation_attempt(KINDS_AND_TAGS[3].0);
    let errno = errno_after_bad_descriptor_attempt();

    for attempts in [
        vec![errno.clone()],
        vec![errno.clone(), operation.clone()],
        vec![operation.clone(), errno.clone(), errno.clone()],
    ] {
        assert_rejected(attempts);
    }

    let mut changed = errno.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_rejected(vec![operation.clone(), changed]);

    let mut changed = errno.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(6),
        post_error: 9,
    });
    assert_rejected(vec![operation.clone(), changed]);

    let mut changed = errno;
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(9),
        post_error: 0,
    });
    assert_rejected(vec![operation, changed]);
}

fn assert_rejected(attempts: Vec<crate::FilesystemOperationAttempt>) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(attempts, Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_descriptor_operation_with_errno_observations(
            &observations,
        )
        .is_err()
    );
}
