use super::*;

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_https_fetch_keeps_ambient_external_reads_available() {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-https-fetch-ambient-read-{}-{sequence}",
        std::process::id()
    ));
    let mutable_root = parent.join("mutable");
    std::fs::create_dir_all(&mutable_root).expect("create HTTPS fetch mutable root");
    let mutable_root = mutable_root
        .canonicalize()
        .expect("canonicalize HTTPS fetch mutable root");
    let ambient_file = parent.join("ambient-config");
    std::fs::write(&ambient_file, b"ambient").expect("write ambient read canary");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let route = loopback_route(&backend);
    let build_command = |executable: &Path| {
        let (mut command, _observation) = backend
            .command_with_endpoint_route_observation(
                executable,
                &[],
                ResolverExecutionPhase::Fetch,
                Some(ResolverExecutionNetworkTransport::Https),
                Some(&route),
                Some(&mutable_root),
            )
            .expect("build HTTPS fetch ambient-read sandbox");
        command.current_dir(&mutable_root);
        command
    };

    let output = build_command(Path::new("/bin/cat"))
        .arg(&ambient_file)
        .output()
        .expect("read ambient file content during HTTPS fetch");
    assert!(
        output.status.success(),
        "ambient content read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ambient");

    let output = build_command(Path::new("/usr/bin/stat"))
        .args([std::ffi::OsStr::new("-f"), std::ffi::OsStr::new("%N")])
        .arg(&ambient_file)
        .output()
        .expect("read ambient file metadata during HTTPS fetch");
    assert!(
        output.status.success(),
        "ambient metadata read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    route.finish().expect("finish HTTPS fetch route");

    std::fs::remove_file(ambient_file).expect("remove ambient read canary");
    std::fs::remove_dir(mutable_root).expect("remove HTTPS fetch mutable root");
    std::fs::remove_dir(parent).expect("remove HTTPS fetch parent");
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_https_discovery_keeps_ambient_external_reads_available() {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-https-discovery-ambient-read-{}-{sequence}",
        std::process::id()
    ));
    let discovery_root = parent.join("working");
    std::fs::create_dir_all(&discovery_root).expect("create HTTPS discovery root");
    let discovery_root = discovery_root
        .canonicalize()
        .expect("canonicalize HTTPS discovery root");
    let ambient_file = parent.join("ambient-config");
    std::fs::write(&ambient_file, b"ambient").expect("write ambient read canary");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let route = loopback_route(&backend);
    let build_command = |executable: &Path| {
        let (mut command, _observation) = backend
            .command_with_discovery_route_observation(
                executable,
                &[],
                ResolverExecutionNetworkTransport::Https,
                &route,
                &discovery_root,
            )
            .expect("build HTTPS discovery ambient-read sandbox");
        command.current_dir(&discovery_root);
        command
    };

    let output = build_command(Path::new("/bin/cat"))
        .arg(&ambient_file)
        .output()
        .expect("read ambient file content during HTTPS discovery");
    assert!(
        output.status.success(),
        "ambient content read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ambient");

    let output = build_command(Path::new("/usr/bin/stat"))
        .args([std::ffi::OsStr::new("-f"), std::ffi::OsStr::new("%N")])
        .arg(&ambient_file)
        .output()
        .expect("read ambient file metadata during HTTPS discovery");
    assert!(
        output.status.success(),
        "ambient metadata read failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    route.finish().expect("finish HTTPS discovery route");

    std::fs::remove_file(ambient_file).expect("remove ambient read canary");
    std::fs::remove_dir(discovery_root).expect("remove HTTPS discovery root");
    std::fs::remove_dir(parent).expect("remove HTTPS discovery parent");
}
