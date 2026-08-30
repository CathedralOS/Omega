#![cfg(unix)]

use super::{
    GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT, GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE, GitExecutionTransport,
    GitExecutor, LocalSourceLimits, ResolverExecutionPhase, SOURCE_BYTE_ABSOLUTE_LIMIT,
    SourceResolveError, change_macos_acl, git_resolution_captured_output_ceiling, run_git_output,
    sealed_git_command, temp_root, test_system_git_executor,
    verify_macos_open_executable_acl_custody,
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
fn git_executor_rejects_unsafe_executable_modes_and_direct_mutation() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("git-executable-custody");
    std::fs::create_dir_all(&root).expect("create executable custody root");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("make executable custody root private");
    let fake_git = root.join("git");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");

    for unsafe_mode in [0o720, 0o4700, 0o600] {
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(unsafe_mode))
            .expect("set unsafe Git executable mode");
        assert!(matches!(
            GitExecutor::open(&fake_git),
            Err(SourceResolveError::GitExecutableInvalid { .. })
        ));
    }

    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("restore safe Git executable mode");
    let executor = GitExecutor::open(&fake_git).expect("capture safe Git executable");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o720))
        .expect("make captured Git executable externally writable");
    assert!(matches!(
        executor.verify(),
        Err(SourceResolveError::GitExecutableChanged { .. })
    ));

    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("restore safe Git executable mode");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn git_executor_normal_custody_does_not_claim_executable_ancestry() {
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
        .expect("normal executable custody must not claim package-cache custody over ancestry");
    executor
        .verify()
        .expect("ancestry mode must remain outside normal executable custody");

    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("restore executable ancestry for cleanup");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(target_os = "macos")]
#[test]
fn git_executor_audits_direct_executable_acl_but_not_ancestry_acl() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("git-executable-acl-custody");
    std::fs::create_dir_all(&root).expect("create executable ACL custody root");
    let root = root
        .canonicalize()
        .expect("canonicalize executable ACL custody root");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("make executable ACL custody root private");
    let fake_git = root.join("git");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fake Git executable private");

    change_macos_acl(&root, &["+a", "everyone allow write"]);
    let executor = GitExecutor::open(&fake_git)
        .expect("an ancestry ACL must remain outside normal executable custody");
    executor
        .verify()
        .expect("an ancestry ACL must not affect repeated direct custody checks");

    change_macos_acl(&fake_git, &["+a", "everyone allow write"]);
    let executable_acl_error = executor
        .verify()
        .expect_err("extended ACL allow on executable must reject");
    assert!(
        matches!(
            &executable_acl_error,
            SourceResolveError::GitExecutableInvalid { path, message }
                if path == &fake_git && message.contains("extended ACL allow")
        ),
        "unexpected executable ACL error: {executable_acl_error:?}"
    );
    change_macos_acl(&fake_git, &["-N"]);
    executor
        .verify()
        .expect("removing executable ACL should restore custody");
    change_macos_acl(&fake_git, &["+a", "everyone deny write"]);
    executor
        .verify()
        .expect("deny-only executable ACL does not broaden custody");
    change_macos_acl(&fake_git, &["-N"]);
    change_macos_acl(&root, &["-N"]);
    executor
        .verify()
        .expect("removing an irrelevant ancestry ACL preserves direct custody");

    std::fs::remove_file(&fake_git).expect("remove fake Git executable");
    std::fs::remove_dir(&root).expect("remove executable ACL custody root");
}

#[cfg(target_os = "macos")]
#[test]
fn executable_acl_handle_open_rejects_classified_path_replacement() {
    use std::os::unix::fs::PermissionsExt;

    let temporary_root = temp_root("git-executable-acl-handle-replacement");
    std::fs::create_dir_all(&temporary_root).expect("create executable ACL test root");
    let root = temporary_root
        .canonicalize()
        .expect("canonicalize executable ACL test root");
    let executable = root.join("git");
    let retained = root.join("retained");
    std::fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("write classified executable");
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
        .expect("make classified executable private");
    let classified =
        std::fs::symlink_metadata(&executable).expect("classify executable before replacement");

    std::fs::rename(&executable, &retained).expect("relocate classified executable");
    std::fs::write(&executable, b"#!/bin/sh\nexit 1\n").expect("write replacement executable");
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
        .expect("make replacement executable private");
    change_macos_acl(&executable, &["+a", "everyone allow write"]);

    assert!(matches!(
        verify_macos_open_executable_acl_custody(&executable, &classified),
        Err(SourceResolveError::GitExecutableChanged { path }) if path == executable
    ));

    change_macos_acl(&executable, &["-N"]);
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(target_os = "macos")]
#[test]
fn system_git_executor_excludes_the_apple_dispatcher() {
    let executor = test_system_git_executor(GitExecutionTransport::Https)
        .expect("concrete macOS Git executor");
    assert_ne!(executor.identity.path, Path::new("/usr/bin/git"));
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
