use super::ResolverExecutionChild;
use crate::{ResolverExecutionBackend, ResolverExecutionPhase};
use std::path::Path;
use std::time::{Duration, Instant};

#[test]
fn spawn_rejects_implicit_inherited_standard_streams() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let inspection_root = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary root");
    let mut prepared = backend
        .prepare_inspection(Path::new("/usr/bin/true"), &inspection_root)
        .expect("prepare inspection execution");
    prepared.env_clear().current_dir(&inspection_root);

    let error = ResolverExecutionChild::spawn(prepared)
        .err()
        .expect("implicit standard streams must be rejected");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("explicitly null or piped"));
}

#[test]
fn command_identity_binds_closed_standard_stream_dispositions() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let inspection_root = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary root");
    let prepare = |piped_stdout: bool| {
        let mut prepared = backend
            .prepare_inspection(Path::new("/usr/bin/true"), &inspection_root)
            .expect("prepare inspection execution");
        prepared
            .env_clear()
            .current_dir(&inspection_root)
            .stdin_null()
            .stderr_null();
        if piped_stdout {
            prepared.stdout_piped();
        } else {
            prepared.stdout_null();
        }
        prepared
            .command_identity()
            .expect("identify closed standard streams")
    };

    assert_ne!(prepare(false), prepare(true));
}

#[test]
fn completion_binds_prepared_command_policy_termination_and_reaping() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let inspection_root = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary root");
    let mut prepared = backend
        .prepare_inspection(Path::new("/usr/bin/true"), &inspection_root)
        .expect("prepare inspection execution");
    prepared
        .env_clear()
        .current_dir(&inspection_root)
        .stdin_null()
        .stdout_null()
        .stderr_null();
    let command = prepared
        .command_identity()
        .expect("identify prepared command");
    let mut child = ResolverExecutionChild::spawn(prepared).expect("spawn prepared execution");
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll prepared execution") {
            break status;
        }
        assert!(Instant::now() < deadline, "prepared execution timed out");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(status.success());
    child.terminate().expect("close native process container");
    child.try_wait().expect("confirm reaped execution");
    let completion = child.finish().expect("issue completion observation");

    assert_eq!(completion.command(), command);
    assert_eq!(
        completion.policy().phase(),
        ResolverExecutionPhase::RepositoryInspection
    );
    assert!(completion.status().success());
    assert!(!completion.canonical_bytes().is_empty());
}

#[test]
fn unfinished_execution_cannot_issue_completion() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let inspection_root = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary root");
    let mut prepared = backend
        .prepare_inspection(Path::new("/usr/bin/true"), &inspection_root)
        .expect("prepare inspection execution");
    prepared
        .env_clear()
        .current_dir(&inspection_root)
        .stdin_null()
        .stdout_null()
        .stderr_null();
    let mut child = ResolverExecutionChild::spawn(prepared).expect("spawn prepared execution");
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().expect("poll prepared execution").is_none() {
        assert!(Instant::now() < deadline, "prepared execution timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(child.finish().is_err());
}
