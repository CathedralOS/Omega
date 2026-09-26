#[cfg(unix)]
use super::{
    GIT_COMMAND_CLEANUP_TIMEOUT, GitCapturedOutputBudget, command_cleanup_reserve,
    run_command_bounded, run_command_bounded_with_budget, shell_command, temp_root,
};
use super::{SourceResolveError, reconcile_git_command_result};
use crate::git::executable::executor::GitCommandDeadline;
use crate::limits::GIT_COMMAND_TIMEOUT;
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

#[cfg(unix)]
#[test]
fn bounded_command_uses_null_stdin_and_drains_both_streams() {
    let null_stdin = shell_command("if IFS= read -r value; then printf input; else printf eof; fi");
    let output = run_command_bounded(
        null_stdin,
        "test-null-stdin",
        16,
        16,
        Duration::from_secs(10),
    )
    .expect("null stdin must reach EOF without blocking");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"eof");

    let both_streams = shell_command(
        "dd if=/dev/zero bs=65536 count=2 1>&2 2>/dev/null; \
             dd if=/dev/zero bs=65536 count=2 2>/dev/null",
    );
    let output = run_command_bounded(
        both_streams,
        "test-both-streams",
        128 * 1024,
        128 * 1024,
        Duration::from_secs(10),
    )
    .expect("stdout and stderr must be drained concurrently");
    assert!(output.status.success());
    assert_eq!(output.stdout.len(), 128 * 1024);
    assert_eq!(output.stderr.len(), 128 * 1024);

    let shared_budget = GitCapturedOutputBudget::new(192 * 1024);
    let aggregate_overflow = shell_command(
        "dd if=/dev/zero bs=65536 count=2 1>&2 2>/dev/null; \
             dd if=/dev/zero bs=65536 count=2 2>/dev/null",
    );
    let error = run_command_bounded_with_budget(
        aggregate_overflow,
        "test-shared-output-budget",
        128 * 1024,
        128 * 1024,
        Duration::from_secs(10),
        shared_budget.clone(),
    )
    .expect_err("stdout and stderr must consume one shared cumulative budget");
    let exact_budget_overflow = matches!(
        &error,
        SourceResolveError::GitResolutionCapturedOutputLimit {
            ceiling,
            attempted,
        } if *ceiling == 192 * 1024 && attempted > ceiling
    );
    let fail_closed_macos_cleanup =
        cfg!(target_os = "macos") && matches!(&error, SourceResolveError::GitCleanupFailed { .. });
    assert!(
        exact_budget_overflow || fail_closed_macos_cleanup,
        "unexpected shared-output error: {error:?}"
    );
    assert!(shared_budget.observed() <= shared_budget.ceiling());
}

#[cfg(unix)]
#[test]
fn bounded_command_rejects_stdout_and_stderr_overflow() {
    for (stream, redirect) in [("stdout", ""), ("stderr", "1>&2")] {
        let script = format!(
            "i=0; while [ $i -lt 4096 ]; do printf x {redirect}; i=$((i + 1)); done; while :; do :; done"
        );
        let command = shell_command(&script);
        let error =
            run_command_bounded(command, "test-overflow", 1024, 1024, Duration::from_secs(2))
                .expect_err("capture overflow must fail closed");
        let exact_overflow = matches!(
            &error,
            SourceResolveError::GitOutputOverflow {
                stream: actual,
                limit: 1024,
                ..
            } if actual == stream
        );
        let fail_closed_macos_cleanup = cfg!(target_os = "macos")
            && matches!(&error, SourceResolveError::GitCleanupFailed { .. });
        assert!(
            exact_overflow || fail_closed_macos_cleanup,
            "unexpected overflow error: {error:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn command_deadline_reserves_cleanup_inside_the_same_budget() {
    assert_eq!(command_cleanup_reserve(Duration::ZERO), Duration::ZERO);
    assert_eq!(
        command_cleanup_reserve(Duration::from_millis(50)),
        Duration::from_micros(12_500)
    );
    assert_eq!(
        command_cleanup_reserve(Duration::from_secs(120)),
        GIT_COMMAND_CLEANUP_TIMEOUT
    );
}

#[cfg(unix)]
#[test]
fn bounded_command_terminates_on_deadline() {
    let command = shell_command("exec sleep 10");
    let started = Instant::now();
    let error = run_command_bounded(
        command,
        "test-timeout",
        1024,
        1024,
        Duration::from_millis(50),
    )
    .expect_err("deadline must fail closed");
    assert!(matches!(
        error,
        SourceResolveError::GitTimedOut {
            operation,
            timeout_millis: 50,
        } if operation == "test-timeout"
    ));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "timed out subprocess was not terminated promptly"
    );
}

#[cfg(unix)]
#[test]
fn bounded_command_terminates_descendants_on_deadline() {
    let root = temp_root("bounded-descendant-timeout");
    std::fs::create_dir_all(&root).expect("create descendant test root");
    let marker = root.join("survived");
    let mut command = shell_command(
        "(sleep 0.25; printf survived > \"$OMEGA_DESCENDANT_MARKER\") & exec sleep 10",
    );
    command.env("OMEGA_DESCENDANT_MARKER", &marker);

    let error = run_command_bounded(
        command,
        "test-descendant-timeout",
        1024,
        1024,
        Duration::from_millis(50),
    )
    .expect_err("deadline must fail closed and terminate descendants");
    assert!(matches!(error, SourceResolveError::GitTimedOut { .. }));

    std::thread::sleep(Duration::from_millis(400));
    assert!(
        !marker.exists(),
        "a descendant survived the bounded command deadline"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn bounded_command_cleans_up_descendants_after_parent_exit() {
    let command = shell_command("(sleep 10) &");
    let started = Instant::now();
    let output = run_command_bounded(
        command,
        "test-descendant-cleanup",
        1024,
        1024,
        Duration::from_secs(2),
    )
    .expect("a completed parent must not wait on descendant-held capture pipes");
    assert!(output.status.success());
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "descendant cleanup did not close inherited capture pipes promptly"
    );
}

#[test]
fn cleanup_failure_outranks_whole_resolution_expiry() {
    let result: Result<(), _> = Err(SourceResolveError::GitCleanupFailed {
        operation: "test".to_owned(),
        message: "process group may remain".to_owned(),
    });
    let budget = Err(SourceResolveError::GitResolutionTimedOut { timeout_millis: 1 });

    assert!(matches!(
        reconcile_git_command_result(result, budget),
        Err(SourceResolveError::GitCleanupFailed { .. })
    ));
}

#[test]
fn resolution_clamped_deadline_keeps_its_origin_when_cleanup_finishes_early() {
    let deadline = GitCommandDeadline::new(Duration::from_millis(29), Duration::from_millis(30));
    assert_eq!(deadline.duration(), Duration::from_millis(29));
    let timeout = SourceResolveError::GitTimedOut {
        operation: "command".to_owned(),
        timeout_millis: 29,
    };
    let result: Result<(), _> = Err(deadline.project_error(timeout));
    // Execution reserves time for cleanup. A successful early cleanup leaves
    // the absolute resolution clock unexpired, but did not change the limit
    // which caused the command to be stopped.
    assert!(matches!(
        reconcile_git_command_result(result, Ok(())),
        Err(SourceResolveError::GitResolutionTimedOut { timeout_millis: 30 })
    ));
    let cleanup = SourceResolveError::GitCleanupFailed {
        operation: "command".to_owned(),
        message: "process group may remain".to_owned(),
    };
    let result: Result<(), _> = Err(deadline.project_error(cleanup));
    assert!(matches!(
        reconcile_git_command_result(result, Ok(())),
        Err(SourceResolveError::GitCleanupFailed { .. })
    ));
    let successful: Result<(), SourceResolveError> = Ok(());
    assert!(reconcile_git_command_result(successful, Ok(())).is_ok());
}

#[test]
fn command_deadline_remains_distinct_until_the_resolution_clock_expires() {
    let deadline = GitCommandDeadline::new(
        GIT_COMMAND_TIMEOUT + Duration::from_secs(1),
        GIT_COMMAND_TIMEOUT + Duration::from_secs(2),
    );
    assert_eq!(deadline.duration(), GIT_COMMAND_TIMEOUT);
    let timeout = || SourceResolveError::GitTimedOut {
        operation: "command".to_owned(),
        timeout_millis: super::duration_millis(GIT_COMMAND_TIMEOUT),
    };
    let result: Result<(), _> = Err(deadline.project_error(timeout()));
    assert!(matches!(
        reconcile_git_command_result(result, Ok(())),
        Err(SourceResolveError::GitTimedOut { .. })
    ));
    let result: Result<(), _> = Err(deadline.project_error(timeout()));
    assert!(matches!(
        reconcile_git_command_result(
            result,
            Err(SourceResolveError::GitResolutionTimedOut {
                timeout_millis: 122000
            })
        ),
        Err(SourceResolveError::GitResolutionTimedOut {
            timeout_millis: 122000
        })
    ));
}
