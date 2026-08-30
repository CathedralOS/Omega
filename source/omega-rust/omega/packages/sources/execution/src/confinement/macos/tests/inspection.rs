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
fn seatbelt_inspection_keeps_ambient_external_reads_available() {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-inspection-ambient-read-{}-{sequence}",
        std::process::id()
    ));
    let repository = parent.join("repository");
    std::fs::create_dir_all(&repository).expect("create inspection repository");
    let repository = repository
        .canonicalize()
        .expect("canonicalize inspection repository");
    let ambient_file = parent.join("ambient-config");
    std::fs::write(&ambient_file, b"ambient").expect("write ambient read canary");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let mut read_content = backend
        .command_with_inspection_read_root(Path::new("/bin/cat"), &[], &repository)
        .expect("build inspection ambient-content sandbox");
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
        .command_with_inspection_read_root(Path::new("/usr/bin/stat"), &[], &repository)
        .expect("build inspection ambient-metadata sandbox");
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
