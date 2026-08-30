use super::*;

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
    assert!(
        inspection_profile
            .contains("(allow file-test-existence file-write-data (literal \"/dev/null\"))")
    );
    let initialization_profile = profile(&initialization_command);
    assert!(!initialization_profile.contains("(import"));
    assert!(!initialization_profile.contains("network-outbound"));
    assert!(!initialization_profile.contains("process-fork"));
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
    let https_fetch_profile = profile(&https_fetch_command);
    assert!(!https_fetch_profile.contains("mach-lookup"));
    assert!(!https_fetch_profile.contains("sysctl-read"));
    for profile in [
        &inspection_profile,
        &initialization_profile,
        &discovery_profile,
        &https_discovery_profile,
        &fetch_profile,
        &https_fetch_profile,
    ] {
        assert!(
            profile.contains("(allow file-read*)"),
            "every resolver phase must retain ambient host reads"
        );
    }

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
    for observation in [
        &initialization,
        &inspection,
        &discovery,
        &https_discovery,
        &fetch,
        &https_fetch,
    ] {
        assert_eq!(
            disposition(
                observation,
                ResolverExecutionGuarantee::FilesystemReadsConfined
            ),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
    }
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
