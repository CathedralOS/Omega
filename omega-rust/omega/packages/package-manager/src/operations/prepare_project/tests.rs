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

// This is a transaction/graph test, not a compiler witness. Supply the declared
// name and empty request ledger directly; SDK tests execute real Build sources.
fn source_graph(preparation: &LocalProjectPreparation) -> ResolvedPackageSourceClosure {
    use crate::declarations::dependencies::read::DependencyProjections;
    use crate::declarations::{BuildDeclarationKind, PackageKey, PackageName};
    use crate::resolution::graph::reconcile::{SourceClosureStep, begin_source_closure};
    use crate::resolution::source::{
        PackageSourceCustody, PackageSourceMaterialization, PackageSourceNavigation,
        PackageSourceSelectionEvidence,
    };
    let acquired = preparation.acquire_root().unwrap();
    let capture = acquired.source_capture();
    let lineage = package_source::SourceLineage::ExternalLocal(
        package_source::ExternalLocalLineage::from_canonical_path(
            capture.canonical_live_root().to_str().unwrap().to_owned(),
            ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
        )
        .unwrap(),
    );
    let custody = PackageSourceCustody::from_resolved_parts(
        PackageKey::new(
            PackageName::parse("machine_self_call_recursion_compile").unwrap(),
            lineage,
        ),
        BuildDeclarationKind::Application,
        package_source::ImmutableSourceResolution::external_local(
            package_source::SourceContentDigest::derive(
                capture.normalized().content_identity.as_bytes(),
            ),
        ),
        PackageSourceMaterialization::from_local(capture.normalized()),
        capture.canonical_live_root().to_path_buf(),
        PackageSourceNavigation::Root,
        PackageSourceSelectionEvidence::Root,
        LocalSourceLimits::default(),
        DependencyProjections::new(Vec::new().into(), Vec::new().into()),
    );
    let step = begin_source_closure::<String>(
        PackageRootSourceRequest::ExternalLocal {
            requested_root: preparation.canonical_root.clone(),
            source_context: ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
        },
        custody,
        Default::default(),
    )
    .unwrap();
    match step {
        SourceClosureStep::Complete(closure) => closure,
        SourceClosureStep::Request(_) => panic!("empty ledger requested source"),
    }
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
fn completion_keeps_the_declared_source_identity() {
    let temporary = project("calls/machine_self_call_recursion_compile");
    let pending = begin(temporary.path());
    let graph = source_graph(&pending);
    let prepared = pending.complete(graph).unwrap();
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
    let closure = source_graph(&second);
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
        let closure = source_graph(&pending);
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
