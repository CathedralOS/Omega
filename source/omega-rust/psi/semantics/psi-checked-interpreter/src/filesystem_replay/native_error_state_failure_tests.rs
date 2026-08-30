use super::{
    FilesystemInputUnknownNativeHandleMutationReplayRecord as MutationRecord,
    FilesystemInputUnknownNativeHandleMutationWithLastErrorReplayRecord as PairRecord,
    native_error_state_failures::{
        get_last_error_after_invalid_handle_attempt,
        get_last_error_after_invalid_handle_attempt_is_exact,
    },
    native_mutation_failure_tests::{checked_fixture, kinds},
    native_mutation_failures::unknown_native_handle_mutation_attempt,
};
use crate::{
    EvaluationObservations, FilesystemAccess, FilesystemObservationProvider,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemReplay,
    InterpretOptions, interpret_entry_with_options,
};

#[test]
fn typed_pairs_reconstruct_each_exact_ordered_failure_and_error_read() {
    for (kind, mutation_tag) in kinds() {
        let mutation = MutationRecord::new(None, kind).unwrap();
        let record = PairRecord::new(mutation);
        assert!(record.mutation().source_input().is_none());
        let replay =
            FilesystemReplay::from_input_unknown_native_handle_mutation_with_last_error_record(
                record,
            )
            .unwrap();
        let [mutation, error_read] = replay.attempts() else {
            panic!("ordered record retains exactly two attempts")
        };
        assert_eq!(mutation.operation_tag(), mutation_tag);
        assert_eq!(error_read.operation_tag(), 35);
        assert_eq!(
            error_read.result(),
            Some(FilesystemOperationResult::Scalar(6))
        );
        assert_eq!(error_read.post_error(), Some(6));
        assert!(get_last_error_after_invalid_handle_attempt_is_exact(
            error_read
        ));
        assert!((0..2).all(|index| replay.executes_replay_attempt(index)));
        assert!(!replay.has_output_attempts());

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let observed = FilesystemReplay::from_input_unknown_native_handle_mutation_with_last_error_observations(
            &observations,
        )
        .unwrap();
        assert_eq!(observed.attempts(), replay.attempts());
    }
}

#[test]
fn ordered_pair_rejects_standalone_reordered_and_drifting_error_reads() {
    let mutation = unknown_native_handle_mutation_attempt(kinds()[2].0.clone());
    let error_read = get_last_error_after_invalid_handle_attempt();

    for attempts in [
        vec![error_read.clone()],
        vec![error_read.clone(), mutation.clone()],
        vec![mutation.clone(), error_read.clone(), error_read.clone()],
    ] {
        assert_rejected(attempts);
    }

    let mut changed = error_read.clone();
    changed.provider = FilesystemObservationProvider::Virtual;
    assert_rejected(vec![mutation.clone(), changed]);

    let mut changed = error_read.clone();
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(5),
        post_error: 6,
    });
    assert_rejected(vec![mutation.clone(), changed]);

    let mut changed = error_read;
    changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(6),
        post_error: 0,
    });
    assert_rejected(vec![mutation, changed]);
}

#[test]
fn each_ordered_pair_executes_without_a_provider() {
    let checked = checked_fixture();
    for (kind, entry) in [
        (kinds()[0].0.clone(), "SetFileTimeMain::probe"),
        (kinds()[1].0.clone(), "LockFileExMain::probe"),
        (kinds()[2].0.clone(), "UnlockFileMain::probe"),
    ] {
        let replay =
            FilesystemReplay::from_input_unknown_native_handle_mutation_with_last_error_record(
                PairRecord::new(MutationRecord::new(None, kind).unwrap()),
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
        assert_eq!(outcome.exit_code, 6, "{entry}");
    }
}

fn assert_rejected(attempts: Vec<crate::FilesystemOperationAttempt>) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(attempts, Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_mutation_with_last_error_observations(
            &observations,
        )
        .is_err()
    );
}
