use super::*;

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_https_fetch_confines_file_content_to_mutable_and_tls_roots() {
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-https-fetch-read-{}-{sequence}",
        std::process::id()
    ));
    let mutable_root = parent.join("mutable");
    std::fs::create_dir_all(&mutable_root).expect("create HTTPS fetch mutable root");
    let mutable_root = mutable_root
        .canonicalize()
        .expect("canonicalize HTTPS fetch mutable root");
    let inside = mutable_root.join("inside");
    let sibling = parent.join("sibling");
    let escaped_link = mutable_root.join("escaped-link");
    std::fs::write(&inside, b"inside").expect("write inside canary");
    std::fs::write(&sibling, b"sibling").expect("write sibling canary");
    symlink(&sibling, &escaped_link).expect("create escaping symlink");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let route = loopback_route(&backend);
    let build_command = || {
        let (mut command, _observation) = backend
            .command_with_endpoint_route_observation(
                Path::new("/bin/cat"),
                &[],
                ResolverExecutionPhase::Fetch,
                Some(ResolverExecutionNetworkTransport::Https),
                Some(&route),
                Some(&mutable_root),
            )
            .expect("build HTTPS fetch content sandbox");
        command.current_dir(&mutable_root);
        command
    };

    let output = build_command()
        .arg(&inside)
        .output()
        .expect("read mutable-root content");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"inside");
    let output = build_command()
        .arg("/private/etc/ssl/openssl.cnf")
        .output()
        .expect("read fixed TLS configuration");
    assert!(output.status.success());
    assert!(!output.stdout.is_empty());

    for denied_path in [&sibling, &escaped_link] {
        let output = build_command()
            .arg(denied_path)
            .output()
            .expect("attempt escaped HTTPS fetch content read");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    route.finish().expect("finish HTTPS fetch route");

    std::fs::remove_file(escaped_link).expect("remove escaping symlink");
    std::fs::remove_file(inside).expect("remove inside canary");
    std::fs::remove_file(sibling).expect("remove sibling canary");
    std::fs::remove_dir(mutable_root).expect("remove HTTPS fetch mutable root");
    std::fs::remove_dir(parent).expect("remove HTTPS fetch parent");
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_https_fetch_confines_metadata_to_mutable_and_tls_roots() {
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-https-fetch-metadata-{}-{sequence}",
        std::process::id()
    ));
    let raw_mutable_root = parent.join("mutable");
    let raw_inside = raw_mutable_root.join("inside");
    let raw_sibling = parent.join("sibling");
    let raw_escaped_link = raw_mutable_root.join("escaped-link");
    std::fs::create_dir_all(&raw_inside).expect("create HTTPS fetch metadata root");
    std::fs::create_dir(&raw_sibling).expect("create HTTPS fetch metadata sibling");
    symlink(&raw_sibling, &raw_escaped_link).expect("create HTTPS fetch escaping metadata symlink");
    let mutable_root = raw_mutable_root
        .canonicalize()
        .expect("canonicalize HTTPS fetch metadata root");
    let inside = mutable_root.join("inside");
    let sibling = raw_sibling
        .canonicalize()
        .expect("canonicalize HTTPS fetch metadata sibling");
    let escaped_link = mutable_root.join("escaped-link");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let route = loopback_route(&backend);
    let run_stat = |arguments: &[&std::ffi::OsStr]| {
        let (mut command, _observation) = backend
            .command_with_endpoint_route_observation(
                Path::new("/usr/bin/stat"),
                &[],
                ResolverExecutionPhase::Fetch,
                Some(ResolverExecutionNetworkTransport::Https),
                Some(&route),
                Some(&mutable_root),
            )
            .expect("build HTTPS fetch-metadata sandbox");
        command.current_dir(&mutable_root);
        command
            .args(arguments)
            .output()
            .expect("run HTTPS fetch metadata canary")
    };

    for allowed_path in [
        inside.as_path(),
        Path::new("/private/etc/ssl/openssl.cnf"),
        Path::new("/etc/ssl/cert.pem"),
    ] {
        let output = run_stat(&[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new("%N"),
            allowed_path.as_os_str(),
        ]);
        assert!(
            output.status.success(),
            "allowed metadata failed for {}: {}",
            allowed_path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
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
    route.finish().expect("finish HTTPS fetch metadata route");

    std::fs::remove_file(escaped_link).expect("remove HTTPS fetch metadata symlink");
    std::fs::remove_dir(inside).expect("remove HTTPS fetch inside metadata canary");
    std::fs::remove_dir(sibling).expect("remove HTTPS fetch sibling metadata canary");
    std::fs::remove_dir(mutable_root).expect("remove HTTPS fetch metadata root");
    std::fs::remove_dir(parent).expect("remove HTTPS fetch metadata parent");
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_https_discovery_confines_file_content_to_working_and_tls_roots() {
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-https-discovery-read-{}-{sequence}",
        std::process::id()
    ));
    let discovery_root = parent.join("working");
    std::fs::create_dir_all(&discovery_root).expect("create HTTPS discovery root");
    let discovery_root = discovery_root
        .canonicalize()
        .expect("canonicalize HTTPS discovery root");
    let inside = discovery_root.join("inside");
    let sibling = parent.join("sibling");
    let escaped_link = discovery_root.join("escaped-link");
    std::fs::write(&inside, b"inside").expect("write inside canary");
    std::fs::write(&sibling, b"sibling").expect("write sibling canary");
    symlink(&sibling, &escaped_link).expect("create escaping symlink");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let route = loopback_route(&backend);
    let build_command = || {
        let (mut command, _observation) = backend
            .command_with_discovery_route_observation(
                Path::new("/bin/cat"),
                &[],
                ResolverExecutionNetworkTransport::Https,
                &route,
                &discovery_root,
            )
            .expect("build HTTPS discovery content sandbox");
        command.current_dir(&discovery_root);
        command
    };

    let output = build_command()
        .arg(&inside)
        .output()
        .expect("read discovery-root content");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"inside");
    let output = build_command()
        .arg("/private/etc/ssl/openssl.cnf")
        .output()
        .expect("read fixed TLS configuration");
    assert!(output.status.success());
    assert!(!output.stdout.is_empty());

    for denied_path in [&sibling, &escaped_link] {
        let output = build_command()
            .arg(denied_path)
            .output()
            .expect("attempt escaped HTTPS discovery content read");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    route.finish().expect("finish HTTPS discovery route");

    std::fs::remove_file(escaped_link).expect("remove escaping symlink");
    std::fs::remove_file(inside).expect("remove inside canary");
    std::fs::remove_file(sibling).expect("remove sibling canary");
    std::fs::remove_dir(discovery_root).expect("remove HTTPS discovery root");
    std::fs::remove_dir(parent).expect("remove HTTPS discovery parent");
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_https_discovery_confines_metadata_to_working_and_tls_roots() {
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-https-discovery-metadata-{}-{sequence}",
        std::process::id()
    ));
    let raw_discovery_root = parent.join("working");
    let raw_inside = raw_discovery_root.join("inside");
    let raw_sibling = parent.join("sibling");
    let raw_escaped_link = raw_discovery_root.join("escaped-link");
    std::fs::create_dir_all(&raw_inside).expect("create HTTPS discovery metadata root");
    std::fs::create_dir(&raw_sibling).expect("create HTTPS discovery metadata sibling");
    symlink(&raw_sibling, &raw_escaped_link)
        .expect("create HTTPS discovery escaping metadata symlink");
    let discovery_root = raw_discovery_root
        .canonicalize()
        .expect("canonicalize HTTPS discovery metadata root");
    let inside = discovery_root.join("inside");
    let sibling = raw_sibling
        .canonicalize()
        .expect("canonicalize HTTPS discovery metadata sibling");
    let escaped_link = discovery_root.join("escaped-link");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let route = loopback_route(&backend);
    let run_stat = |arguments: &[&std::ffi::OsStr]| {
        let (mut command, _observation) = backend
            .command_with_discovery_route_observation(
                Path::new("/usr/bin/stat"),
                &[],
                ResolverExecutionNetworkTransport::Https,
                &route,
                &discovery_root,
            )
            .expect("build HTTPS discovery-metadata sandbox");
        command.current_dir(&discovery_root);
        command
            .args(arguments)
            .output()
            .expect("run HTTPS discovery metadata canary")
    };

    for allowed_path in [
        inside.as_path(),
        Path::new("/private/etc/ssl/openssl.cnf"),
        Path::new("/etc/ssl/cert.pem"),
    ] {
        let output = run_stat(&[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new("%N"),
            allowed_path.as_os_str(),
        ]);
        assert!(
            output.status.success(),
            "allowed metadata failed for {}: {}",
            allowed_path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
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
    route
        .finish()
        .expect("finish HTTPS discovery metadata route");

    std::fs::remove_file(escaped_link).expect("remove HTTPS discovery metadata symlink");
    std::fs::remove_dir(inside).expect("remove HTTPS discovery inside metadata canary");
    std::fs::remove_dir(sibling).expect("remove HTTPS discovery sibling metadata canary");
    std::fs::remove_dir(discovery_root).expect("remove HTTPS discovery metadata root");
    std::fs::remove_dir(parent).expect("remove HTTPS discovery metadata parent");
}
