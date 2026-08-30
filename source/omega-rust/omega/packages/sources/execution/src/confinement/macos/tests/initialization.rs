use super::*;

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_denies_an_ordinary_write_outside_the_mutable_root() {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "omega-resolver-execution-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&root).expect("create sandbox root");
    let root = root.canonicalize().expect("canonicalize sandbox root");
    let inside = root.join("inside");
    let outside = root.with_extension("outside");
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let helper_executables = [Path::new("/bin/bash").to_path_buf()];
    let mut allowed = backend
        .command(
            Path::new("/bin/sh"),
            &helper_executables,
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&root),
        )
        .expect("build writable sandbox");
    allowed.current_dir(&root);
    let status = allowed
        .args(["-c", "printf allowed > \"$1\"", "resolver-test"])
        .arg(&inside)
        .status()
        .expect("run allowed write");
    assert!(status.success());

    let mut denied = backend
        .command(
            Path::new("/bin/sh"),
            &helper_executables,
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&root),
        )
        .expect("build confined sandbox");
    denied.current_dir(&root);
    let status = denied
        .args(["-c", "printf denied > \"$1\"", "resolver-test"])
        .arg(&outside)
        .status()
        .expect("run denied write");
    assert!(!status.success());
    assert!(!outside.exists());
    std::fs::remove_file(inside).expect("remove sandbox output");
    std::fs::remove_dir(root).expect("remove sandbox root");
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_initialization_keeps_ambient_external_reads_available() {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-initialization-ambient-read-{}-{sequence}",
        std::process::id()
    ));
    let mutable_root = parent.join("mutable");
    std::fs::create_dir_all(&mutable_root).expect("create initialization mutable root");
    let mutable_root = mutable_root
        .canonicalize()
        .expect("canonicalize initialization mutable root");
    let ambient_file = parent.join("ambient-config");
    std::fs::write(&ambient_file, b"ambient").expect("write ambient read canary");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let mut read_content = backend
        .command(
            Path::new("/bin/cat"),
            &[],
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&mutable_root),
        )
        .expect("build initialization ambient-content sandbox");
    read_content.current_dir(&mutable_root);
    let output = read_content
        .arg(&ambient_file)
        .output()
        .expect("read ambient file content");
    assert!(
        output.status.success(),
        "ambient content read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ambient");

    let mut read_metadata = backend
        .command(
            Path::new("/usr/bin/stat"),
            &[],
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&mutable_root),
        )
        .expect("build initialization ambient-metadata sandbox");
    read_metadata.current_dir(&mutable_root);
    let output = read_metadata
        .args([std::ffi::OsStr::new("-f"), std::ffi::OsStr::new("%N")])
        .arg(&ambient_file)
        .output()
        .expect("read ambient file metadata");
    assert!(
        output.status.success(),
        "ambient metadata read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::remove_file(ambient_file).expect("remove ambient read canary");
    std::fs::remove_dir(mutable_root).expect("remove initialization mutable root");
    std::fs::remove_dir(parent).expect("remove initialization parent");
}
