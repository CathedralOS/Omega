use super::*;
use crate::{
    ResolverExecutionBackend, ResolverExecutionEndpointOutcome, ResolverExecutionEndpointRoute,
    ResolverExecutionGuarantee, ResolverExecutionGuaranteeDisposition,
    ResolverExecutionNetworkTransport, ResolverExecutionPhase, ResolverExecutionPolicyObservation,
    ResolverExecutionRequestedEndpoint, ResolverExecutionTransferBudget,
};
use std::path::{Path, PathBuf};
use std::process::Stdio;

fn loopback_route(backend: &ResolverExecutionBackend) -> ResolverExecutionEndpointRoute {
    backend
        .open_endpoint_route(
            ResolverExecutionRequestedEndpoint::new("127.0.0.1", 9)
                .expect("construct loopback endpoint"),
            ResolverExecutionTransferBudget::new(1024 * 1024).expect("construct transfer budget"),
        )
        .expect("open loopback endpoint route")
}

fn inspection_root() -> PathBuf {
    std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary inspection root")
}

#[cfg(target_os = "macos")]
#[test]
fn confined_metadata_paths_are_derived_deduplicated_and_bounded() {
    let additional = [
        PathBuf::from("/bin/sh"),
        PathBuf::from("/usr/bin/git"),
        PathBuf::from("/usr/bin/git"),
    ];
    let paths = macos_confined_metadata_paths(
        Path::new("/usr/bin/git"),
        &additional,
        &[
            Path::new("/private/tmp/repository"),
            Path::new(MACOS_TLS_CONFIGURATION_ROOT),
            Path::new(MACOS_TLS_CONFIGURATION_ALIAS_ROOT),
        ],
    )
    .expect("derive metadata paths");
    assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
    for required in [
        "/",
        "/bin",
        "/bin/sh",
        "/dev",
        "/dev/null",
        "/private",
        "/private/tmp",
        "/private/tmp/repository",
        "/private/etc",
        "/private/etc/ssl",
        "/etc",
        "/etc/ssl",
        "/usr",
        "/usr/bin",
        "/usr/bin/git",
    ] {
        assert!(paths.iter().any(|path| path == Path::new(required)));
    }
    assert!(
        !paths
            .iter()
            .any(|path| path == Path::new("/private/tmp/sibling"))
    );

    let mut excessive = PathBuf::from("/");
    for _ in 0..MACOS_CONFINED_METADATA_PATH_LIMIT {
        excessive.push("a");
    }
    assert!(macos_confined_metadata_paths(Path::new("/bin/sh"), &[], &[&excessive]).is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn helper_metadata_roots_are_derived_deduplicated_and_never_global() {
    let roots = macos_helper_metadata_roots(&[
        PathBuf::from("/opt/omega/libexec/git-remote-https"),
        PathBuf::from("/opt/omega/libexec/git-remote-http"),
    ])
    .expect("derive helper metadata roots");
    assert_eq!(roots, [PathBuf::from("/opt/omega/libexec")]);
    assert!(macos_helper_metadata_roots(&[PathBuf::from("/helper")]).is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_observation_reports_exact_known_enforcement_and_gaps() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let executable = Path::new("/bin/sh");
    let mutable_root = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary root");
    let (inspection_command, inspection) = backend
        .command_with_inspection_read_root_observation(executable, &[], &mutable_root)
        .expect("issue inspection policy observation");
    let (initialization_command, initialization) = backend
        .command_with_observation(
            executable,
            &[],
            ResolverExecutionPhase::RepositoryInitialization,
            None,
            Some(&mutable_root),
        )
        .expect("issue initialization policy observation");
    let discovery_route = loopback_route(&backend);
    let (discovery_command, discovery) = backend
        .command_with_discovery_route_observation(
            executable,
            &[],
            ResolverExecutionNetworkTransport::Ssh,
            &discovery_route,
            &mutable_root,
        )
        .expect("issue discovery policy observation");
    let https_discovery_route = loopback_route(&backend);
    let (https_discovery_command, https_discovery) = backend
        .command_with_discovery_route_observation(
            executable,
            &[PathBuf::from("/usr/bin/stat")],
            ResolverExecutionNetworkTransport::Https,
            &https_discovery_route,
            &mutable_root,
        )
        .expect("issue HTTPS discovery policy observation");
    let fetch_route = loopback_route(&backend);
    let (fetch_command, fetch) = backend
        .command_with_endpoint_route_observation(
            executable,
            &[],
            ResolverExecutionPhase::Fetch,
            Some(ResolverExecutionNetworkTransport::Ssh),
            Some(&fetch_route),
            Some(&mutable_root),
        )
        .expect("issue fetch policy observation");
    let https_fetch_route = loopback_route(&backend);
    let (https_fetch_command, https_fetch) = backend
        .command_with_endpoint_route_observation(
            executable,
            &[PathBuf::from("/usr/bin/stat")],
            ResolverExecutionPhase::Fetch,
            Some(ResolverExecutionNetworkTransport::Https),
            Some(&https_fetch_route),
            Some(&mutable_root),
        )
        .expect("issue HTTPS fetch policy observation");
    assert!(inspection.generated_policy_sha256().is_some());
    assert!(initialization.generated_policy_sha256().is_some());
    assert!(discovery.generated_policy_sha256().is_some());
    assert!(fetch.generated_policy_sha256().is_some());
    assert!(https_fetch.generated_policy_sha256().is_some());
    assert_ne!(
        inspection.generated_policy_sha256(),
        fetch.generated_policy_sha256()
    );
    assert_ne!(
        discovery.generated_policy_sha256(),
        https_discovery.generated_policy_sha256()
    );
    assert_ne!(
        discovery.canonical_bytes(),
        https_discovery.canonical_bytes()
    );
    assert_ne!(fetch.canonical_bytes(), https_fetch.canonical_bytes());
    assert_eq!(
        discovery.discovery_read_root(),
        Some(mutable_root.as_path())
    );

    let profile = |command: &std::process::Command| {
        let arguments = command.get_args().collect::<Vec<_>>();
        let profile_index = arguments
            .iter()
            .position(|argument| *argument == "-p")
            .expect("Seatbelt command carries an inline policy")
            + 1;
        arguments[profile_index]
            .to_str()
            .expect("compiler-generated policy is UTF-8")
            .to_owned()
    };
    let inspection_profile = profile(&inspection_command);
    assert!(!inspection_profile.contains("(import"));
    assert!(!inspection_profile.contains("network-outbound"));
    assert!(!inspection_profile.contains("file-write*"));
    assert!(!inspection_profile.contains("process-fork"));
    assert!(!inspection_profile.contains("(allow file-read*)"));
    assert!(!inspection_profile.contains("(allow file-read-metadata)"));
    assert!(inspection_profile.contains(
        "file-read-metadata file-test-existence (subpath (param \"INSPECTION_READ_ROOT\"))"
    ));
    assert!(inspection_profile.contains("(literal (param \"METADATA_PATH_0\"))"));
    assert!(
        inspection_profile
            .contains("(allow file-read-data (subpath (param \"INSPECTION_READ_ROOT\"))")
    );
    assert!(
        inspection_profile
            .contains("(allow file-test-existence file-write-data (literal \"/dev/null\"))")
    );
    let initialization_profile = profile(&initialization_command);
    assert!(!initialization_profile.contains("(import"));
    assert!(!initialization_profile.contains("network-outbound"));
    assert!(!initialization_profile.contains("process-fork"));
    assert!(!initialization_profile.contains("(allow file-read*)"));
    assert!(!initialization_profile.contains("(allow file-read-metadata)"));
    assert!(
        initialization_profile
            .contains("file-read-metadata file-test-existence (subpath (param \"MUTABLE_ROOT\"))")
    );
    assert!(initialization_profile.contains("(literal (param \"METADATA_PATH_0\"))"));
    assert!(
        initialization_profile.contains("(allow file-read-data (subpath (param \"MUTABLE_ROOT\"))")
    );
    assert!(
        initialization_profile.contains("(allow file-write* (subpath (param \"MUTABLE_ROOT\")))")
    );
    assert!(
        initialization_profile
            .contains("(allow file-test-existence file-write-data (literal \"/dev/null\"))")
    );
    let discovery_profile = profile(&discovery_command);
    assert!(!discovery_profile.contains("(import"));
    assert!(
        discovery_profile
            .contains("(allow network-outbound (remote tcp (param \"BROKER_ENDPOINT\")))")
    );
    assert!(!discovery_profile.contains("(allow network-outbound)"));
    assert!(!discovery_profile.contains("file-write*"));
    assert!(discovery_profile.contains("(allow process-fork)"));
    assert!(
        discovery_profile.contains(
            "(allow mach-lookup (global-name \"com.apple.system.opendirectoryd.libinfo\"))"
        )
    );
    assert!(discovery_profile.contains("(allow sysctl-read (sysctl-name \"kern.hostname\"))"));
    assert!(discovery_profile.contains("(allow sysctl-read (sysctl-name \"hw.pagesize_compat\"))"));
    assert!(!discovery_profile.contains("(allow sysctl-read)"));
    let https_discovery_profile = profile(&https_discovery_command);
    assert!(
        https_discovery_profile
            .contains("(allow network-outbound (remote tcp (param \"BROKER_ENDPOINT\")))")
    );
    assert!(!https_discovery_profile.contains("mach-lookup"));
    assert!(!https_discovery_profile.contains("sysctl-read"));
    assert!(!https_discovery_profile.contains("(allow file-read*)"));
    assert!(!https_discovery_profile.contains("(allow file-read-metadata)"));
    assert!(https_discovery_profile.contains(
        "file-read-metadata file-test-existence (subpath (param \"DISCOVERY_READ_ROOT\"))"
    ));
    assert!(https_discovery_profile.contains("(literal (param \"METADATA_PATH_0\"))"));
    assert!(https_discovery_profile.contains("(subpath (param \"METADATA_SUBPATH_0\"))"));
    assert!(https_discovery_profile.contains("(subpath \"/etc/ssl\")"));
    assert!(
        https_discovery_profile
            .contains("(allow file-read-data (subpath (param \"DISCOVERY_READ_ROOT\"))")
    );
    assert!(https_discovery_profile.contains("(subpath \"/private/etc/ssl\")"));
    let fetch_profile = profile(&fetch_command);
    assert!(!fetch_profile.contains("(import"));
    assert!(
        fetch_profile.contains("(allow network-outbound (remote tcp (param \"BROKER_ENDPOINT\")))")
    );
    assert!(fetch_profile.contains("(allow file-write* (subpath (param \"MUTABLE_ROOT\")))"));
    assert!(fetch_profile.contains("(allow process-fork)"));
    assert!(
        fetch_profile.contains(
            "(allow mach-lookup (global-name \"com.apple.system.opendirectoryd.libinfo\"))"
        )
    );
    assert!(fetch_profile.contains("(allow sysctl-read (sysctl-name \"kern.hostname\"))"));
    assert!(fetch_profile.contains("(allow sysctl-read (sysctl-name \"hw.pagesize_compat\"))"));
    assert!(fetch_profile.contains("(allow file-read*)"));
    let https_fetch_profile = profile(&https_fetch_command);
    assert!(!https_fetch_profile.contains("(allow file-read*)"));
    assert!(!https_fetch_profile.contains("(allow file-read-metadata)"));
    assert!(
        https_fetch_profile
            .contains("file-read-metadata file-test-existence (subpath (param \"MUTABLE_ROOT\"))")
    );
    assert!(https_fetch_profile.contains("(literal (param \"METADATA_PATH_0\"))"));
    assert!(https_fetch_profile.contains("(subpath (param \"METADATA_SUBPATH_0\"))"));
    assert!(https_fetch_profile.contains("(subpath \"/etc/ssl\")"));
    assert!(
        https_fetch_profile.contains("(allow file-read-data (subpath (param \"MUTABLE_ROOT\"))")
    );
    assert!(https_fetch_profile.contains("(subpath \"/private/etc/ssl\")"));
    assert!(!https_fetch_profile.contains("mach-lookup"));
    assert!(!https_fetch_profile.contains("sysctl-read"));

    let disposition = |observation: &ResolverExecutionPolicyObservation, guarantee| {
        observation
            .guarantees()
            .iter()
            .find(|row| row.guarantee() == guarantee)
            .expect("complete guarantee row set")
            .disposition()
    };
    assert_eq!(
        disposition(
            &inspection,
            ResolverExecutionGuarantee::FilesystemWritesConfined
        ),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    for guarantee in [
        ResolverExecutionGuarantee::FilesystemWritesConfined,
        ResolverExecutionGuarantee::NetworkDenied,
        ResolverExecutionGuarantee::ExecutablePathsConfined,
        ResolverExecutionGuarantee::DescendantProcessesContained,
    ] {
        assert_eq!(
            disposition(&initialization, guarantee),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
    }
    assert_eq!(
        disposition(
            &initialization,
            ResolverExecutionGuarantee::FilesystemReadsConfined
        ),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(
            &discovery,
            ResolverExecutionGuarantee::FilesystemWritesConfined
        ),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(
            &discovery,
            ResolverExecutionGuarantee::ExecutablePathsConfined
        ),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(&discovery, ResolverExecutionGuarantee::NetworkDenied),
        ResolverExecutionGuaranteeDisposition::NotRequired
    );
    assert_eq!(
        disposition(
            &discovery,
            ResolverExecutionGuarantee::NetworkEndpointsConfined
        ),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(
            &discovery,
            ResolverExecutionGuarantee::FilesystemReadsConfined
        ),
        ResolverExecutionGuaranteeDisposition::Unavailable
    );
    assert_eq!(
        disposition(
            &https_discovery,
            ResolverExecutionGuarantee::FilesystemReadsConfined
        ),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(&fetch, ResolverExecutionGuarantee::FilesystemReadsConfined),
        ResolverExecutionGuaranteeDisposition::Unavailable
    );
    assert_eq!(
        disposition(
            &https_fetch,
            ResolverExecutionGuarantee::FilesystemReadsConfined
        ),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(
            &inspection,
            ResolverExecutionGuarantee::FilesystemReadsConfined
        ),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(&inspection, ResolverExecutionGuarantee::NetworkDenied),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(
            &inspection,
            ResolverExecutionGuarantee::ExecutablePathsConfined
        ),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(
            &inspection,
            ResolverExecutionGuarantee::DescendantProcessesContained
        ),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(
            &discovery,
            ResolverExecutionGuarantee::DescendantProcessesContained
        ),
        ResolverExecutionGuaranteeDisposition::Unavailable
    );
    assert_eq!(
        disposition(&fetch, ResolverExecutionGuarantee::NetworkDenied),
        ResolverExecutionGuaranteeDisposition::NotRequired
    );
    assert_eq!(
        disposition(&fetch, ResolverExecutionGuarantee::NetworkEndpointsConfined),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(&fetch, ResolverExecutionGuarantee::ExecutablePathsConfined),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(&fetch, ResolverExecutionGuarantee::FilesystemWritesConfined),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(&fetch, ResolverExecutionGuarantee::CoreDumpsDenied),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(&fetch, ResolverExecutionGuarantee::CpuTimeConfined),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(&fetch, ResolverExecutionGuarantee::SingleFileSizeConfined),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(&fetch, ResolverExecutionGuarantee::OpenFilesConfined),
        ResolverExecutionGuaranteeDisposition::Enforced
    );
    assert_eq!(
        disposition(&fetch, ResolverExecutionGuarantee::AddressSpaceConfined),
        ResolverExecutionGuaranteeDisposition::Unavailable
    );
}

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

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_denies_nonnetwork_tcp_and_confines_network_phases_to_the_broker() {
    use std::io::ErrorKind;
    use std::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback canary");
    listener
        .set_nonblocking(true)
        .expect("make canary listener nonblocking");
    let port = listener.local_addr().expect("read canary address").port();
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let inspection_root = inspection_root();
    let mut denied = backend
        .command_with_inspection_read_root(Path::new("/usr/bin/nc"), &[], &inspection_root)
        .expect("build network-denied sandbox");
    let status = denied
        .args(["127.0.0.1", &port.to_string()])
        .stdin(Stdio::null())
        .status()
        .expect("attempt denied loopback connection");
    assert!(!status.success());
    assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));

    let mutable_root = std::env::temp_dir().join(format!(
        "omega-resolver-network-denial-{}-{port}",
        std::process::id(),
    ));
    std::fs::create_dir(&mutable_root).expect("create network-denial mutable root");
    let mutable_root = mutable_root
        .canonicalize()
        .expect("canonicalize network-denial mutable root");
    let mut denied = backend
        .command(
            Path::new("/usr/bin/nc"),
            &[],
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&mutable_root),
        )
        .expect("build initialization network-denied sandbox");
    denied.current_dir(&mutable_root);
    let status = denied
        .args(["127.0.0.1", &port.to_string()])
        .stdin(Stdio::null())
        .status()
        .expect("attempt denied initialization loopback connection");
    assert!(!status.success());
    assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));
    std::fs::remove_dir(mutable_root).expect("remove network-denial mutable root");

    let route = backend
        .open_endpoint_route(
            ResolverExecutionRequestedEndpoint::new("127.0.0.1", port)
                .expect("construct broker destination"),
            ResolverExecutionTransferBudget::new(1024 * 1024).expect("construct transfer budget"),
        )
        .expect("open endpoint route");
    let broker_port = route.policy().broker_endpoint().port();
    let discovery_read_root = inspection_root.clone();
    let (mut allowed, _) = backend
        .command_with_discovery_route_observation(
            Path::new("/usr/bin/nc"),
            &[],
            ResolverExecutionNetworkTransport::Https,
            &route,
            &discovery_read_root,
        )
        .expect("build network-enabled sandbox");
    let status = allowed
        .args(["127.0.0.1", &broker_port.to_string()])
        .stdin(Stdio::null())
        .status()
        .expect("connect to exact loopback broker");
    assert!(status.success());

    let (mut direct, _) = backend
        .command_with_discovery_route_observation(
            Path::new("/usr/bin/nc"),
            &[],
            ResolverExecutionNetworkTransport::Https,
            &route,
            &discovery_read_root,
        )
        .expect("build endpoint-confined sandbox");
    let status = direct
        .args(["127.0.0.1", &port.to_string()])
        .stdin(Stdio::null())
        .status()
        .expect("attempt direct second-loopback connection");
    assert!(!status.success());
    assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));

    let observation = route.finish().expect("finish endpoint route");
    assert_eq!(observation.events().len(), 1);
    assert_eq!(
        observation.events()[0].outcome(),
        ResolverExecutionEndpointOutcome::MalformedConnect
    );
}
