use super::*;

#[test]
fn every_preparation_reuses_the_frozen_absolute_executable() {
    let backend = backend();
    let root = inspection_root();
    let inspection = backend
        .prepare_inspection(&root)
        .expect("prepare inspection");
    let discovery = backend.prepare_discovery(&root).expect("prepare discovery");

    assert!(backend.executable().is_absolute());
    assert_eq!(inspection.get_program(), backend.executable().as_os_str());
    assert_eq!(discovery.get_program(), backend.executable().as_os_str());
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
fn phase_roots_are_required_exactly_and_exclude_the_executable() {
    let backend = backend();
    let root = inspection_root();
    assert!(
        backend
            .prepare_with_roots(
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
            .prepare(ResolverExecutionPhase::Fetch, None)
            .is_err()
    );
    assert!(backend.prepare_discovery(Path::new("relative")).is_err());

    let executable_parent = backend
        .executable()
        .parent()
        .expect("resolver executable has parent");
    assert!(backend.prepare_inspection(executable_parent).is_err());
}

#[test]
fn executable_authority_rejects_directories() {
    let root = inspection_root();
    assert!(ResolverExecutionBackend::open(&root, &[] as &[PathBuf]).is_err());
}
