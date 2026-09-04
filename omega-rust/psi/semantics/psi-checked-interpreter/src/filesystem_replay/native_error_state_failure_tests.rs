use super::{
    FilesystemInputUnknownNativeHandleCloseHandleReplayRecord as CloseRecord,
    FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord as FinalPathRecord,
    FilesystemInputUnknownNativeHandleMutationReplayRecord as MutationRecord,
    FilesystemInputUnknownNativeHandleMutationWithLastErrorReplayRecord as PairRecord,
    native_error_state_failures::{
        get_last_error_after_invalid_handle_attempt,
        get_last_error_after_invalid_handle_attempt_is_exact,
    },
    native_mutation_failure_tests::{checked_fixture, kinds},
    native_mutation_failures::unknown_native_handle_mutation_attempt,
    unknown_native_handle_close_handle_attempt,
    unknown_native_handle_final_path_name_by_handle_attempt,
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

#[test]
fn every_exact_unknown_native_handle_failure_accepts_immediate_last_error() {
    let mut cases = vec![
        (
            29,
            FilesystemReplay::from_input_unknown_native_handle_close_handle_record(
                CloseRecord::new(None),
            )
            .unwrap(),
        ),
        (
            31,
            FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_record(
                FinalPathRecord::new(None, vec![0; 4], 4, 0).unwrap(),
            )
            .unwrap(),
        ),
    ];
    cases.extend(kinds().into_iter().map(|(kind, tag)| {
        (
            tag,
            FilesystemReplay::from_input_unknown_native_handle_mutation_record(
                MutationRecord::new(None, kind).unwrap(),
            )
            .unwrap(),
        )
    }));

    for (failure_tag, failure) in cases {
        let replay = failure
            .with_immediate_last_error_after_unknown_native_handle_failure()
            .expect("exact invalid-handle failure accepts its immediate error-state read");
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(crate::FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![failure_tag, 35]
        );
        assert!((0..2).all(|index| replay.executes_replay_attempt(index)));
        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let observed = FilesystemReplay::from_input_unknown_native_handle_failure_with_last_error_observations(
            &observations,
        )
        .expect("generic observed invalid-handle pair is admitted");
        assert_eq!(observed.attempts(), replay.attempts());
    }
}

#[test]
fn close_and_final_path_error_pairs_execute_without_a_provider() {
    for (entry, replay) in [
        (
            "CloseHandleMain::probe",
            FilesystemReplay::from_input_unknown_native_handle_close_handle_record(
                CloseRecord::new(None),
            )
            .unwrap(),
        ),
        (
            "FinalPathMain::probe",
            FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_record(
                FinalPathRecord::new(None, vec![0; 4], 4, 0).unwrap(),
            )
            .unwrap(),
        ),
    ] {
        let replay = replay
            .with_immediate_last_error_after_unknown_native_handle_failure()
            .unwrap();
        let outcome = interpret_entry_with_options(
            &checked_fixture(),
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

#[test]
fn generic_native_error_pair_rejects_drift_and_non_native_failure() {
    let error_read = get_last_error_after_invalid_handle_attempt();
    let mut changed_close = unknown_native_handle_close_handle_attempt();
    changed_close.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: 5,
    });
    assert_rejected_generic(vec![changed_close, error_read.clone()]);

    let final_path = unknown_native_handle_final_path_name_by_handle_attempt(vec![0; 4], 4, 0);
    assert_rejected_generic(vec![error_read.clone(), final_path]);
    assert_rejected_generic(vec![error_read]);
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

fn assert_rejected_generic(attempts: Vec<crate::FilesystemOperationAttempt>) {
    let observations =
        EvaluationObservations::from_filesystem_operation_attempts(attempts, Vec::new());
    assert!(
        FilesystemReplay::from_input_unknown_native_handle_failure_with_last_error_observations(
            &observations,
        )
        .is_err()
    );
}
