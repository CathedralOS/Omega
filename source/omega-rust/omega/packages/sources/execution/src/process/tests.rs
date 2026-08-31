use super::ResolverExecutionChild;
use crate::ResolverExecutionBackend;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn backend(executable: &Path) -> ResolverExecutionBackend {
    ResolverExecutionBackend::open(executable, &[] as &[PathBuf]).expect("open resolver backend")
}

fn inspection_root() -> PathBuf {
    std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary root")
}

#[test]
fn spawn_rejects_implicit_inherited_standard_streams() {
    let executable = Path::new("/usr/bin/true")
        .canonicalize()
        .expect("canonical true executable");
    let prepared = backend(&executable)
        .prepare_inspection(&inspection_root())
        .expect("prepare inspection execution");

    let error = ResolverExecutionChild::spawn(prepared)
        .err()
        .expect("implicit standard streams must be rejected");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("explicitly null or piped"));
}

#[test]
fn completion_requires_process_container_closure_and_reaping() {
    let executable = Path::new("/usr/bin/true")
        .canonicalize()
        .expect("canonical true executable");
    let mut prepared = backend(&executable)
        .prepare_inspection(&inspection_root())
        .expect("prepare inspection execution");
    prepared.stdin_null().stdout_null().stderr_null();
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
    child.terminate().expect("close process container");
    child.try_wait().expect("confirm reaped execution");
    let completion = child.finish().expect("finish execution");
    assert!(completion.status().success());
}

#[test]
fn unfinished_execution_cannot_finish() {
    let executable = Path::new("/usr/bin/true")
        .canonicalize()
        .expect("canonical true executable");
    let mut prepared = backend(&executable)
        .prepare_inspection(&inspection_root())
        .expect("prepare inspection execution");
    prepared.stdin_null().stdout_null().stderr_null();
    let mut child = ResolverExecutionChild::spawn(prepared).expect("spawn prepared execution");
    while child.try_wait().expect("poll prepared execution").is_none() {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(child.finish().is_err());
}

#[test]
fn dropping_execution_kills_descendants_remaining_in_the_process_group() {
    let test_root = std::env::temp_dir().join(format!(
        "omega-resolver-tree-cleanup-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&test_root).expect("create cleanup root");
    let test_root = test_root.canonicalize().expect("canonical cleanup root");
    let marker = test_root.join("escaped-descendant");
    let shell = Path::new("/bin/sh")
        .canonicalize()
        .expect("canonical shell executable");
    let mut prepared = backend(&shell)
        .prepare_inspection(&test_root)
        .expect("prepare cleanup execution");
    prepared
        .args([
            "-c",
            "(sleep 1; printf escaped > \"$1\") & wait",
            "omega-resolver-cleanup",
        ])
        .arg(&marker)
        .stdin_null()
        .stdout_null()
        .stderr_null();

    let child = ResolverExecutionChild::spawn(prepared).expect("spawn cleanup execution");
    std::thread::sleep(Duration::from_millis(100));
    drop(child);
    std::thread::sleep(Duration::from_millis(1100));

    assert!(
        !marker.exists(),
        "descendant survived process-group cleanup"
    );
    std::fs::remove_dir_all(test_root).expect("remove cleanup root");
}
