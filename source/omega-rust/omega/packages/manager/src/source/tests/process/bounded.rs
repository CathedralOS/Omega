#[cfg(unix)]
use super::super::{
    GIT_COMMAND_CLEANUP_TIMEOUT, GitCapturedOutputBudget, StreamCaptureResult,
    capture_stream_bounded, command_cleanup_reserve, open_git_transport_executable,
    open_https_transport_executable, run_command_bounded, run_command_bounded_with_budget,
    shell_command, temp_root, verify_git_transport_executable,
};
use super::super::{
    SourceResolveError, reconcile_git_command_endpoint_result, reconcile_git_command_result,
};
#[cfg(unix)]
use std::time::{Duration, Instant};

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
    assert!(shared_budget.observed() <= shared_budget.ceiling);
}

#[cfg(unix)]
#[test]
fn bounded_command_rejects_stdout_and_stderr_overflow() {
    assert!(matches!(
        capture_stream_bounded(std::io::Cursor::new(vec![0_u8; 1025]), 1024),
        StreamCaptureResult::Overflow
    ));
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
fn ssh_transport_executable_reuses_resolver_executable_custody() {
    use std::os::unix::fs::PermissionsExt;

    let temporary_root = temp_root("ssh-transport-executable");
    std::fs::create_dir_all(&temporary_root).expect("create SSH executable custody root");
    let root = temporary_root
        .canonicalize()
        .expect("canonicalize SSH executable custody root");
    let fake_ssh = root.join("ssh");
    std::fs::write(&fake_ssh, b"#!/bin/sh\nexit 0\n").expect("write fake SSH executable");
    std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o700))
        .expect("make fake SSH executable");

    let executable =
        open_git_transport_executable(&fake_ssh).expect("capture SSH executable identity");
    assert!(executable.identity.path.is_absolute());
    assert_eq!(executable.identity.content_identity.len(), 64);
    verify_git_transport_executable(&executable).expect("verify unchanged SSH executable");

    std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o777))
        .expect("make SSH executable unsafe");
    assert!(matches!(
        verify_git_transport_executable(&executable),
        Err(SourceResolveError::GitExecutableChanged { .. })
            | Err(SourceResolveError::GitExecutableInvalid { .. })
    ));

    std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn https_transport_executable_binds_invocation_alias_and_canonical_target() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let temporary_root = temp_root("https-transport-executable");
    std::fs::create_dir_all(&temporary_root).expect("create HTTPS helper custody root");
    let root = temporary_root
        .canonicalize()
        .expect("canonicalize HTTPS helper custody root");
    let bin = root.join("bin");
    let helpers = root.join("libexec/git-core");
    std::fs::create_dir_all(&bin).expect("create fake Git bin directory");
    std::fs::create_dir_all(&helpers).expect("create fake Git helper directory");

    let fake_git = bin.join("git");
    let helper_target = helpers.join("git-remote-http");
    let helper_alias = helpers.join("git-remote-https");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");
    std::fs::write(&helper_target, b"#!/bin/sh\nexit 0\n").expect("write fake HTTPS helper target");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fake Git executable");
    std::fs::set_permissions(&helper_target, std::fs::Permissions::from_mode(0o700))
        .expect("make fake HTTPS helper target executable");
    symlink("git-remote-http", &helper_alias).expect("create HTTPS helper alias");

    let executable = open_https_transport_executable(&fake_git)
        .expect("capture HTTPS helper alias and target identity");
    assert_eq!(executable.identity.invocation_path(), helper_alias);
    assert_eq!(
        executable.identity.path(),
        helper_target
            .canonicalize()
            .expect("canonicalize HTTPS helper target")
    );
    assert_eq!(executable.identity.content_identity().len(), 64);
    verify_git_transport_executable(&executable).expect("verify unchanged HTTPS helper");

    let replacement = helpers.join("replacement");
    std::fs::write(&replacement, b"#!/bin/sh\nexit 1\n").expect("write replacement HTTPS helper");
    std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o700))
        .expect("make replacement HTTPS helper executable");
    std::fs::remove_file(&helper_alias).expect("remove original HTTPS helper alias");
    symlink("replacement", &helper_alias).expect("replace HTTPS helper alias");
    assert!(matches!(
        verify_git_transport_executable(&executable),
        Err(SourceResolveError::GitExecutableChanged { .. })
    ));

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
        reconcile_git_command_result(result, Ok(()), budget),
        Err(SourceResolveError::GitCleanupFailed { .. })
    ));
}

#[test]
fn network_transfer_ceiling_outranks_ordinary_git_failure() {
    let operation = Err(SourceResolveError::Git {
        operation: "command".to_owned(),
        status: Some(1),
        stderr: "connection closed".to_owned(),
    });
    let endpoint = Err(SourceResolveError::GitResolutionNetworkTransferCeiling { ceiling: 1024 });

    assert!(matches!(
        reconcile_git_command_endpoint_result::<()>(operation, endpoint, Ok(()), Ok(())),
        Err(SourceResolveError::GitResolutionNetworkTransferCeiling { ceiling: 1024 })
    ));
}
