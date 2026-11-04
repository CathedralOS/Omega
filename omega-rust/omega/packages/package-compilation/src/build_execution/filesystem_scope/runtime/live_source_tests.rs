use super::*;

fn account() -> EvaluationInvocationAccount {
    let limits = evaluation_accounting::BuildEvaluationSponsorLimits::new(
        1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000, 1_000_000,
    )
    .unwrap();
    let sponsor = evaluation_accounting::BuildEvaluationSponsor::new(limits);
    sponsor
        .begin_invocation(sponsor.measurement(), 1_000_000)
        .unwrap()
}

fn scope(root: &std::path::Path) -> BuildMachineFilesystemScope {
    std::fs::create_dir(root.join("source")).unwrap();
    let sponsor = filesystem_authority::FilesystemSponsor::create_private(
        &root.join("staging"),
        filesystem_authority::FilesystemSponsorLimits::COMPILER_DEFAULT,
    )
    .unwrap();
    let output = sponsor.session_root().unwrap().join("output");
    BuildMachineFilesystemScope::for_root(&root.join("source/main.omg"), output, Some(sponsor))
}

fn read(runtime: &mut BuildFilesystemRuntime, path: &[u8]) -> [u8; 4] {
    let descriptor = runtime.open(BUILD_SOURCE_ROOT_IDENTITY, path, 0).unwrap();
    assert!(descriptor >= 3);
    let mut bytes = [0; 4];
    assert_eq!(
        runtime
            .read(BUILD_SOURCE_ROOT_IDENTITY, descriptor, &mut bytes, 4)
            .unwrap(),
        4
    );
    assert_eq!(
        runtime
            .close(BUILD_SOURCE_ROOT_IDENTITY, descriptor)
            .unwrap(),
        0
    );
    bytes
}

#[test]
fn admitted_live_source_reads_changes_and_preserves_observation_mode() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-live-source",
        false,
    )
    .unwrap();
    let mut scope = scope(temporary.path());
    std::fs::write(temporary.path().join("source/asset"), b"cold").unwrap();
    let access = scope.establish_filesystem_access().unwrap();
    assert!(!scope.is_private_snapshot_execution(&access));
    std::fs::write(temporary.path().join("source/asset"), b"live").unwrap();
    let mut runtime = scope.begin_filesystem_runtime(account()).unwrap();
    assert_eq!(&read(&mut runtime, b"asset"), b"live");
    assert_eq!(
        runtime
            .open(BUILD_SOURCE_ROOT_IDENTITY, b"asset", 1)
            .unwrap(),
        -1
    );
    let effects = runtime.finish().unwrap();
    assert_eq!(effects.attempts.len(), 4);
    assert!(
        effects
            .attempts
            .iter()
            .all(|attempt| attempt.provider == FilesystemObservationProvider::RealScoped)
    );
    assert_eq!(effects.attempts[3].grant_refusals.len(), 1);
    assert_eq!(
        std::fs::read(temporary.path().join("source/asset")).unwrap(),
        b"live"
    );
    let mut observations = crate::CapturedBuildObservations::new(&scope);
    observations.retain_filesystem_effects(effects).unwrap();
    assert!(
        observations
            .settle(true, "build")
            .unwrap()
            .captured_source_inventory()
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn admitted_live_source_cannot_be_retargeted_by_root_or_descendant_links() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-live-source-links",
        false,
    )
    .unwrap();
    let mut scope = scope(temporary.path());
    let root = temporary.path().join("source");
    let outside = temporary.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("asset"), b"evil").unwrap();
    std::fs::write(root.join("asset"), b"live").unwrap();
    std::os::unix::fs::symlink(outside.join("asset"), root.join("leaf")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("directory")).unwrap();
    scope.establish_filesystem_access().unwrap();
    std::fs::rename(&root, temporary.path().join("original")).unwrap();
    std::os::unix::fs::symlink(&outside, &root).unwrap();
    let mut runtime = scope.begin_filesystem_runtime(account()).unwrap();
    assert_eq!(&read(&mut runtime, b"asset"), b"live");
    assert_eq!(
        runtime
            .open(BUILD_SOURCE_ROOT_IDENTITY, b"leaf", 0)
            .unwrap(),
        -1
    );
    assert_eq!(
        runtime
            .open(BUILD_SOURCE_ROOT_IDENTITY, b"directory/asset", 0)
            .unwrap(),
        -1
    );
    assert_eq!(
        runtime
            .open(BUILD_SOURCE_ROOT_IDENTITY, b"../outside/asset", 0)
            .unwrap_err(),
        "Build path is not a canonical relative path"
    );
    runtime.finish().unwrap();
}

#[test]
fn independent_live_root_admissions_cannot_exchange_effects() {
    let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-live-source-identity",
        false,
    )
    .unwrap();
    let mut first = scope(temporary.path());
    let mut second = first.clone();
    std::fs::write(temporary.path().join("source/asset"), b"live").unwrap();
    first.establish_filesystem_access().unwrap();
    second.establish_filesystem_access().unwrap();
    let mut runtime = first.begin_filesystem_runtime(account()).unwrap();
    assert_eq!(&read(&mut runtime, b"asset"), b"live");
    assert!(
        crate::CapturedBuildObservations::new(&second)
            .retain_filesystem_effects(runtime.finish().unwrap())
            .is_err()
    );
}
