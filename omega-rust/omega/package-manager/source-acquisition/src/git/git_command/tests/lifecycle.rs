use super::super::{GitCommandCapture, run_git_bytes_stdout, run_git_output};
use super::{GitExecutionTransport, ResolverExecutionPhase, SourceResolveError};
use super::{temp_root, test_system_git_executor};
use bounded_process::BoundedProcessInput;
use std::fs::{self, File};

#[test]
fn null_and_file_input_share_launch_and_capture_accounting() {
    let root = temp_root("git-command-inputs");
    fs::create_dir_all(&root).expect("create working directory");
    let executor = test_system_git_executor(GitExecutionTransport::File).expect("select Git");

    let empty = run_git_output(
        &executor,
        &root,
        ResolverExecutionPhase::RepositoryInspection,
        ["hash-object", "--stdin"],
        GitCommandCapture::default(),
    )
    .expect("hash null input");
    assert!(empty.status.success());
    assert_eq!(empty.stdout, b"e69de29bb2d1d6434b8b29ae775ad8c2e48c5391\n");

    let input_path = root.join("input");
    fs::write(&input_path, b"hello\n").expect("write input");
    let output = run_git_output(
        &executor,
        &root,
        ResolverExecutionPhase::RepositoryInspection,
        ["hash-object", "--stdin"],
        GitCommandCapture {
            operation: "hash supplied bytes",
            input: BoundedProcessInput::File(File::open(&input_path).expect("open input")),
            stdout_limit: 41,
        },
    )
    .expect("hash supplied input");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"ce013625030ba8dba906f756967f9e9ca394464a\n");
    assert_eq!(executor.launches.get(), 2);
    assert_eq!(executor.captured_output_budget.observed(), 82);

    fs::remove_dir_all(root).expect("remove input after child cleanup");
}

#[test]
fn operation_capture_limit_and_resolution_launch_limit_remain_distinct() {
    let root = temp_root("git-command-limits");
    fs::create_dir_all(&root).expect("create working directory");
    let mut executor = test_system_git_executor(GitExecutionTransport::File).expect("select Git");
    executor.maximum_launches = 1;
    let error = run_git_output(
        &executor,
        &root,
        ResolverExecutionPhase::RepositoryInspection,
        ["hash-object", "--stdin"],
        GitCommandCapture {
            operation: "bounded hash",
            stdout_limit: 1,
            ..GitCommandCapture::default()
        },
    )
    .expect_err("operation output exceeds its own bound");
    assert!(matches!(error, SourceResolveError::GitOutputOverflow {
        operation, stream, limit: 1,
    } if operation == "bounded hash" && stream == "stdout"));
    let error = run_git_output(
        &executor,
        &root,
        ResolverExecutionPhase::RepositoryInspection,
        ["hash-object", "--stdin"],
        GitCommandCapture::default(),
    )
    .expect_err("failed capture still consumed its launch");
    assert!(matches!(
        error,
        SourceResolveError::GitResolutionCommandLimit { limit: 1 }
    ));
    assert_eq!(executor.launches.get(), 1);
    fs::remove_dir_all(root).expect("remove working directory");
}

#[test]
fn raw_status_is_preserved_while_success_only_calls_reject() {
    let root = temp_root("git-command-status");
    fs::create_dir_all(&root).expect("create working directory");
    let executor = test_system_git_executor(GitExecutionTransport::File).expect("select Git");
    let output = run_git_output(
        &executor,
        &root,
        ResolverExecutionPhase::RepositoryInspection,
        ["--omega-invalid-command-option"],
        GitCommandCapture::default(),
    )
    .expect("a failed exit is still a captured command result");
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
    let error = run_git_bytes_stdout(
        &executor,
        &root,
        ResolverExecutionPhase::RepositoryInspection,
        ["--omega-invalid-command-option"],
    )
    .expect_err("success-only call rejects that same exit");
    assert!(matches!(error, SourceResolveError::Git {
        operation, status, stderr,
    } if operation == "command" && status == output.status.code()
        && stderr == String::from_utf8_lossy(&output.stderr)));
    fs::remove_dir_all(root).expect("remove working directory");
}
