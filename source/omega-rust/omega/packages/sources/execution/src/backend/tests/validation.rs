use super::*;

#[test]
fn mutability_is_derived_from_the_closed_phase() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let executable = if cfg!(windows) {
        Path::new(r"C:\Windows\System32\cmd.exe")
    } else {
        Path::new("/bin/sh")
    };
    let root = inspection_root();

    assert!(
        backend
            .command_with_authority_roots_observation(
                executable,
                ResolverExecutionPhase::RepositoryInspection,
                ResolverExecutionAuthorityRoots {
                    discovery_read_root: None,
                    inspection_read_root: Some(&root),
                    mutable_root: Some(&root),
                },
            )
            .is_err()
    );
    assert!(
        backend
            .command(executable, ResolverExecutionPhase::Fetch, None)
            .is_err()
    );
}

#[test]
fn phase_specific_read_roots_are_required_exactly_for_their_phase() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let executable = if cfg!(windows) {
        Path::new(r"C:\Windows\System32\cmd.exe")
    } else {
        Path::new("/bin/sh")
    };
    let root = inspection_root();

    assert!(
        backend
            .command_with_observation(
                executable,
                ResolverExecutionPhase::RepositoryInspection,
                None,
            )
            .is_err()
    );
    assert!(
        backend
            .command_with_observation(executable, ResolverExecutionPhase::TransportDiscovery, None,)
            .is_err()
    );
    assert!(
        backend
            .command_with_discovery_observation(executable, Path::new("relative"))
            .is_err()
    );
    assert!(
        backend
            .command_with_authority_roots_observation(
                executable,
                ResolverExecutionPhase::RepositoryInitialization,
                ResolverExecutionAuthorityRoots {
                    discovery_read_root: None,
                    inspection_read_root: Some(&root),
                    mutable_root: Some(&root),
                },
            )
            .is_err()
    );
}

#[test]
fn executable_authority_rejects_directories() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let root = inspection_root();
    assert!(
        backend
            .command_with_inspection_read_root_observation(&root, &root)
            .is_err(),
        "a directory cannot become the selected resolver executable",
    );
}
