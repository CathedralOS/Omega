//! Preparation lifecycle controls. These exercise package recovery/custody,
//! not compiler acceptance; the Build SDK executes the retained source route.

use super::*;
use platform_custody::temporary_directory::TemporaryDirectory;

fn project(fixture: &str) -> TemporaryDirectory {
    let temporary = TemporaryDirectory::create("project-preparation", false).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .join("tests/omega/pass")
        .join(fixture);
    for name in ["build.omg", "main.omg"] {
        std::fs::copy(fixture.join(name), temporary.path().join(name)).unwrap();
    }
    temporary
}

fn begin(root: &Path) -> LocalProjectPreparation {
    begin_local_project_preparation(
        &root.join("main.omg"),
        &[TargetProfile::host_if_supported().unwrap()],
        true,
    )
    .unwrap()
    .unwrap()
}

#[test]
fn beginning_preparation_does_not_project_dynamic_declarations() {
    let temporary = project("build/executed_dependency_source_join_exit");
    let pending = begin(temporary.path());
    let acquired = pending.acquire_root().unwrap();
    assert_eq!(
        acquired.source_capture().canonical_live_root(),
        temporary.path().canonicalize().unwrap()
    );
    // The missing selected/unselected dependency directories do not matter
    // before Build actually requests source work.
    assert!(pending.accepted.is_none());
    assert!(pending.transaction.is_none());
}

#[test]
fn synchronous_preparation_keeps_its_existing_source_result() {
    let temporary = project("calls/machine_self_call_recursion_compile");
    let prepared = prepare_local_project_for_targets(
        &temporary.path().join("main.omg"),
        &[TargetProfile::host_if_supported().unwrap()],
        true,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        prepared.entry_path(),
        temporary.path().canonicalize().unwrap().join("main.omg")
    );
    assert_eq!(
        prepared.source_closure.graph().root().name().as_str(),
        "machine_self_call_recursion_compile"
    );
}

#[test]
fn completion_refuses_another_projects_resolved_graph() {
    let first = project("calls/machine_self_call_recursion_compile");
    let second = project("calls/machine_self_call_recursion_compile");
    let first = begin(first.path());
    let second = begin(second.path());
    let closure = second.resolve_static_source_closure().unwrap();
    let error = first
        .complete(closure)
        .err()
        .expect("foreign graph must refuse");
    assert!(matches!(error, PrepareLocalProjectError::SourceJoin(_)));
}

#[test]
fn preparation_retains_transaction_exclusion_and_pair_baseline() {
    for changed_file in ["build.omg", "omega.lock"] {
        let temporary = project("calls/machine_self_call_recursion_compile");
        drop(
            PackageFileTransaction::open(temporary.path(), PackagePublicationLimits::default())
                .unwrap(),
        );
        let pending = begin(temporary.path());
        assert!(matches!(
            PackageFileTransaction::open(temporary.path(), PackagePublicationLimits::default()),
            Err(PackagePublicationError::Busy)
        ));
        let closure = pending.resolve_static_source_closure().unwrap();
        std::fs::write(
            temporary.path().join(changed_file),
            b"concurrent owner edit",
        )
        .unwrap();
        let error = pending
            .complete(closure)
            .err()
            .expect("changed accepted pair must refuse");
        assert!(matches!(error, PrepareLocalProjectError::Publication(
            PackagePublicationError::ConcurrentEdit { file }
        ) if file == changed_file));
        // Completion failure releases its retained mutex without publishing.
        assert!(
            PackageFileTransaction::open(temporary.path(), PackagePublicationLimits::default())
                .is_ok()
        );
    }
}

#[test]
fn invalid_lock_refuses_before_storage_or_source_acquisition() {
    let temporary = project("calls/machine_self_call_recursion_compile");
    std::fs::write(temporary.path().join("omega.lock"), b"not an accepted lock").unwrap();
    let result = begin_local_project_preparation_in_storage(
        &temporary.path().join("main.omg"),
        &[TargetProfile::host_if_supported().unwrap()],
        true,
        |_| panic!("storage must not open before lock recovery succeeds"),
    );
    assert!(matches!(result, Err(PrepareLocalProjectError::Locked(_))));
}
