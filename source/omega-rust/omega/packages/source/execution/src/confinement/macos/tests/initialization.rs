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
fn seatbelt_initialization_confines_file_content_to_the_mutable_root() {
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-initialization-read-{}-{sequence}",
        std::process::id()
    ));
    let mutable_root = parent.join("mutable");
    std::fs::create_dir_all(&mutable_root).expect("create initialization mutable root");
    let mutable_root = mutable_root
        .canonicalize()
        .expect("canonicalize initialization mutable root");
    let inside = mutable_root.join("inside");
    let sibling = parent.join("sibling");
    let escaped_link = mutable_root.join("escaped-link");
    std::fs::write(&inside, b"inside").expect("write inside canary");
    std::fs::write(&sibling, b"sibling").expect("write sibling canary");
    symlink(&sibling, &escaped_link).expect("create escaping symlink");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let mut allowed = backend
        .command(
            Path::new("/bin/cat"),
            &[],
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&mutable_root),
        )
        .expect("build initialization-content sandbox");
    allowed.current_dir(&mutable_root);
    let output = allowed.arg(&inside).output().expect("read inside content");
    assert!(
        output.status.success(),
        "inside read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"inside");

    for denied_path in [&sibling, &escaped_link] {
        let mut denied = backend
            .command(
                Path::new("/bin/cat"),
                &[],
                ResolverExecutionPhase::RepositoryInitialization,
                Some(&mutable_root),
            )
            .expect("build initialization-content sandbox");
        denied.current_dir(&mutable_root);
        let output = denied
            .arg(denied_path)
            .output()
            .expect("attempt escaped content read");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }

    std::fs::remove_file(escaped_link).expect("remove escaping symlink");
    std::fs::remove_file(inside).expect("remove inside canary");
    std::fs::remove_file(sibling).expect("remove sibling canary");
    std::fs::remove_dir(mutable_root).expect("remove initialization mutable root");
    std::fs::remove_dir(parent).expect("remove initialization parent");
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_initialization_confines_metadata_to_the_mutable_root() {
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-initialization-metadata-{}-{sequence}",
        std::process::id()
    ));
    let raw_mutable_root = parent.join("mutable");
    let raw_inside = raw_mutable_root.join("inside");
    let raw_sibling = parent.join("sibling");
    let raw_escaped_link = raw_mutable_root.join("escaped-link");
    std::fs::create_dir_all(&raw_inside).expect("create initialization metadata root");
    std::fs::create_dir(&raw_sibling).expect("create initialization metadata sibling");
    symlink(&raw_sibling, &raw_escaped_link)
        .expect("create initialization escaping metadata symlink");
    let mutable_root = raw_mutable_root
        .canonicalize()
        .expect("canonicalize initialization metadata root");
    let inside = mutable_root.join("inside");
    let sibling = raw_sibling
        .canonicalize()
        .expect("canonicalize initialization metadata sibling");
    let escaped_link = mutable_root.join("escaped-link");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let run_stat = |arguments: &[&std::ffi::OsStr]| {
        let mut command = backend
            .command(
                Path::new("/usr/bin/stat"),
                &[],
                ResolverExecutionPhase::RepositoryInitialization,
                Some(&mutable_root),
            )
            .expect("build initialization-metadata sandbox");
        command.current_dir(&mutable_root);
        command
            .args(arguments)
            .output()
            .expect("run initialization metadata canary")
    };

    let inside_output = run_stat(&[
        std::ffi::OsStr::new("-f"),
        std::ffi::OsStr::new("%N"),
        inside.as_os_str(),
    ]);
    assert!(
        inside_output.status.success(),
        "inside metadata failed: {}",
        String::from_utf8_lossy(&inside_output.stderr)
    );
    let link_output = run_stat(&[
        std::ffi::OsStr::new("-f"),
        std::ffi::OsStr::new("%N"),
        escaped_link.as_os_str(),
    ]);
    assert!(
        link_output.status.success(),
        "reading the in-root symlink entry must remain allowed"
    );
    let sibling_output = run_stat(&[
        std::ffi::OsStr::new("-f"),
        std::ffi::OsStr::new("%N"),
        sibling.as_os_str(),
    ]);
    assert!(!sibling_output.status.success());
    let escaped_output = run_stat(&[std::ffi::OsStr::new("-L"), escaped_link.as_os_str()]);
    assert!(!escaped_output.status.success());

    std::fs::remove_file(escaped_link).expect("remove initialization metadata symlink");
    std::fs::remove_dir(inside).expect("remove initialization inside metadata canary");
    std::fs::remove_dir(sibling).expect("remove initialization sibling metadata canary");
    std::fs::remove_dir(mutable_root).expect("remove initialization metadata root");
    std::fs::remove_dir(parent).expect("remove initialization metadata parent");
}
