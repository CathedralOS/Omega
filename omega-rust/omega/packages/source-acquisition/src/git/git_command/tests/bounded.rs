use super::{SourceResolveError, reconcile_git_command_result};
#[cfg(unix)]
use super::{run_command_bounded, shell_command};
use std::time::Duration;

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
