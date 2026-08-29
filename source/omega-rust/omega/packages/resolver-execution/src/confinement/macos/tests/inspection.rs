use super::*;

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_inspection_allows_only_the_fixed_null_write_sink() {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "omega-resolver-inspection-write-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&root).expect("create inspection canary root");
    let root = root.canonicalize().expect("canonicalize inspection root");
    let marker = root.join("marker");
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let helper_executables = [Path::new("/bin/bash").to_path_buf()];

    let mut allowed = backend
        .command_with_inspection_read_root(Path::new("/bin/sh"), &helper_executables, &root)
        .expect("build inspection sandbox");
    let status = allowed
        .args(["-c", "printf allowed > /dev/null"])
        .status()
        .expect("write the fixed null sink");
    assert!(status.success());

    let mut denied = backend
        .command_with_inspection_read_root(Path::new("/bin/sh"), &helper_executables, &root)
        .expect("build inspection sandbox");
    let status = denied
        .args(["-c", "printf denied > \"$1\"", "resolver-test"])
        .arg(&marker)
        .status()
        .expect("attempt ordinary inspection write");
    assert!(!status.success());
    assert!(!marker.exists());
    std::fs::remove_dir(root).expect("remove inspection canary root");
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_inspection_confines_file_content_to_the_retained_repository() {
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-inspection-read-{}-{sequence}",
        std::process::id()
    ));
    let repository = parent.join("repository");
    std::fs::create_dir_all(&repository).expect("create inspection repository");
    let repository = repository
        .canonicalize()
        .expect("canonicalize inspection repository");
    let inside = repository.join("inside");
    let sibling = parent.join("sibling");
    let escaped_link = repository.join("escaped-link");
    std::fs::write(&inside, b"inside").expect("write inside canary");
    std::fs::write(&sibling, b"sibling").expect("write sibling canary");
    symlink(&sibling, &escaped_link).expect("create escaping symlink");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let mut allowed = backend
        .command_with_inspection_read_root(Path::new("/bin/cat"), &[], &repository)
        .expect("build repository-content sandbox");
    let output = allowed.arg(&inside).output().expect("read inside content");
    assert!(
        output.status.success(),
        "inside read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"inside");

    for denied_path in [&sibling, &escaped_link] {
        let mut denied = backend
            .command_with_inspection_read_root(Path::new("/bin/cat"), &[], &repository)
            .expect("build repository-content sandbox");
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
    std::fs::remove_dir(repository).expect("remove inspection repository");
    std::fs::remove_dir(parent).expect("remove inspection parent");
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_inspection_confines_metadata_to_the_retained_repository() {
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-inspection-metadata-{}-{sequence}",
        std::process::id()
    ));
    let repository = parent.join("repository");
    let raw_inside = repository.join("inside");
    let raw_sibling = parent.join("sibling");
    let raw_escaped_link = repository.join("escaped-link");
    std::fs::create_dir_all(&raw_inside).expect("create inspection repository metadata");
    std::fs::create_dir(&raw_sibling).expect("create sibling metadata canary");
    symlink(&raw_sibling, &raw_escaped_link).expect("create escaping metadata symlink");
    let repository = repository
        .canonicalize()
        .expect("canonicalize inspection repository");
    let inside = repository.join("inside");
    let sibling = raw_sibling
        .canonicalize()
        .expect("canonicalize sibling metadata canary");
    let escaped_link = repository.join("escaped-link");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let run_stat = |arguments: &[&std::ffi::OsStr]| {
        let mut command = backend
            .command_with_inspection_read_root(Path::new("/usr/bin/stat"), &[], &repository)
            .expect("build repository-metadata sandbox");
        command
            .args(arguments)
            .output()
            .expect("run metadata canary")
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

    std::fs::remove_file(escaped_link).expect("remove escaping metadata symlink");
    std::fs::remove_dir(inside).expect("remove inside metadata canary");
    std::fs::remove_dir(sibling).expect("remove sibling metadata canary");
    std::fs::remove_dir(repository).expect("remove inspection repository");
    std::fs::remove_dir(parent).expect("remove inspection parent");
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_rejects_unlisted_descendant_executables() {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let output = std::env::temp_dir().join(format!(
        "omega-resolver-exec-denied-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&output).expect("create executable-denial root");
    let output = output.canonicalize().expect("canonicalize denial root");
    let marker = output.join("marker");
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let helper_executables = [Path::new("/bin/bash").to_path_buf()];
    let route = backend
        .open_endpoint_route(
            ResolverExecutionRequestedEndpoint::new("127.0.0.1", 9)
                .expect("construct executable-denial endpoint"),
            ResolverExecutionTransferBudget::new(1024 * 1024).expect("construct transfer budget"),
        )
        .expect("open executable-denial route");
    let mut command = backend
        .command_with_endpoint_route_observation(
            Path::new("/bin/sh"),
            &helper_executables,
            ResolverExecutionPhase::Fetch,
            Some(ResolverExecutionNetworkTransport::Https),
            Some(&route),
            Some(&output),
        )
        .map(|(command, _)| command)
        .expect("build closed-executable sandbox");
    command.current_dir(&output);
    let status = command
        .args(["-c", "/usr/bin/touch \"$1\"", "resolver-test"])
        .arg(&marker)
        .status()
        .expect("attempt unlisted descendant execution");
    assert!(!status.success());
    assert!(!marker.exists());
    route.finish().expect("finish executable-denial route");
    std::fs::remove_dir(output).expect("remove executable-denial root");
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_denies_allowlisted_descendant_creation_during_inspection() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let helper_executables = [
        Path::new("/bin/bash").to_path_buf(),
        Path::new("/usr/bin/true").to_path_buf(),
    ];
    let inspection_root = inspection_root();
    let mut command = backend
        .command_with_inspection_read_root(
            Path::new("/bin/sh"),
            &helper_executables,
            &inspection_root,
        )
        .expect("build descendant-denial sandbox");
    let status = command
        .args(["-c", "/usr/bin/true & wait"])
        .status()
        .expect("attempt allowlisted descendant creation");
    assert!(!status.success());
}
