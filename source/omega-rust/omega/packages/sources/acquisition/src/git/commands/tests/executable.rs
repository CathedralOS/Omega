#![cfg(unix)]

use super::{
    GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT, GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE, GitExecutionTransport,
    GitExecutor, LocalSourceLimits, ResolverExecutionPhase, SOURCE_BYTE_ABSOLUTE_LIMIT,
    SourceResolveError, git_resolution_captured_output_ceiling, run_git_output, sealed_git_command,
    temp_root, test_system_git_executor,
};
use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

#[cfg(unix)]
#[test]
fn git_executor_uses_committed_absolute_program_inherited_environment_and_explicit_cwd() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("sealed-git-executor");
    let working_directory = root.join("working");
    std::fs::create_dir_all(&working_directory).expect("create explicit working directory");
    let working_directory = working_directory
        .canonicalize()
        .expect("canonical explicit working directory");
    let fake_git = root.join("git");
    std::fs::write(
            &fake_git,
            b"#!/bin/sh\nprintf 'cwd='\npwd\nprintf 'home=%s\\n' \"${HOME-unset}\"\nprintf 'path=%s\\n' \"$PATH\"\n",
        )
        .expect("write fake Git executable");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fake Git executable");
    let executor = GitExecutor::open(&fake_git).expect("capture fake Git identity");

    let output = run_git_output(
        &executor,
        &working_directory,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("ignored-by-test-helper")],
    )
    .expect("run sealed fake Git");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("test helper emits UTF-8");
    assert!(
        stdout.contains(&format!("cwd={}\n", working_directory.display())),
        "sealed helper reported {stdout:?}"
    );
    let inherited_home = std::env::var("HOME").expect("test process has a UTF-8 HOME");
    let inherited_path = std::env::var("PATH").expect("test process has a UTF-8 PATH");
    assert!(stdout.contains(&format!("home={inherited_home}\n")));
    assert!(stdout.contains(&format!("path={inherited_path}\n")));

    let command = sealed_git_command(&executor, &working_directory, ResolverExecutionPhase::Fetch)
        .expect("construct sealed fake Git command");
    assert_eq!(command.get_program(), fake_git.canonicalize().unwrap());
    assert_eq!(command.get_current_dir(), Some(working_directory.as_path()));

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn git_executor_rejects_relative_paths_and_executable_drift() {
    use std::os::unix::fs::PermissionsExt;

    assert!(matches!(
        GitExecutor::open(Path::new("git")),
        Err(SourceResolveError::GitExecutableInvalid { .. })
    ));

    let root = temp_root("git-executable-drift");
    std::fs::create_dir_all(&root).expect("create executable drift root");
    let fake_git = root.join("git");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fake Git executable");
    let executor = GitExecutor::open(&fake_git).expect("capture fake Git identity");
    let replacement = root.join("replacement");
    std::fs::write(&replacement, b"#!/bin/sh\nexit 1\n").expect("write replacement Git executable");
    std::fs::rename(&replacement, &fake_git).expect("replace fake Git executable");

    assert!(matches!(
        executor.verify(),
        Err(SourceResolveError::GitExecutableChanged { .. })
    ));
    assert!(matches!(
        sealed_git_command(&executor, &root, ResolverExecutionPhase::Fetch),
        Err(SourceResolveError::GitExecutableChanged { .. })
    ));

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn git_executor_requires_launchability_but_does_not_invent_mode_trust() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("git-executable-launchability");
    std::fs::create_dir_all(&root).expect("create executable test root");
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
    executor
        .verify()
        .expect("writable mode does not become source admission policy");

    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("restore safe Git executable mode");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn git_executor_selection_does_not_claim_executable_ancestry() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("git-executable-normal-ancestry");
    std::fs::create_dir_all(&root).expect("create executable ancestry root");
    let fake_git = root.join("git");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("make Git executable directly safe");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o720))
        .expect("make executable ancestry externally writable");

    let executor = GitExecutor::open(&fake_git)
        .expect("ordinary executable selection must not inspect ancestry permissions");
    executor
        .verify()
        .expect("ancestry mode remains host policy");

    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("restore executable ancestry for cleanup");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn system_git_executor_uses_one_absolute_path_selection() {
    let executor = test_system_git_executor(GitExecutionTransport::Https)
        .expect("Git selected from the invoking PATH snapshot");
    assert!(executor.identity.path.is_absolute());
}

#[cfg(unix)]
#[test]
fn git_executor_post_check_overrides_success_and_nonzero_exit_after_drift() {
    use std::os::unix::fs::PermissionsExt;

    for exit_status in [0, 7] {
        let root = temp_root(&format!("git-post-drift-{exit_status}"));
        std::fs::create_dir_all(&root).expect("create post-drift root");
        let fake_git = root.join("git");
        let replacement = root.join("git.replacement");
        std::fs::write(
            &fake_git,
            format!("#!/bin/sh\nmv \"$0.replacement\" \"$0\"\nexit {exit_status}\n"),
        )
        .expect("write self-replacing Git executable");
        std::fs::write(&replacement, b"#!/bin/sh\nexit 0\n")
            .expect("write replacement Git executable");
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
            .expect("make self-replacing Git executable");
        std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o700))
            .expect("make replacement Git executable");
        let executor = GitExecutor::open(&fake_git).expect("capture original Git identity");

        assert!(matches!(
            run_git_output(
                &executor,
                &root,
                ResolverExecutionPhase::Fetch,
                [OsStr::new("ignored")],
            ),
            Err(SourceResolveError::GitExecutableChanged { .. })
        ));

        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(unix)]
#[test]
fn git_executor_rehash_checkpoint_detects_content_with_restored_metadata() {
    use std::fs::{File, FileTimes};
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("git-content-checkpoint-drift");
    std::fs::create_dir_all(&root).expect("create content-checkpoint root");
    let fake_git = root.join("git");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write original Git executable");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("make original Git executable launchable");
    let original_modified = std::fs::metadata(&fake_git)
        .and_then(|metadata| metadata.modified())
        .expect("capture original modification time");
    let executor = GitExecutor::open(&fake_git).expect("capture original Git identity");

    std::fs::write(&fake_git, b"#!/bin/sh\nexit 1\n").expect("mutate Git bytes in place");
    File::options()
        .write(true)
        .open(&fake_git)
        .and_then(|file| file.set_times(FileTimes::new().set_modified(original_modified)))
        .expect("restore metadata observed around ordinary launches");

    executor
        .verify()
        .expect("launch metadata alone cannot identify restored executable bytes");
    assert!(matches!(
        executor.verify_content(),
        Err(SourceResolveError::GitExecutableChanged { .. })
    ));

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
    let fast_git = root.join("fast-git");
    std::fs::write(&fast_git, b"#!/bin/sh\nexit 0\n").expect("write fast fake Git");
    std::fs::set_permissions(&fast_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fast fake Git executable");
    let launch_bounded = GitExecutor::open_with_budget(&fast_git, 1, Duration::from_secs(1))
        .expect("capture launch-bounded Git");
    run_git_output(
        &launch_bounded,
        &root,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("first")],
    )
    .expect("first launch fits the budget");
    assert!(matches!(
        run_git_output(
            &launch_bounded,
            &root,
            ResolverExecutionPhase::Fetch,
            [OsStr::new("second")],
        ),
        Err(SourceResolveError::GitResolutionCommandLimit { limit: 1 })
    ));

    let slow_git = root.join("slow-git");
    std::fs::write(&slow_git, b"#!/bin/sh\nsleep 1\n").expect("write slow fake Git");
    std::fs::set_permissions(&slow_git, std::fs::Permissions::from_mode(0o700))
        .expect("make slow fake Git executable");
    let time_bounded = GitExecutor::open_with_budget(&slow_git, 1, Duration::from_millis(30))
        .expect("capture time-bounded Git");
    assert!(matches!(
        run_git_output(
            &time_bounded,
            &root,
            ResolverExecutionPhase::Fetch,
            [OsStr::new("slow")],
        ),
        Err(SourceResolveError::GitResolutionTimedOut { .. })
    ));

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
        &root,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("first")],
    )
    .expect("first command fits cumulative output budget");
    let output_error = run_git_output(
        &output_bounded,
        &root,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("second")],
    )
    .expect_err("second command must exhaust cumulative output budget");
    assert!(
        matches!(
        &output_error,
        SourceResolveError::GitResolutionCapturedOutputLimit {
            ceiling: 12,
            attempted,
        } if *attempted > 12
        ),
        "unexpected cumulative output error: {output_error:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}
