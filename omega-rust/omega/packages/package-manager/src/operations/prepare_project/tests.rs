// Unit tests for local project preparation lifecycle, transaction exclusion, and lock recovery gating.

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

#[test]
fn build_reuse_refuses_both_widening_and_narrowing_the_captured_authority() {
    use package_source::{SourceCaptureObligation as Obligation, SourceCaptureRequest};
    let temporary = project("calls/machine_self_call_recursion_compile");
    std::fs::write(temporary.path().join("other.omg"), "const OTHER: u32 = 1;").unwrap();
    let pending = begin(temporary.path());
    let selected = SourceCaptureRequest::new([
        (b"main.omg".to_vec(), Obligation::Required),
        (b"build.omg".to_vec(), Obligation::Required),
    ])
    .unwrap();
    let narrow = pending.acquire_selected_root(&selected).unwrap();
    let wide = pending.acquire_root().unwrap();
    assert!(narrow.same_source(&wide));
    assert!(
        !narrow
            .source_capture()
            .tree()
            .exists(Path::new("other.omg"))
    );
    assert!(wide.source_capture().tree().exists(Path::new("other.omg")));
    for (completed, requested) in [(&wide, &narrow), (&narrow, &wide)] {
        assert_eq!(
            completed.verify_reused_capture(requested).unwrap_err(),
            "reused Build source changed its immutable capture"
        );
    }
    narrow
        .verify_reused_capture(&pending.acquire_selected_root(&selected).unwrap())
        .unwrap();
}

#[test]
fn build_reuse_preserves_optional_request_authority_even_with_equal_captured_bytes() {
    use package_source::{SourceCaptureObligation as Obligation, SourceCaptureRequest};
    let temporary = project("calls/machine_self_call_recursion_compile");
    let pending = begin(temporary.path());
    let required = [
        (b"main.omg".to_vec(), Obligation::Required),
        (b"build.omg".to_vec(), Obligation::Required),
    ];
    let selected = SourceCaptureRequest::new(required.clone()).unwrap();
    let with_optional = SourceCaptureRequest::new(
        required
            .into_iter()
            .chain([(b"absent.omg".to_vec(), Obligation::Optional)]),
    )
    .unwrap();
    let narrow = pending.acquire_selected_root(&selected).unwrap();
    let optional = pending.acquire_selected_root(&with_optional).unwrap();
    assert!(narrow.same_source(&optional));
    assert_eq!(
        narrow.source_capture().tree(),
        optional.source_capture().tree()
    );
    for (completed, requested) in [(&narrow, &optional), (&optional, &narrow)] {
        assert!(completed.verify_reused_capture(requested).is_err());
    }
}

#[test]
fn selected_project_preparation_requires_its_actual_compilation_entry() {
    use package_source::{SourceCaptureObligation as Obligation, SourceCaptureRequest};
    let temporary = project("calls/machine_self_call_recursion_compile");
    let pending = begin(temporary.path());
    let only_build =
        SourceCaptureRequest::new([(b"build.omg".to_vec(), Obligation::Required)]).unwrap();
    let error = pending.acquire_selected_root(&only_build).err().unwrap();
    assert!(error.to_string().contains("required compilation source"));
}

#[test]
fn package_candidate_reacquisition_keeps_the_staged_selection_and_live_lineage() {
    use crate::declarations::dependencies::edit::BuildFileReplacement;
    use package_source::{SourceCaptureObligation as Obligation, SourceCaptureRequest};
    let temporary = project("calls/machine_self_call_recursion_compile");
    std::fs::write(
        temporary.path().join("unselected.omg"),
        "const OTHER: u32 = 1;",
    )
    .unwrap();
    let selected = SourceCaptureRequest::new([
        (b"main.omg".to_vec(), Obligation::Required),
        (b"build.omg".to_vec(), Obligation::Required),
        (b"absent.omg".to_vec(), Obligation::Optional),
    ])
    .unwrap();
    let original = begin(temporary.path())
        .acquire_selected_root(&selected)
        .unwrap();
    let before = std::fs::read_to_string(temporary.path().join("build.omg")).unwrap();
    let replacement = BuildFileReplacement::from_sources(
        temporary.path().join("build.omg"),
        &before,
        before.clone(),
    );
    let storage = SourceResolver::for_current_user(PrimaryGitChoices::default()).unwrap();
    let staged = crate::operations::stage_build_dependency_edit(
        &replacement,
        &storage,
        LocalSourceLimits::default(),
        Some(&selected),
    )
    .unwrap();
    let candidate = AcquiredBuildSource::capture_staged(
        &staged,
        ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
    )
    .unwrap();
    assert!(candidate.same_source(&original));
    assert_eq!(
        candidate.source_capture().tree(),
        original.source_capture().tree()
    );
    assert_eq!(
        candidate.source_capture().selection_identity(),
        original.source_capture().selection_identity()
    );
    assert!(
        !candidate
            .source_capture()
            .tree()
            .exists(Path::new("unselected.omg"))
    );
    assert_eq!(
        candidate.source_capture().normalized().content_identity,
        staged.normalized().content_identity
    );
}
