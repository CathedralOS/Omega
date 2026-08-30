use super::*;

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
