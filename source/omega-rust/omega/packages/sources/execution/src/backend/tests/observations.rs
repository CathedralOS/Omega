use super::*;
use crate::request::RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT;
#[cfg(windows)]
use crate::{ResolverExecutionBackendIdentity, ResolverExecutionGuarantee};

#[test]
fn policy_observation_is_complete_canonical_and_locally_fail_closed() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let executable = if cfg!(windows) {
        Path::new(r"C:\Windows\System32\cmd.exe")
    } else {
        Path::new("/bin/sh")
    };
    let mutable_root = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary root");
    let inspection_root = inspection_root();
    let (_, inspection) = backend
        .command_with_inspection_read_root_observation(executable, &[], &inspection_root)
        .expect("issue inspection policy observation");
    let fetch_route = loopback_route(&backend);
    let (_, fetch) = backend
        .command_with_endpoint_route_observation(
            executable,
            &[],
            ResolverExecutionPhase::Fetch,
            Some(ResolverExecutionNetworkTransport::Ssh),
            Some(&fetch_route),
            Some(&mutable_root),
        )
        .expect("issue fetch policy observation");
    assert_eq!(inspection.network_transport(), None);
    assert!(inspection.endpoint_route().is_none());
    assert_eq!(
        fetch.network_transport(),
        Some(ResolverExecutionNetworkTransport::Ssh)
    );
    assert_eq!(
        fetch
            .endpoint_route()
            .expect("fetch route policy")
            .requested_endpoint()
            .port(),
        9
    );
    assert!(
        backend
            .command_with_observation(
                executable,
                &[],
                ResolverExecutionPhase::TransportDiscovery,
                None,
                None,
            )
            .is_err(),
        "networked phases require explicit transport authority"
    );
    assert!(
        backend
            .command_with_authority_roots_observation(
                executable,
                &[],
                ResolverExecutionPhase::RepositoryInspection,
                Some(ResolverExecutionNetworkTransport::Https),
                None,
                ResolverExecutionAuthorityRoots {
                    discovery_read_root: None,
                    inspection_read_root: Some(&inspection_root),
                    mutable_root: None,
                },
            )
            .is_err(),
        "nonnetwork phases reject transport authority"
    );
    assert!(
        backend
            .command_with_authority_roots_observation(
                executable,
                &[],
                ResolverExecutionPhase::RepositoryInspection,
                None,
                Some(&fetch_route),
                ResolverExecutionAuthorityRoots {
                    discovery_read_root: None,
                    inspection_read_root: Some(&inspection_root),
                    mutable_root: None,
                },
            )
            .is_err(),
        "nonnetwork phases reject endpoint routes"
    );

    assert_eq!(inspection.guarantees().len(), 13);
    assert!(
        inspection
            .guarantees()
            .windows(2)
            .all(|rows| rows[0].guarantee() < rows[1].guarantee())
    );
    assert_eq!(
        inspection.canonical_bytes(),
        backend
            .command_with_inspection_read_root_observation(executable, &[], &inspection_root,)
            .expect("reissue inspection policy observation")
            .1
            .canonical_bytes()
    );
    assert_ne!(inspection.canonical_bytes(), fetch.canonical_bytes());
    let alternate_inspection_root = inspection_root.join("alternate");
    let alternate_inspection = backend
        .command_with_inspection_read_root_observation(executable, &[], &alternate_inspection_root)
        .expect("issue alternate inspection policy observation")
        .1;
    assert_ne!(
        inspection.canonical_bytes(),
        alternate_inspection.canonical_bytes()
    );
    assert_eq!(inspection.executable(), executable);
    assert_eq!(
        inspection.inspection_read_root(),
        Some(inspection_root.as_path())
    );
    assert_eq!(fetch.mutable_root(), Some(mutable_root.as_path()));
    #[cfg(unix)]
    {
        assert_eq!(inspection.resource_ceilings().core_dump_bytes(), Some(0));
        assert_eq!(inspection.resource_ceilings().cpu_seconds(), Some(120));
        assert_eq!(
            inspection.resource_ceilings().single_file_bytes(),
            Some(1024 * 1024 * 1024)
        );
        assert_eq!(inspection.resource_ceilings().open_files(), Some(256));
    }
    #[cfg(windows)]
    {
        assert!(matches!(
            backend.identity(),
            ResolverExecutionBackendIdentity::WindowsJobObject
        ));
        assert_eq!(inspection.resource_ceilings().process_count(), Some(16));
        assert_eq!(
            inspection.resource_ceilings().per_process_memory_bytes(),
            Some(2 * 1024 * 1024 * 1024)
        );
        assert_eq!(
            inspection.resource_ceilings().aggregate_memory_bytes(),
            Some(4 * 1024 * 1024 * 1024)
        );
        assert_eq!(
            inspection.resource_ceilings().aggregate_cpu_seconds(),
            Some(120)
        );
        let disposition = |guarantee| {
            inspection
                .guarantees()
                .iter()
                .find(|row| row.guarantee() == guarantee)
                .expect("complete Windows guarantee row")
                .disposition()
        };
        for guarantee in [
            ResolverExecutionGuarantee::DescendantProcessesContained,
            ResolverExecutionGuarantee::CpuTimeConfined,
            ResolverExecutionGuarantee::ProcessCountConfined,
            ResolverExecutionGuarantee::AggregateResourcesConfined,
        ] {
            assert_eq!(
                disposition(guarantee),
                ResolverExecutionGuaranteeDisposition::Enforced
            );
        }
        assert_eq!(
            disposition(ResolverExecutionGuarantee::FilesystemReadsConfined),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
    }
}

#[test]
fn policy_observation_normalizes_and_bounds_executable_sets() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let executable = if cfg!(windows) {
        Path::new(r"C:\Windows\System32\cmd.exe")
    } else {
        Path::new("/bin/sh")
    };
    let first = if cfg!(windows) {
        Path::new(r"C:\Windows\System32\where.exe").to_path_buf()
    } else {
        Path::new("/bin/bash").to_path_buf()
    };
    let second = if cfg!(windows) {
        Path::new(r"C:\Windows\System32\whoami.exe").to_path_buf()
    } else {
        Path::new("/usr/bin/git").to_path_buf()
    };
    let inspection_root = inspection_root();
    let (_, left) = backend
        .command_with_inspection_read_root_observation(
            executable,
            &[second.clone(), first.clone(), second.clone()],
            &inspection_root,
        )
        .expect("construct normalized policy observation");
    let (_, right) = backend
        .command_with_inspection_read_root_observation(
            executable,
            &[first.clone(), second.clone()],
            &inspection_root,
        )
        .expect("reconstruct normalized policy observation");
    assert_eq!(left.canonical_bytes(), right.canonical_bytes());
    assert_eq!(left.additional_executables(), &[first, second]);

    let excessive = vec![
        if cfg!(windows) {
            Path::new(r"C:\Windows\System32\where.exe").to_path_buf()
        } else {
            Path::new("/bin/bash").to_path_buf()
        };
        RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT + 1
    ];
    assert!(
        backend
            .command_with_inspection_read_root_observation(
                executable,
                &excessive,
                &inspection_root,
            )
            .is_err()
    );
}
