use super::BoundedProcessChild;
use crate::{BoundedProcessLimits, BoundedProcessPrepared};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn configured_resource_ceilings_never_loosen_inherited_limits() {
    use super::limits::intersect_limit;
    use rustix::process::Rlimit;

    assert_eq!(
        intersect_limit(
            Rlimit {
                current: Some(64),
                maximum: Some(1_024),
            },
            256,
        ),
        Rlimit {
            current: Some(64),
            maximum: Some(256),
        }
    );
    assert_eq!(
        intersect_limit(
            Rlimit {
                current: Some(64),
                maximum: Some(64),
            },
            256,
        ),
        Rlimit {
            current: Some(64),
            maximum: Some(64),
        }
    );
    assert_eq!(
        intersect_limit(
            Rlimit {
                current: None,
                maximum: None,
            },
            256,
        ),
        Rlimit {
            current: Some(256),
            maximum: Some(256),
        }
    );
}

fn limits() -> BoundedProcessLimits {
    BoundedProcessLimits::new(
        120,
        8 * 1024 * 1024 * 1024,
        1024 * 1024 * 1024,
        256,
        16,
        2 * 1024 * 1024 * 1024,
        4 * 1024 * 1024 * 1024,
    )
}

fn prepared(executable: &Path) -> BoundedProcessPrepared {
    let mut command = Command::new(executable);
    command.current_dir(
        std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary root"),
    );
    BoundedProcessPrepared::new(command, limits(), "test bounded process")
        .expect("prepare bounded process")
}

#[test]
fn preparation_rejects_inconsistent_resource_limits() {
    let executable = Path::new("/usr/bin/true")
        .canonicalize()
        .expect("canonical true executable");
    let mut invalid = limits();
    invalid.cpu_seconds = 0;
    let error =
        BoundedProcessPrepared::new(Command::new(executable), invalid, "test bounded process")
            .expect_err("zero CPU ceiling must be rejected");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn spawn_rejects_implicit_inherited_standard_streams() {
    let executable = Path::new("/usr/bin/true")
        .canonicalize()
        .expect("canonical true executable");
    let prepared = prepared(&executable);

    let error = BoundedProcessChild::spawn(prepared)
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
    let mut prepared = prepared(&executable);
    prepared.stdin_null().stdout_null().stderr_null();
    let mut child = BoundedProcessChild::spawn(prepared).expect("spawn prepared execution");
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
    let mut prepared = prepared(&executable);
    prepared.stdin_null().stdout_null().stderr_null();
    let mut child = BoundedProcessChild::spawn(prepared).expect("spawn prepared execution");
    while child.try_wait().expect("poll prepared execution").is_none() {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(child.finish().is_err());
}

#[test]
fn dropping_execution_kills_descendants_remaining_in_the_process_group() {
    let test_root =
        std::env::temp_dir().join(format!("omega-bounded-tree-cleanup-{}", std::process::id()));
    std::fs::create_dir_all(&test_root).expect("create cleanup root");
    let test_root = test_root.canonicalize().expect("canonical cleanup root");
    let marker = test_root.join("escaped-descendant");
    let shell = Path::new("/bin/sh")
        .canonicalize()
        .expect("canonical shell executable");
    let mut prepared = prepared(&shell);
    prepared
        .args([
            "-c",
            "(sleep 1; printf escaped > \"$1\") & wait",
            "omega-bounded-cleanup",
        ])
        .arg(&marker)
        .stdin_null()
        .stdout_null()
        .stderr_null();

    let child = BoundedProcessChild::spawn(prepared).expect("spawn cleanup execution");
    std::thread::sleep(Duration::from_millis(100));
    drop(child);
    std::thread::sleep(Duration::from_millis(1100));

    assert!(
        !marker.exists(),
        "descendant survived process-group cleanup"
    );
    std::fs::remove_dir_all(test_root).expect("remove cleanup root");
}
