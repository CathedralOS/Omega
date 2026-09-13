use super::*;

#[test]
fn every_preparation_reuses_the_frozen_absolute_executable() {
    let backend = backend();
    let root = inspection_root();
    assert!(backend.executable().is_absolute());
    for phase in phases() {
        let prepared = backend.prepare(phase, &root).expect("prepare phase");
        assert_eq!(prepared.get_program(), backend.executable().as_os_str());
        assert_eq!(prepared.get_current_dir(), Some(root.as_path()));
        assert_eq!(prepared.limits().cpu_seconds, 120);
        assert_eq!(
            prepared.limits().address_space_bytes,
            8 * 1024 * 1024 * 1024
        );
        assert_eq!(prepared.limits().file_size_bytes, 1024 * 1024 * 1024);
        assert_eq!(prepared.limits().open_files, 256);
        assert_eq!(prepared.limits().active_processes, 16);
        assert_eq!(
            prepared.limits().process_memory_bytes,
            2 * 1024 * 1024 * 1024
        );
        assert_eq!(
            prepared.limits().aggregate_memory_bytes,
            4 * 1024 * 1024 * 1024
        );
    }
}

#[test]
fn executable_inside_package_controlled_root_is_rejected() {
    let root = std::env::temp_dir().join(format!(
        "omega-resolver-controlled-root-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create package-controlled root");
    let root = root.canonicalize().expect("canonical controlled root");
    let executable = root.join("selected-resolver");
    std::fs::copy(
        std::env::current_exe().expect("current executable"),
        &executable,
    )
    .expect("create selected resolver fixture");

    let error = ResolverExecutionBackend::open(&executable, std::slice::from_ref(&root))
        .expect_err("package-controlled executable must be rejected");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("inside package-controlled root"));

    std::fs::remove_dir_all(root).expect("remove package-controlled root");
}

#[cfg(unix)]
#[test]
fn executable_link_cannot_hide_a_package_controlled_target() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "omega-resolver-controlled-link-{}",
        std::process::id()
    ));
    let package_root = root.join("package");
    let host_root = root.join("host");
    std::fs::create_dir_all(&package_root).expect("create package-controlled root");
    std::fs::create_dir_all(&host_root).expect("create host-selected root");
    let package_root = package_root.canonicalize().expect("canonical package root");
    let package_executable = package_root.join("selected-resolver");
    std::fs::copy(
        std::env::current_exe().expect("current executable"),
        &package_executable,
    )
    .expect("create package-controlled executable fixture");
    let selected_link = host_root.join("selected-resolver");
    symlink(&package_executable, &selected_link).expect("link host coordinate to package target");

    let error = ResolverExecutionBackend::open(&selected_link, &[package_root])
        .expect_err("canonical package-controlled executable target must be rejected");
    assert!(error.to_string().contains("inside package-controlled root"));

    std::fs::remove_dir_all(root).expect("remove package-controlled link fixture");
}

#[test]
fn every_phase_rejects_relative_noncanonical_oversized_and_executable_roots() {
    let backend = backend();
    let root = inspection_root();
    let executable_parent = backend
        .executable()
        .parent()
        .expect("resolver executable has parent");
    // PathBuf::push normalizes `..` under Windows verbatim roots. Preserve
    // the authored spelling so this actually exercises the rejection.
    let mut parent_traversal = root.as_os_str().to_owned();
    parent_traversal.push(format!(
        "{}child{}..",
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR,
    ));
    let parent_traversal = PathBuf::from(parent_traversal);
    let oversized = root.join("a".repeat(32 * 1024 + 1));
    for phase in phases() {
        for invalid in [
            Path::new("relative"),
            parent_traversal.as_path(),
            oversized.as_path(),
            executable_parent,
        ] {
            let error = backend.prepare(phase, invalid).expect_err(&format!(
                "{phase:?} accepted invalid root {}",
                invalid.display()
            ));
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::InvalidInput,
                "{phase:?}: {error}"
            );
        }
    }
}

fn phases() -> [ResolverExecutionPhase; 4] {
    [
        ResolverExecutionPhase::TransportDiscovery,
        ResolverExecutionPhase::RepositoryInitialization,
        ResolverExecutionPhase::Fetch,
        ResolverExecutionPhase::RepositoryInspection,
    ]
}

#[test]
fn executable_authority_rejects_directories() {
    let root = inspection_root();
    assert!(ResolverExecutionBackend::open(&root, &[] as &[PathBuf]).is_err());
}
