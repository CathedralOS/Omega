use super::*;

#[test]
fn observations_claim_only_phase_controls_actually_enforced() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let root = inspection_root();
    let inspection = backend
        .command_with_inspection_read_root_observation(Path::new("/bin/sh"), &root)
        .expect("build inspection observation")
        .1;
    let initialization = backend
        .command_with_observation(
            Path::new("/bin/sh"),
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&root),
        )
        .expect("build initialization observation")
        .1;
    let discovery = backend
        .command_with_discovery_observation(Path::new("/bin/sh"), &root)
        .expect("build discovery observation")
        .1;

    for local in [&inspection, &initialization] {
        assert!(local.generated_policy_sha256().is_some());
        for guarantee in [
            ResolverExecutionGuarantee::FilesystemWritesConfined,
            ResolverExecutionGuarantee::ExecutablePathsConfined,
            ResolverExecutionGuarantee::DescendantProcessesContained,
            ResolverExecutionGuarantee::NetworkDenied,
        ] {
            assert_eq!(
                disposition(local, guarantee),
                ResolverExecutionGuaranteeDisposition::Enforced
            );
        }
    }

    assert!(discovery.generated_policy_sha256().is_none());
    for guarantee in [
        ResolverExecutionGuarantee::FilesystemWritesConfined,
        ResolverExecutionGuarantee::ExecutablePathsConfined,
        ResolverExecutionGuarantee::DescendantProcessesContained,
    ] {
        assert_eq!(
            disposition(&discovery, guarantee),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
    }
    assert_eq!(
        disposition(&discovery, ResolverExecutionGuarantee::NetworkDenied),
        ResolverExecutionGuaranteeDisposition::NotRequired
    );
}
