#![cfg(unix)]

use super::super::GitCommandCapture;
use super::super::run_git_output;
use super::{
    GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT, GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE, GitExecutor,
    LocalSourceLimits, ResolverExecutionPhase, SOURCE_BYTE_ABSOLUTE_LIMIT, SourceResolveError,
    git_resolution_captured_output_ceiling, sealed_git_command, temp_root,
};
use std::ffi::OsStr;
use std::time::Duration;

#[cfg(unix)]
#[test]
fn git_executor_requires_launchability_but_does_not_invent_mode_trust() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("git-executable-launchability");
    std::fs::create_dir_all(&root).expect("create executable test root");
    let working_directory = root.join("working");
    std::fs::create_dir(&working_directory).expect("create package-controlled working root");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("make executable test root private");
    let fake_git = root.join("git");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");

    for host_selected_mode in [0o720, 0o4700, 0o777] {
        std::fs::set_permissions(
            &fake_git,
            std::fs::Permissions::from_mode(host_selected_mode),
        )
        .expect("set host-selected Git executable mode");
        GitExecutor::open(&fake_git)
            .expect("ownership, writable mode, and set-id state are not Git trust proofs");
    }

    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o600))
        .expect("remove executable mode bits");
    assert!(matches!(
        GitExecutor::open(&fake_git),
        Err(SourceResolveError::GitExecutableInvalid { .. })
    ));

    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("restore launchable Git mode");
    let executor = GitExecutor::open(&fake_git).expect("capture launchable Git executable");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o720))
        .expect("change irrelevant host mode bits");
    sealed_git_command(&executor, &working_directory, ResolverExecutionPhase::Fetch)
        .expect("writable mode does not become source admission policy");

    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("restore safe Git executable mode");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn git_executor_enforces_whole_resolution_launch_and_time_budgets() {
    use std::os::unix::fs::PermissionsExt;

    assert_eq!(
        git_resolution_captured_output_ceiling(LocalSourceLimits::default()),
        LocalSourceLimits::default().max_bytes + GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE
    );
    assert_eq!(
        git_resolution_captured_output_ceiling(LocalSourceLimits {
            max_bytes: SOURCE_BYTE_ABSOLUTE_LIMIT,
            ..LocalSourceLimits::default()
        }),
        GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT
    );
    let root = temp_root("git-resolution-budget");
    std::fs::create_dir_all(&root).expect("create Git budget root");
    let working_directory = root.join("working");
    std::fs::create_dir(&working_directory).expect("create package-controlled working root");
    let fast_git = root.join("fast-git");
    std::fs::write(&fast_git, b"#!/bin/sh\nexit 0\n").expect("write fast fake Git");
    std::fs::set_permissions(&fast_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fast fake Git executable");
    let launch_bounded = GitExecutor::open_with_budget(&fast_git, 1, Duration::from_secs(1))
        .expect("capture launch-bounded Git");
    run_git_output(
        &launch_bounded,
        &working_directory,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("first")],
        GitCommandCapture::default(),
    )
    .expect("first launch fits the budget");
    assert!(matches!(
        run_git_output(
            &launch_bounded,
            &working_directory,
            ResolverExecutionPhase::Fetch,
            [OsStr::new("second")],
            GitCommandCapture::default(),
        ),
        Err(SourceResolveError::GitResolutionCommandLimit { limit: 1 })
    ));

    let slow_git = root.join("slow-git");
    std::fs::write(&slow_git, b"#!/bin/sh\nsleep 1\n").expect("write slow fake Git");
    std::fs::set_permissions(&slow_git, std::fs::Permissions::from_mode(0o700))
        .expect("make slow fake Git executable");
    let time_bounded = GitExecutor::open_with_budget(&slow_git, 1, Duration::from_millis(30))
        .expect("capture time-bounded Git");
    let time_error = run_git_output(
        &time_bounded,
        &working_directory,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("slow")],
        GitCommandCapture::default(),
    )
    .expect_err("slow command must exhaust the resolution deadline");
    assert!(
        matches!(
            &time_error,
            SourceResolveError::GitResolutionTimedOut { .. }
        ) || cfg!(target_os = "macos")
            && matches!(&time_error, SourceResolveError::GitCleanupFailed { .. }),
        "unexpected deadline error: {time_error:?}"
    );

    let output_git = root.join("output-git");
    std::fs::write(
            &output_git,
            b"#!/bin/sh\nfor argument do last=$argument; done\nprintf 12345678\nif [ \"$last\" = second ]; then while :; do :; done; fi\n",
        )
        .expect("write output fake Git");
    std::fs::set_permissions(&output_git, std::fs::Permissions::from_mode(0o700))
        .expect("make output fake Git executable");
    let output_bounded =
        GitExecutor::open_with_resource_budgets(&output_git, 2, Duration::from_secs(1), 12)
            .expect("capture output-bounded Git");
    run_git_output(
        &output_bounded,
        &working_directory,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("first")],
        GitCommandCapture::default(),
    )
    .expect("first command fits cumulative output budget");
    let output_error = run_git_output(
        &output_bounded,
        &working_directory,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("second")],
        GitCommandCapture::default(),
    )
    .expect_err("second command must exhaust cumulative output budget");
    assert!(
        matches!(
            &output_error,
            SourceResolveError::GitResolutionCapturedOutputLimit {
                ceiling: 12,
                attempted,
            } if *attempted > 12
        ) || cfg!(target_os = "macos")
            && matches!(&output_error, SourceResolveError::GitCleanupFailed { .. }),
        "unexpected cumulative output error: {output_error:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}
