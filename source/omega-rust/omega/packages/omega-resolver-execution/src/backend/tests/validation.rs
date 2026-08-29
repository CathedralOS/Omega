use super::*;

#[test]
fn mutability_is_derived_from_the_closed_phase() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let executable = if cfg!(windows) {
        Path::new(r"C:\Windows\System32\cmd.exe")
    } else {
        Path::new("/bin/sh")
    };
    let inspection_root = inspection_root();
    assert!(
        backend
            .command_with_authority_roots_observation(
                executable,
                &[],
                ResolverExecutionPhase::RepositoryInspection,
                None,
                None,
                ResolverExecutionAuthorityRoots {
                    discovery_read_root: None,
                    inspection_read_root: Some(&inspection_root),
                    mutable_root: Some(Path::new("/tmp")),
                },
            )
            .is_err()
    );
    assert!(
        backend
            .command(executable, &[], ResolverExecutionPhase::Fetch, None,)
            .is_err()
    );
}

#[test]
fn endpoint_routes_are_required_exactly_for_network_phases() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let executable = if cfg!(windows) {
        Path::new(r"C:\Windows\System32\cmd.exe")
    } else {
        Path::new("/bin/sh")
    };
    let mutable_root = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary root");
    let route = loopback_route(&backend);
    let inspection_root = inspection_root();

    let discovery = backend
        .command_with_discovery_route_observation(
            executable,
            &[],
            ResolverExecutionNetworkTransport::Https,
            &route,
            &mutable_root,
        )
        .expect("construct discovery policy")
        .1;
    assert_eq!(
        discovery.discovery_read_root(),
        Some(mutable_root.as_path())
    );
    let alternate_discovery_root = mutable_root.join("alternate");
    let alternate_discovery = backend
        .command_with_discovery_route_observation(
            executable,
            &[],
            ResolverExecutionNetworkTransport::Https,
            &route,
            &alternate_discovery_root,
        )
        .expect("construct alternate discovery policy")
        .1;
    assert_ne!(
        discovery.canonical_bytes(),
        alternate_discovery.canonical_bytes()
    );
    assert!(
        backend
            .command_with_discovery_route_observation(
                executable,
                &[],
                ResolverExecutionNetworkTransport::Https,
                &route,
                Path::new("relative"),
            )
            .is_err()
    );
    assert!(
        backend
            .command_with_endpoint_route_observation(
                executable,
                &[],
                ResolverExecutionPhase::Fetch,
                Some(ResolverExecutionNetworkTransport::Https),
                Some(&route),
                Some(&mutable_root),
            )
            .is_ok()
    );
    for (phase, mutable_root) in [
        (ResolverExecutionPhase::TransportDiscovery, None),
        (ResolverExecutionPhase::Fetch, Some(mutable_root.as_path())),
    ] {
        assert!(
            backend
                .command_with_observation(
                    executable,
                    &[],
                    phase,
                    Some(ResolverExecutionNetworkTransport::Https),
                    mutable_root,
                )
                .is_err()
        );
    }
    for (phase, mutable_root) in [
        (ResolverExecutionPhase::RepositoryInspection, None),
        (
            ResolverExecutionPhase::RepositoryInitialization,
            Some(mutable_root.as_path()),
        ),
    ] {
        assert!(
            backend
                .command_with_authority_roots_observation(
                    executable,
                    &[],
                    phase,
                    None,
                    Some(&route),
                    ResolverExecutionAuthorityRoots {
                        discovery_read_root: None,
                        inspection_read_root: (phase
                            == ResolverExecutionPhase::RepositoryInspection)
                            .then_some(inspection_root.as_path()),
                        mutable_root,
                    },
                )
                .is_err()
        );
    }
}

#[test]
fn inspection_read_roots_are_required_exactly_for_inspection() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let executable = if cfg!(windows) {
        Path::new(r"C:\Windows\System32\cmd.exe")
    } else {
        Path::new("/bin/sh")
    };
    let inspection_root = inspection_root();

    assert!(
        backend
            .command_with_observation(
                executable,
                &[],
                ResolverExecutionPhase::RepositoryInspection,
                None,
                None,
            )
            .is_err(),
        "inspection requires an explicit content-read root"
    );
    assert!(
        backend
            .command_with_authority_roots_observation(
                executable,
                &[],
                ResolverExecutionPhase::RepositoryInitialization,
                None,
                None,
                ResolverExecutionAuthorityRoots {
                    discovery_read_root: None,
                    inspection_read_root: Some(&inspection_root),
                    mutable_root: Some(&inspection_root),
                },
            )
            .is_err(),
        "other phases reject inspection content-read authority"
    );
    assert!(
        backend
            .command_with_inspection_read_root_observation(executable, &[], Path::new("relative"),)
            .is_err(),
        "inspection content-read roots must be absolute"
    );
}
